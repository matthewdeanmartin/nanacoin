//! A NanaCoin bank on miniframework's board runner. The framework brings up
//! Wi-Fi, SNTP, mDNS, HTTP and HTTPS, and serves the site; this file opens
//! the ledger, starts NanaCoin's own light, incident sampler, diagnostics
//! and screen worker, and runs scheduled payments.
//!
//! Startup steps (a failed startup blinks the number and explains itself
//! on http://<board>:8080/): 1-7 are miniframework's (system, NVS, Wi-Fi
//! driver, Wi-Fi join, network, app, listen); NanaCoin adds:
//! 8 ledger partition, 9 journal storage, 10 ledger replay, 11 background
//! tasks.

#[cfg(not(target_os = "espidf"))]
compile_error!(
    "build firmware with scripts/build-esp32.sh s3|s2 (xtensa target, --features esp32)"
);

use esp_idf_svc::{
    hal::{cpu::Core, peripherals::Peripherals},
    sys,
};
use miniframework::{
    esp::{self, BoardConfig},
    status::SIGNALS,
    sys::STATS,
    Site,
};
use nanacoin::{
    incidents::Kind,
    journal::Service,
    server::{self, Bank},
};
use std::{
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};

#[path = "esp32/diagnostics.rs"]
mod diagnostics;
use diagnostics::Diagnostics;

#[path = "esp32/incidents.rs"]
mod incidents;

#[path = "esp32/journal.rs"]
mod nvs_journal;
use nvs_journal::NvsJournal;

#[path = "esp32/status_led.rs"]
mod status_led;

/// Where NanaCoin's own tasks (light, samplers) run. The S3 keeps them with
/// Wi-Fi and TLS handshakes on core 0, leaving core 1 to serving; the
/// single-core S2 leaves tasks unpinned.
#[cfg(not(feature = "board-s2"))]
pub(crate) const NETWORK_CORE: Option<Core> = Some(Core::Core0);
#[cfg(feature = "board-s2")]
pub(crate) const NETWORK_CORE: Option<Core> = None;

/// Deployment tooling refuses an image whose marker names another board.
#[used]
static BOARD_MARKER: &str = nanacoin::board::MARKER;

#[allow(non_upper_case_globals)] // ESP-IDF generated constant names.
fn reset_reason(reason: sys::esp_reset_reason_t) -> &'static str {
    use sys::*;
    match reason {
        esp_reset_reason_t_ESP_RST_POWERON => "power on",
        esp_reset_reason_t_ESP_RST_EXT => "reset pin",
        esp_reset_reason_t_ESP_RST_SW => "software restart",
        esp_reset_reason_t_ESP_RST_PANIC => "crash",
        esp_reset_reason_t_ESP_RST_INT_WDT
        | esp_reset_reason_t_ESP_RST_TASK_WDT
        | esp_reset_reason_t_ESP_RST_WDT => "watchdog",
        esp_reset_reason_t_ESP_RST_DEEPSLEEP => "deep sleep",
        esp_reset_reason_t_ESP_RST_BROWNOUT => "brownout (power dipped)",
        esp_reset_reason_t_ESP_RST_USB => "USB",
        _ => "other",
    }
}

/// Each bank has a leaf for its own hostname, signed by the household CA.
#[cfg(not(feature = "board-s2"))]
fn board_config() -> BoardConfig {
    let mut config = BoardConfig::s3(
        env!("NANACOIN_WIFI_SSID"),
        env!("NANACOIN_WIFI_PASSWORD"),
        nanacoin::board::HOSTNAME,
    );
    config.instance = "NanaCoin Rust household ledger";
    config.cert_pem = concat!(include_str!("../../certs/nanacoin-ca-signed.crt"), "\0").as_bytes();
    config.key_pem = concat!(include_str!("../../certs/nanacoin-ca-signed.key"), "\0").as_bytes();
    // At most this plus one maximum reply is held for slow readers.
    config.limits.response_budget = 2 * 1024 * 1024;
    config.ntp_server = option_env!("NANACOIN_NTP_SERVER");
    config
}

/// 2 MiB PSRAM: each TLS session keeps a 16 KiB receive record buffer.
#[cfg(feature = "board-s2")]
fn board_config() -> BoardConfig {
    let mut config = BoardConfig::s2(
        env!("NANACOIN_WIFI_SSID"),
        env!("NANACOIN_WIFI_PASSWORD"),
        nanacoin::board::HOSTNAME,
    );
    config.instance = "NanaCoin second household bank";
    config.cert_pem =
        concat!(include_str!("../../certs/nanacoin-s2-ca-signed.crt"), "\0").as_bytes();
    config.key_pem =
        concat!(include_str!("../../certs/nanacoin-s2-ca-signed.key"), "\0").as_bytes();
    config.limits.tls_clients = 3;
    config.limits.http_clients = 2;
    config.limits.handshakes = 1;
    config.limits.response_budget = 2 * nanacoin::api::RESPONSE_LIMIT;
    config.ntp_server = option_env!("NANACOIN_NTP_SERVER");
    config
}

fn scheduled_payments(shared: &Mutex<Service<NvsJournal>>, screen: &mut nanacoin::screen::Worker) {
    let mut service = shared.lock().unwrap();
    screen.pump(&mut service);
    let failed_before = service.storage_failed();
    if let Err(error) = service.tick() {
        log::warn!("Scheduled payments: {error:?}");
    }
    if !failed_before && service.storage_failed() {
        incidents::record(Kind::StorageFailed, 0);
    }
}

fn main() {
    if let Err(error) = run() {
        // Blinks the step, logs every 5 s, explains itself on port 8080.
        esp::fail(&error.to_string());
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    esp::init();
    miniframework::events::observe(|event| nanacoin::incidents::LOG.apply(incidents::now(), event));
    incidents::start()?;
    let peripherals = Peripherals::take()?;
    status_led::start(peripherals.pins);
    let board = esp::start(board_config(), peripherals.modem)?;

    SIGNALS.step(8, "ledger partition");
    let partition = board.partition("ledger")?;
    SIGNALS.step(9, "journal storage");
    let mut journal = NvsJournal::open(partition).map_err(|e| format!("journal storage: {e:?}"))?;
    if option_env!("NANACOIN_RECOVER_HTTP") == Some("1") {
        use nanacoin::journal::Journal;
        journal
            .set_https_only(false)
            .map_err(|e| format!("transport recovery: {e:?}"))?;
        log::warn!("USB recovery build: HTTP enabled; reinstall normal firmware next");
    }
    SIGNALS.step(10, "ledger replay");
    let service = Service::open(journal).map_err(|e| format!("ledger startup: {e:?}"))?;
    status_led::configure(&service.light_config);
    let shared = Arc::new(Mutex::new(service));

    SIGNALS.step(11, "background tasks");
    // Core 0 owns live probes. Only a small snapshot copy uses the
    // diagnostics mutex; ledger work and sockets never hold it. Never writes
    // flash, so on the S2 its stack may live in PSRAM.
    let samples = Arc::new(Diagnostics::default());
    let sampler = Arc::clone(&samples);
    esp::spawn_task(
        c"nanacoin-diag",
        4096,
        cfg!(feature = "board-s2"),
        NETWORK_CORE,
        0,
        move || sampler.run(),
    )?;
    let mut screen = nanacoin::screen::Worker::spawn()?;

    let origins = option_env!("NANACOIN_ORIGINS").unwrap_or(nanacoin::api::DEFAULT_ORIGINS);
    let latest = Arc::clone(&samples);
    let bank = Bank::new(Arc::clone(&shared))
        .with_diagnostics(Box::new(move |path, out| {
            if path.ends_with("/events") {
                let limit = out.len().min(nanacoin::incidents::RESPONSE_BYTES);
                nanacoin::diagnostics::response(
                    &nanacoin::incidents::LOG.snapshot(),
                    &mut out[..limit],
                )
            } else if path.ends_with("/static") {
                nanacoin::diagnostics::response(&diagnostics::system_info(), out)
            } else {
                let mut snapshot = latest.snapshot();
                snapshot.requests = STATS.requests.load(Ordering::Relaxed);
                snapshot.errors = STATS.errors.load(Ordering::Relaxed);
                nanacoin::diagnostics::response(&snapshot, out)
            }
        }))
        .on_storage_failed(|| incidents::record(Kind::StorageFailed, 0));
    let site = Site::new(
        server::config(nanacoin::board::FQDN, origins),
        bank,
        board.platform(),
    );
    // SAFETY: monotonic timer query has no pointer arguments.
    let ready_ms = unsafe { sys::esp_timer_get_time() / 1000 } as u32;
    samples.boot_ready_ms.store(ready_ms, Ordering::Relaxed);
    incidents::record(Kind::Ready, 0);
    log::info!(
        "Ready at https://{} after {ready_ms} ms; board {}; bundled UI + API",
        nanacoin::board::FQDN,
        nanacoin::board::ID,
    );
    // Scheduled payments write flash, so they run on the calling (main)
    // task, whose stack is internal RAM: between connection-loop turns on
    // the S2, beside the serving core on the S3.
    let mut last_tick = Instant::now();
    let mut last_light = last_tick;
    board.serve(site, move |_| {
        if last_tick.elapsed() >= Duration::from_secs(1) {
            last_tick = Instant::now();
            scheduled_payments(&shared, &mut screen);
        }
        if last_light.elapsed() >= Duration::from_secs(10) {
            last_light = Instant::now();
            if let Ok(service) = shared.try_lock() {
                status_led::configure(&service.light_config);
            }
        }
    })
}
