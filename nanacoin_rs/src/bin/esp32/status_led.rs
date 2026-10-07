//! Optional WS2812 RGB diagnostics. Never owns the service or network lock.
//! A missing LED cannot be detected (WS2812 has no acknowledgement).
//!
//! Startup steps, Wi-Fi, mDNS, readiness and fatal errors come from
//! miniframework's `SIGNALS` (the board runner sets them); the colours,
//! Morse rotation and the household's light settings are NanaCoin's.
use esp_idf_svc::{
    hal::{
        gpio::{AnyOutputPin, PinDriver, Pins},
        rmt::{
            config::TxChannelConfig, encoder::CopyEncoder, PinState, RmtChannel, Symbol,
            TxChannelDriver,
        },
        task::thread::ThreadSpawnConfiguration,
        units::Hertz,
    },
    sys::EspError,
};
use miniframework::status::SIGNALS;
use nanacoin::{
    board_status::{self, Health, LedPin},
    incidents::{Kind, LOG},
};
use std::time::Duration;

static CONFIG: std::sync::Mutex<Option<board_status::LightConfig>> = std::sync::Mutex::new(None);
pub fn configure(config: &board_status::LightConfig) {
    let mut current = CONFIG.lock().unwrap();
    if current.as_ref() != Some(config) {
        *current = Some(config.clone());
    }
}

/// The framework's view of the board, in NanaCoin's health terms.
fn health(now: u64, recent_error: bool) -> Health {
    let board = SIGNALS.health(recent_error);
    Health {
        ready: board.ready,
        wifi: board.wifi,
        setup: false,
        workers: LOG.workers_healthy(now),
        mdns: board.mdns,
        fatal: SIGNALS.is_fatal() || LOG.count(Kind::StorageFailed) > 0,
        recent_error,
    }
}

pub fn start(pins: Pins) {
    let pin: AnyOutputPin<'static> =
        match board_status::led_pin(option_env!("NANACOIN_STATUS_LED_PIN")) {
            Ok(None) => {
                log::info!("Status LED disabled (NANACOIN_STATUS_LED_PIN=off)");
                return;
            }
            #[cfg(not(feature = "board-s2"))]
            Ok(Some(LedPin::Gpio38)) => pins.gpio38.into(),
            #[cfg(not(feature = "board-s2"))]
            Ok(Some(LedPin::Gpio48)) => pins.gpio48.into(),
            #[cfg(feature = "board-s2")]
            Ok(Some(LedPin::Gpio15)) => pins.gpio15.into(),
            Ok(Some(other)) => {
                log::warn!("Status LED: {other:?} is not this board's LED; LED disabled");
                return;
            }
            Err(e) => {
                log::warn!("Status LED configuration: {e}");
                return;
            }
        };
    // Keep optional diagnostics lower priority than both server workers.
    if let Err(e) = (ThreadSpawnConfiguration {
        name: Some(c"status-led"),
        priority: 2,
        pin_to_core: super::NETWORK_CORE,
        #[cfg(feature = "board-s2")]
        stack_alloc_caps: esp_idf_svc::hal::task::thread::MallocCap::Spiram
            | esp_idf_svc::hal::task::thread::MallocCap::Cap8bit,
        ..Default::default()
    })
    .set()
    {
        log::warn!("Status LED disabled: thread configuration failed: {e}");
        return;
    }
    let spawned = std::thread::Builder::new()
        .stack_size(6 * 1024)
        .spawn(move || {
            let result = if cfg!(feature = "board-s2") {
                run_mono(pin)
            } else {
                run(pin)
            };
            if let Err(e) = result {
                log::warn!("Status LED disabled: {e}; server continues");
            }
        });
    if let Err(e) = ThreadSpawnConfiguration::default().set() {
        log::warn!("Status LED: could not restore thread defaults: {e}");
    }
    if let Err(e) = spawned {
        log::warn!("Status LED disabled: could not start task: {e}");
    }
}

/// Single-colour LED (S2 Mini GPIO15): the startup marker and reset flashes,
/// the S3's Morse rotation while healthy, and otherwise one rhythm per health
/// state; see `board_status::mono`.
fn run_mono(pin: AnyOutputPin<'static>) -> Result<(), EspError> {
    let mut led = PinDriver::output(pin)?;
    let began = super::incidents::now();
    let reason = super::reset_reason(esp_idf_svc::hal::reset::ResetReason::get());
    let flashes = board_status::reset_flashes(reason);
    log::info!("Status LED: single colour, GPIO 15; reset {reason} ({flashes} flashes)");
    let mut config = board_status::LightConfig::default();
    let mut pattern = board_status::Rotation::new(&config);
    let mut healthy_since = None;
    let mut last_errors = 0;
    let mut error_until = 0;
    let mut previous = None;
    loop {
        let now = super::incidents::now();
        let errors = LOG
            .count(Kind::AllocationFailed)
            .wrapping_add(LOG.count(Kind::TlsInitFailed));
        if errors != last_errors {
            error_until = now + 10_000;
            last_errors = errors;
        }
        let health = health(now, now < error_until);
        if let Ok(pending) = CONFIG.try_lock() {
            if let Some(next) = pending.as_ref().filter(|next| *next != &config) {
                config.clone_from(next);
                pattern = board_status::Rotation::new(&config);
                healthy_since = None;
            }
        }
        let state = health.state();
        let startup = board_status::reset_color(now - began, flashes);
        if state != board_status::State::Healthy || startup.is_some() {
            healthy_since = None;
        }
        let on = if SIGNALS.is_fatal() {
            board_status::stage_code(SIGNALS.current_stage(), now)
        } else if health.fatal {
            board_status::mono(state, now)
        } else if let Some(color) = startup {
            color != [0, 0, 0]
        } else if state == board_status::State::Healthy {
            let began = *healthy_since.get_or_insert(now);
            pattern.color(now - began) != [0, 0, 0]
        } else {
            board_status::mono(state, now)
        };
        if previous != Some(on) {
            if on {
                led.set_high()?;
            } else {
                led.set_low()?;
            }
            previous = Some(on);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn run(pin: AnyOutputPin<'static>) -> Result<(), EspError> {
    let config = TxChannelConfig {
        resolution: Hertz(10_000_000),
        transaction_queue_depth: 1,
        ..Default::default()
    };
    let mut channel = TxChannelDriver::new(pin, &config)?;
    let encoder = CopyEncoder::new()?;
    let zero = Symbol::new_with(
        config.resolution,
        PinState::High,
        Duration::from_nanos(300),
        PinState::Low,
        Duration::from_nanos(900),
    )?;
    let one = Symbol::new_with(
        config.resolution,
        PinState::High,
        Duration::from_nanos(800),
        PinState::Low,
        Duration::from_nanos(400),
    )?;
    let latch = Symbol::new_half_split(
        config.resolution,
        PinState::Low,
        PinState::Low,
        Duration::from_micros(300),
    )?;
    let mut queue = channel.queue([encoder]);
    let transmit = esp_idf_svc::hal::rmt::config::TransmitConfig {
        queue_non_blocking: true,
        ..Default::default()
    };
    let began = super::incidents::now();
    let reason = super::reset_reason(esp_idf_svc::hal::reset::ResetReason::get());
    let flashes = board_status::reset_flashes(reason);
    log::info!(
        "Status LED: WS2812 GRB, GPIO {}, dim output; reset {reason} ({flashes} white flashes)",
        option_env!("NANACOIN_STATUS_LED_PIN").unwrap_or("off")
    );
    let mut config = board_status::LightConfig::default();
    let mut pattern = board_status::Rotation::new(&config);
    let mut healthy_since = None;
    let mut previous = None;
    let mut last_errors = 0;
    let mut error_until = 0;
    loop {
        let now = super::incidents::now();
        let errors = LOG
            .count(Kind::AllocationFailed)
            .wrapping_add(LOG.count(Kind::TlsInitFailed));
        if errors != last_errors {
            error_until = now + 10_000;
            last_errors = errors;
        }
        let health = health(now, now < error_until);
        if let Ok(pending) = CONFIG.try_lock() {
            if let Some(next) = pending.as_ref().filter(|next| *next != &config) {
                config.clone_from(next);
                pattern = board_status::Rotation::new(&config);
                healthy_since = None;
            }
        }
        let state = health.state();
        let startup = board_status::reset_color(now - began, flashes);
        if state != board_status::State::Healthy || startup.is_some() {
            healthy_since = None;
        }
        // A failed startup blinks its step number in red (see DEPLOY.md);
        // stalled workers or storage failure while running stay solid red.
        let color = if SIGNALS.is_fatal() {
            if board_status::stage_code(SIGNALS.current_stage(), now) {
                [8, 0, 0]
            } else {
                [0, 0, 0]
            }
        } else if health.fatal {
            board_status::color(health.state(), now)
        } else {
            startup.unwrap_or_else(|| {
                if state == board_status::State::Healthy {
                    let began = *healthy_since.get_or_insert(now);
                    pattern.color(now - began)
                } else {
                    board_status::color(state, now)
                }
            })
        };
        if previous != Some(color) {
            let [r, g, b] = color;
            let mut symbols = [latch; 25];
            for (byte_index, byte) in [g, r, b].into_iter().enumerate() {
                for bit in 0..8 {
                    symbols[byte_index * 8 + bit] =
                        if byte & (0x80 >> bit) == 0 { zero } else { one };
                }
            }
            // The queue owns a copy of the frame until RMT finishes. No
            // application thread waits on this task, even on driver failure.
            let result = queue.push(&symbols, &transmit).and_then(|()| {
                queue
                    .channel()
                    .wait_all_done(Some(Duration::from_millis(20)))
            });
            if let Err(e) = result {
                // Cancel before dropping the queue's buffers. If cancellation
                // fails, retain its small buffers until reboot rather than
                // freeing memory still referenced by the peripheral.
                if queue.channel().disable().is_err() {
                    std::mem::forget(queue);
                }
                return Err(e);
            }
            previous = Some(color);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
