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
    "build firmware with scripts/build-esp32.sh s3|s2|p4 (ESP-IDF target, --features esp32)"
);

use esp_idf_svc::hal::{cpu::Core, peripherals::Peripherals};
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
#[cfg(not(feature = "board-p4"))]
mod status_led;

#[cfg(feature = "board-p4")]
mod status_led {
    pub fn start(_pins: esp_idf_svc::hal::gpio::Pins) {
        log::info!("This P4 board has no configured status LED");
    }
    pub fn configure(_config: &nanacoin::board_status::LightConfig) {}
}

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

#[cfg(not(feature = "board-p4"))]
fn reset_reason(reason: esp_idf_svc::hal::reset::ResetReason) -> &'static str {
    use esp_idf_svc::hal::reset::ResetReason::*;
    match reason {
        PowerOn => "power on",
        ExternalPin => "reset pin",
        Software => "software restart",
        Panic => "crash",
        InterruptWatchdog | TaskWatchdog | Watchdog => "watchdog",
        DeepSleep => "deep sleep",
        Brownout => "brownout (power dipped)",
        USBPeripheral => "USB",
        _ => "other",
    }
}

fn reset_code(reason: esp_idf_svc::hal::reset::ResetReason) -> i32 {
    use esp_idf_svc::{hal::reset::ResetReason::*, sys::*};
    (match reason {
        Unknown => esp_reset_reason_t_ESP_RST_UNKNOWN,
        PowerOn => esp_reset_reason_t_ESP_RST_POWERON,
        ExternalPin => esp_reset_reason_t_ESP_RST_EXT,
        Software => esp_reset_reason_t_ESP_RST_SW,
        Panic => esp_reset_reason_t_ESP_RST_PANIC,
        InterruptWatchdog => esp_reset_reason_t_ESP_RST_INT_WDT,
        TaskWatchdog => esp_reset_reason_t_ESP_RST_TASK_WDT,
        Watchdog => esp_reset_reason_t_ESP_RST_WDT,
        DeepSleep => esp_reset_reason_t_ESP_RST_DEEPSLEEP,
        Brownout => esp_reset_reason_t_ESP_RST_BROWNOUT,
        Sdio => esp_reset_reason_t_ESP_RST_SDIO,
        USBPeripheral => esp_reset_reason_t_ESP_RST_USB,
        JTAG => esp_reset_reason_t_ESP_RST_JTAG,
        EfuseError => esp_reset_reason_t_ESP_RST_EFUSE,
        PowerGlitch => esp_reset_reason_t_ESP_RST_PWR_GLITCH,
        CPULockup => esp_reset_reason_t_ESP_RST_CPU_LOCKUP,
    }) as i32
}

/// Each bank has a leaf for its own hostname, signed by the household CA.
#[cfg(not(any(feature = "board-s2", feature = "board-p4")))]
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

#[cfg(feature = "board-p4")]
fn board_config() -> BoardConfig {
    let mut config = BoardConfig::p4(
        env!("NANACOIN_WIFI_SSID"),
        env!("NANACOIN_WIFI_PASSWORD"),
        nanacoin::board::HOSTNAME,
    );
    config.instance = "NanaCoin P4 household bank";
    config.cert_pem =
        concat!(include_str!("../../certs/nanacoin-p4-ca-signed.crt"), "\0").as_bytes();
    config.key_pem =
        concat!(include_str!("../../certs/nanacoin-p4-ca-signed.key"), "\0").as_bytes();
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
    // An explicit emulator build can supply time without reaching an external
    // NTP server. Normal firmware has no clock override or extra HTTP route.
    #[cfg(feature = "emulator-clock")]
    {
        let seconds: i64 = env!("NANACOIN_EMULATOR_UNIX_SECONDS").parse()?;
        let initial = esp_idf_svc::sys::timeval {
            tv_sec: seconds.try_into()?,
            tv_usec: 0,
        };
        // SAFETY: initialized timeval, no timezone pointer; IDF synchronizes
        // the system clock. This runs before service or scheduler creation.
        if unsafe { esp_idf_svc::sys::settimeofday(&initial, std::ptr::null()) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        log::info!("Emulator conformance clock initialized at {seconds}");
    }
    #[cfg(feature = "cobol-core")]
    nanacoin::cobol::initialize().map_err(|error| format!("COBOL bank startup: {error}"))?;
    log::info!(
        "Banking engine: {}",
        if cfg!(feature = "cobol-core") {
            "cobol"
        } else {
            "rust"
        }
    );
    miniframework::events::observe(|event| nanacoin::incidents::LOG.apply(incidents::now(), event));
    incidents::boot()?;
    let peripherals = Peripherals::take()?;
    status_led::start(peripherals.pins);
    let mut config = board_config();
    // The COBOL call frames and publication plans add to the JSON/framework
    // frames. Keep the default Rust profile; the optional engine needs a
    // larger internal serving stack on boards with a separate serving task.
    if cfg!(feature = "cobol-core") {
        config.serve_stack = 64 * 1024;
        if !cfg!(feature = "board-p4") && !cfg!(feature = "board-s2") {
            // The S3 main task already owns a 64 KiB internal stack. Serving
            // there avoids requesting another contiguous block after Wi-Fi
            // and libcob have fragmented the smaller internal heap.
            config.app_core = None;
        }
    }
    let board = esp::start(config, peripherals.modem)?;
    let platform = board.platform(peripherals.temp_sensor);
    let temperature = platform.temperature_reader();
    let station = platform.station_reader();
    incidents::start(platform.station_reader())?;

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
        move || sampler.run(temperature, station),
    )?;
    let mut screen = nanacoin::screen::Worker::spawn_with_resolver(board.resolver())?;

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
        platform,
    );
    let ready_ms = incidents::now() as u32;
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
