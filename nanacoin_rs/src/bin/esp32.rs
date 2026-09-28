#[cfg(not(target_os = "espidf"))]
compile_error!(
    "build firmware with scripts/build-esp32.sh s3|s2 (xtensa target, --features esp32)"
);

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{cpu::Core, peripherals::Peripherals, task::thread::ThreadSpawnConfiguration},
    mdns::EspMdns,
    nvs::{EspDefaultNvsPartition, EspNvsPartition, NvsCustom},
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use nanacoin::journal::Service;
use std::{
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[cfg(feature = "board-s2")]
use esp_idf_svc::hal::task::thread::MallocCap;

#[path = "esp32/diagnostics.rs"]
mod diagnostics;
use diagnostics::Diagnostics;

#[path = "esp32/server.rs"]
mod server;

#[path = "esp32/incidents.rs"]
mod incidents;

#[path = "esp32/journal.rs"]
mod nvs_journal;
use nvs_journal::NvsJournal;

#[path = "esp32/status_led.rs"]
mod status_led;

/// Task placement. The S3 keeps Wi-Fi, TLS handshakes and diagnostics on core 0
/// and HTTP/ledger work on core 1. The single-core S2 leaves tasks unpinned.
#[cfg(not(feature = "board-s2"))]
pub(crate) const NETWORK_CORE: Option<Core> = Some(Core::Core0);
#[cfg(not(feature = "board-s2"))]
pub(crate) const APP_CORE: Option<Core> = Some(Core::Core1);
#[cfg(feature = "board-s2")]
pub(crate) const NETWORK_CORE: Option<Core> = None;

/// Last Wi-Fi disconnect, for the main task to log. IDF reason codes:
/// 2 auth expired, 15 4-way handshake timeout (often a wrong password),
/// 201 no AP found, 202 auth failed, 203 association failed.
static LAST_WIFI_REASON: AtomicI32 = AtomicI32::new(0);
static LAST_WIFI_RSSI: AtomicI32 = AtomicI32::new(0);

/// Deployment tooling refuses an image whose marker names another board.
#[used]
static BOARD_MARKER: &str = nanacoin::board::MARKER;

#[allow(non_upper_case_globals)] // ESP-IDF generated constant names.
fn reset_reason(reason: esp_idf_svc::sys::esp_reset_reason_t) -> &'static str {
    use esp_idf_svc::sys::*;
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

fn scheduled_payments(shared: &Mutex<Service<NvsJournal>>) {
    let mut service = shared.lock().unwrap();
    let failed_before = service.storage_failed();
    if let Err(error) = service.tick() {
        log::warn!("Scheduled payments: {error:?}");
    }
    if !failed_before && service.storage_failed() {
        incidents::record(nanacoin::incidents::Kind::StorageFailed, 0);
    }
}

fn main() {
    if let Err(error) = run() {
        status_led::fatal();
        // Repeat: a native-USB console (the S2) attaches after early boot
        // output is gone, so a single line would never be seen.
        loop {
            log::error!("Startup failed: {error}");
            std::thread::sleep(Duration::from_secs(5));
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;
    status_led::start(peripherals.pins);
    // Startup steps, counted out by a single-colour LED if startup fails.
    status_led::stage(1); // incidents and system event loop
    incidents::start()?;
    let event_loop = EspSystemEventLoop::take()?;
    let _wifi_events = event_loop.subscribe::<esp_idf_svc::wifi::WifiEvent, _>(|event| {
        if let esp_idf_svc::wifi::WifiEvent::StaDisconnected(info) = event {
            // Runs on IDF's small system-event task: record only. The main
            // task logs these; logging here stalled the S2.
            LAST_WIFI_REASON.store(i32::from(info.reason()), Ordering::Relaxed);
            LAST_WIFI_RSSI.store(i32::from(info.rssi()), Ordering::Relaxed);
            status_led::wifi(false);
            incidents::record(
                nanacoin::incidents::Kind::WifiDown,
                i32::from(info.reason()),
            );
        }
    })?;
    let _ip_events =
        event_loop.subscribe::<esp_idf_svc::netif::IpEvent, _>(|event| match event {
            esp_idf_svc::netif::IpEvent::DhcpIpAssigned(_) => status_led::wifi(true),
            esp_idf_svc::netif::IpEvent::DhcpIpDeassigned(_) => status_led::wifi(false),
            _ => {}
        })?;
    status_led::stage(2); // system NVS
                          // Never auto-erase NVS on a version/full error: it may contain data.
    let system_nvs = EspDefaultNvsPartition::take_with(false)?;
    status_led::stage(3); // ledger partition
                          // The svc custom-partition convenience constructor auto-erases on some
                          // init errors. Pre-initialize and propagate every error before calling it.
                          // IDF initialization is idempotent; its second call sees an initialized
                          // partition. Never reach the convenience constructor after a failed init.
                          // SAFETY: the partition name is a static NUL-terminated C string.
    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::nvs_flash_init_partition(c"ledger".as_ptr())
    })?;
    let partition = EspNvsPartition::<NvsCustom>::take("ledger")?;
    status_led::stage(4); // journal storage
    let mut journal = NvsJournal::open(partition).map_err(|e| format!("journal storage: {e:?}"))?;
    if option_env!("NANACOIN_RECOVER_HTTP") == Some("1") {
        use nanacoin::journal::Journal;
        journal
            .set_https_only(false)
            .map_err(|e| format!("transport recovery: {e:?}"))?;
        log::warn!("USB recovery build: HTTP enabled; reinstall normal firmware next");
    }
    status_led::stage(5); // ledger replay
    let service = Service::open(journal).map_err(|e| format!("ledger startup: {e:?}"))?;
    // The mutex covers API/domain work, including JSON encoding; all network
    // I/O and TLS handshakes happen outside it.
    status_led::configure(&service.light_config);
    let shared = Arc::new(Mutex::new(service));
    let diagnostics = Arc::new(Diagnostics::default());
    status_led::stage(6); // Wi-Fi driver
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, event_loop.clone(), Some(system_nvs))?,
        event_loop,
    )?;
    status_led::stage(7); // Wi-Fi configuration and start
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: env!("NANACOIN_WIFI_SSID")
            .try_into()
            .map_err(|_| "SSID exceeds 32 bytes")?,
        password: env!("NANACOIN_WIFI_PASSWORD")
            .try_into()
            .map_err(|_| "Wi-Fi password exceeds 64 bytes")?,
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;
    wifi.start()?;
    // A transient association/DHCP timeout must not terminate the server at
    // boot. Keep retrying just as we do after a later Wi-Fi disconnection.
    loop {
        match wifi.connect().and_then(|_| wifi.wait_netif_up()) {
            Ok(()) => break,
            Err(error) => {
                incidents::record(nanacoin::incidents::Kind::ReconnectFailed, error.code());
                log::warn!(
                    "Initial Wi-Fi connection: {error}; last disconnect reason {} rssi {}; retrying",
                    LAST_WIFI_REASON.load(Ordering::Relaxed),
                    LAST_WIFI_RSSI.load(Ordering::Relaxed)
                );
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
    // From here Wi-Fi is up. A later startup failure keeps Wi-Fi running and
    // explains itself on port 8080, since the S2 may have no usable console.
    if let Err(error) = online(&mut wifi, shared, diagnostics) {
        status_led::fatal();
        report_failure(&*error);
    }
    Ok(())
}

fn online(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    shared: Arc<Mutex<Service<NvsJournal>>>,
    diagnostics: Arc<Diagnostics>,
) -> Result<(), Box<dyn std::error::Error>> {
    status_led::stage(8); // power save and time
                          // USB-powered server: avoid incoming packet delays from modem sleep.
    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::esp_wifi_set_ps(esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
    })?;
    // Keep the SNTP service alive. Timed offers refuse writes until wall time
    // is valid; persisted deadlines must not restart when the board reboots.
    let mut time_config = esp_idf_svc::sntp::SntpConf::default();
    if let Some(server) = option_env!("NANACOIN_NTP_SERVER") {
        time_config.servers.fill(server);
    }
    let _sntp = esp_idf_svc::sntp::EspSntp::new(&time_config)?;

    status_led::stage(9); // HTTP/TLS server
    let http = server::start(server::Context {
        shared: Arc::clone(&shared),
        diagnostics: Arc::clone(&diagnostics),
    })?;
    #[cfg(not(feature = "board-s2"))]
    server::spawn(http)?;
    status_led::stage(10); // mDNS and background tasks
    let _mdns = (|| -> Result<EspMdns, esp_idf_svc::sys::EspError> {
        let mut mdns = EspMdns::take()?;
        // The legacy standalone UI board must not advertise this name concurrently.
        mdns.set_hostname(nanacoin::board::HOSTNAME)?;
        mdns.set_instance_name(if nanacoin::board::ID == "s3" {
            "NanaCoin Rust household ledger"
        } else {
            "NanaCoin second household bank"
        })?;
        mdns.add_service(
            Some("NanaCoin setup"),
            "_http",
            "_tcp",
            80,
            &[("path", "/trust")],
        )?;
        mdns.add_service(
            Some("NanaCoin"),
            "_https",
            "_tcp",
            443,
            &[("path", "/api/v1/status")],
        )?;
        Ok(mdns)
    })()
    .map_err(|error| {
        log::warn!("mDNS unavailable: {error}; server continues by IP");
        error
    })
    .ok();
    // Core 0 owns live probes. Only a small snapshot copy uses the independent
    // diagnostics mutex; ledger work and sockets never hold that mutex.
    let sampler = Arc::clone(&diagnostics);
    // Applies to the next thread spawned on this task, then is restored so it
    // does not leak onto anything spawned later.
    ThreadSpawnConfiguration {
        name: Some(c"nanacoin-diag"),
        pin_to_core: NETWORK_CORE,
        // Never writes flash. The S2's internal RAM cannot hold every worker
        // stack beside Wi-Fi, so there this stack lives in PSRAM.
        #[cfg(feature = "board-s2")]
        stack_alloc_caps: MallocCap::Spiram | MallocCap::Cap8bit,
        ..Default::default()
    }
    .set()?;
    let _sampler = std::thread::Builder::new()
        .stack_size(4096)
        .spawn(move || sampler.run())?;
    // Independent of HTTP traffic and potentially blocking Wi-Fi reconnection.
    // The S2 cannot spare another 24 KiB internal stack (the tick writes
    // flash, so PSRAM is not an option); its main loop runs the tick instead.
    #[cfg(not(feature = "board-s2"))]
    {
        let scheduler = Arc::clone(&shared);
        ThreadSpawnConfiguration {
            name: Some(c"nanacoin-loans"),
            pin_to_core: APP_CORE,
            ..Default::default()
        }
        .set()?;
        let _scheduler = std::thread::Builder::new()
            .stack_size(24 * 1024)
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                scheduled_payments(&scheduler);
            })?;
    }
    ThreadSpawnConfiguration::default().set()?;
    // SAFETY: monotonic timer query has no pointer arguments.
    diagnostics.boot_ready_ms.store(
        unsafe { esp_idf_svc::sys::esp_timer_get_time() / 1000 } as u32,
        Ordering::Relaxed,
    );
    status_led::ready(_mdns.is_some());
    incidents::record(nanacoin::incidents::Kind::Ready, 0);
    log::info!(
        "Ready at https://{}; board {}; bundled UI + API; {}",
        nanacoin::board::FQDN,
        nanacoin::board::ID,
        server::PLACEMENT
    );
    // S3: the main task only keeps house. S2: the main task (whose 32 KiB
    // internal stack already exists) runs the HTTP multiplexer, with the
    // scheduled-payment tick and housekeeping between turns. A Wi-Fi
    // reconnect pauses serving there, which costs nothing while it is down.
    #[cfg(feature = "board-s2")]
    {
        let mut last_tick = std::time::Instant::now();
        let mut last_house = last_tick;
        http.serve(move || {
            if last_tick.elapsed() >= Duration::from_secs(1) {
                last_tick = std::time::Instant::now();
                scheduled_payments(&shared);
            }
            if last_house.elapsed() >= Duration::from_secs(10) {
                last_house = std::time::Instant::now();
                if let Err(error) = housekeeping(wifi, &shared) {
                    log::warn!("Housekeeping: {error}");
                }
            }
        })
    }
    #[cfg(not(feature = "board-s2"))]
    loop {
        std::thread::sleep(Duration::from_secs(10));
        housekeeping(wifi, &shared)?;
    }
}

/// Every ten seconds: LED settings, Wi-Fi reconnection and a heap log line.
fn housekeeping(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    shared: &Mutex<Service<NvsJournal>>,
) -> Result<(), esp_idf_svc::sys::EspError> {
    {
        if let Ok(service) = shared.try_lock() {
            status_led::configure(&service.light_config);
        }
        if !wifi.is_connected()? {
            incidents::record(nanacoin::incidents::Kind::Reconnect, 0);
            log::warn!("Wi-Fi disconnected; reconnecting");
            if let Err(e) = wifi.connect().and_then(|_| wifi.wait_netif_up()) {
                incidents::record(nanacoin::incidents::Kind::ReconnectFailed, e.code());
                log::warn!(
                    "Reconnect: {e}; last disconnect reason {} rssi {}",
                    LAST_WIFI_REASON.load(Ordering::Relaxed),
                    LAST_WIFI_RSSI.load(Ordering::Relaxed)
                );
            }
        }
        // Internal largest-free-block matters more for TLS than total PSRAM.
        let caps = esp_idf_svc::sys::MALLOC_CAP_INTERNAL | esp_idf_svc::sys::MALLOC_CAP_8BIT;
        // SAFETY: ESP-IDF heap query functions have no pointer arguments.
        unsafe {
            log::info!(
                "internal heap: free={} largest={} minimum={}",
                esp_idf_svc::sys::heap_caps_get_free_size(caps),
                esp_idf_svc::sys::heap_caps_get_largest_free_block(caps),
                esp_idf_svc::sys::heap_caps_get_minimum_free_size(caps)
            );
        }
    }
    Ok(())
}

/// Plain-text failure report on HTTP port 8080 (no TLS, no allocation-heavy
/// server), repeated in the log. Never returns.
fn report_failure(error: &dyn std::error::Error) -> ! {
    use std::io::Write;
    let listener = std::net::TcpListener::bind(("0.0.0.0", 8080));
    loop {
        let (internal, internal_largest, psram, psram_largest) = unsafe {
            use esp_idf_svc::sys::*;
            let internal = MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT;
            (
                heap_caps_get_free_size(internal),
                heap_caps_get_largest_free_block(internal),
                heap_caps_get_free_size(MALLOC_CAP_SPIRAM),
                heap_caps_get_largest_free_block(MALLOC_CAP_SPIRAM),
            )
        };
        let text = format!(
            "NanaCoin {} ({}) startup failed at step {}: {error}\n\
             internal free {internal} largest {internal_largest}; \
             psram free {psram} largest {psram_largest}\n",
            nanacoin::board::ID,
            nanacoin::board::FQDN,
            status_led::current_stage(),
        );
        log::error!("{text}");
        match &listener {
            Ok(listener) => {
                if let Ok((mut stream, _)) = listener.accept() {
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    let _ = write!(
                        stream,
                        "HTTP/1.0 500 Startup failed\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{text}"
                    );
                }
            }
            Err(_) => std::thread::sleep(Duration::from_secs(5)),
        }
    }
}
