//! Optional WS2812 RGB diagnostics. Never owns the service or network lock.
//! A missing LED cannot be detected (WS2812 has no acknowledgement).
use esp_idf_svc::{
    hal::{
        cpu::Core,
        gpio::{AnyOutputPin, Pins},
        rmt::{
            config::TxChannelConfig, encoder::CopyEncoder, PinState, RmtChannel, Symbol,
            TxChannelDriver,
        },
        task::thread::ThreadSpawnConfiguration,
        units::Hertz,
    },
    sys::EspError,
};
use nanacoin::{
    board_status::{self, Health, LedPin},
    incidents::{Kind, LOG},
};
use std::{
    sync::atomic::{AtomicBool, Ordering::Relaxed},
    time::Duration,
};

static READY: AtomicBool = AtomicBool::new(false);
static WIFI: AtomicBool = AtomicBool::new(false);
static MDNS: AtomicBool = AtomicBool::new(false);
static FATAL: AtomicBool = AtomicBool::new(false);

pub fn wifi(up: bool) {
    WIFI.store(up, Relaxed);
}
pub fn ready(mdns: bool) {
    MDNS.store(mdns, Relaxed);
    READY.store(true, Relaxed);
}
pub fn fatal() {
    FATAL.store(true, Relaxed);
}

pub fn start(pins: Pins) {
    let pin: AnyOutputPin<'static> =
        match board_status::led_pin(option_env!("NANACOIN_STATUS_LED_PIN")) {
            Ok(None) => {
                log::info!("Status LED disabled (NANACOIN_STATUS_LED_PIN=off)");
                return;
            }
            Ok(Some(LedPin::Gpio38)) => pins.gpio38.into(),
            Ok(Some(LedPin::Gpio48)) => pins.gpio48.into(),
            Err(e) => {
                log::warn!("Status LED configuration: {e}");
                return;
            }
        };
    // Keep optional diagnostics lower priority than both server workers.
    if let Err(e) = (ThreadSpawnConfiguration {
        name: Some(c"status-led"),
        priority: 2,
        pin_to_core: Some(Core::Core0),
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
            if let Err(e) = run(pin) {
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
    // SAFETY: read-only reset query, no arguments.
    let reason = super::reset_reason(unsafe { esp_idf_svc::sys::esp_reset_reason() });
    let flashes = board_status::reset_flashes(reason);
    log::info!(
        "Status LED: WS2812 GRB, GPIO {}, dim output; reset {reason} ({flashes} white flashes)",
        option_env!("NANACOIN_STATUS_LED_PIN").unwrap_or("off")
    );
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
        let health = Health {
            ready: READY.load(Relaxed),
            wifi: WIFI.load(Relaxed),
            setup: false,
            workers: LOG.workers_healthy(now),
            mdns: MDNS.load(Relaxed),
            fatal: FATAL.load(Relaxed) || LOG.count(Kind::StorageFailed) > 0,
            recent_error: now < error_until,
        };
        let color = if health.fatal {
            board_status::color(health.state(), now)
        } else {
            board_status::reset_color(now - began, flashes)
                .unwrap_or_else(|| board_status::color(health.state(), now))
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
