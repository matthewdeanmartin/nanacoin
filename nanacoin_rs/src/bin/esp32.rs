#[cfg(not(target_os = "espidf"))]
compile_error!(
    "build firmware with --target xtensa-esp32s3-espidf --no-default-features --features esp32"
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
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    incidents::start()?;
    let peripherals = Peripherals::take()?;
    let event_loop = EspSystemEventLoop::take()?;
    let _wifi_events = event_loop.subscribe::<esp_idf_svc::wifi::WifiEvent, _>(|event| {
        if let esp_idf_svc::wifi::WifiEvent::StaDisconnected(info) = event {
            incidents::record(
                nanacoin::incidents::Kind::WifiDown,
                i32::from(info.reason()),
            );
        }
    })?;
    // Never auto-erase NVS on a version/full error: it may contain data.
    let system_nvs = EspDefaultNvsPartition::take_with(false)?;
    // The svc custom-partition convenience constructor auto-erases on some
    // init errors. Pre-initialize and propagate every error before calling it.
    // IDF initialization is idempotent; its second call sees an initialized
    // partition. Never reach the convenience constructor after a failed init.
    // SAFETY: the partition name is a static NUL-terminated C string.
    esp_idf_svc::sys::esp!(unsafe {
        esp_idf_svc::sys::nvs_flash_init_partition(c"ledger".as_ptr())
    })?;
    let partition = EspNvsPartition::<NvsCustom>::take("ledger")?;
    let mut journal = NvsJournal::open(partition).map_err(|e| format!("journal storage: {e:?}"))?;
    if option_env!("NANACOIN_RECOVER_HTTP") == Some("1") {
        use nanacoin::journal::Journal;
        journal
            .set_https_only(false)
            .map_err(|e| format!("transport recovery: {e:?}"))?;
        log::warn!("USB recovery build: HTTP enabled; reinstall normal firmware next");
    }
    let service = Service::open(journal).map_err(|e| format!("ledger startup: {e:?}"))?;
    // The mutex covers API/domain work, including JSON encoding; all network
    // I/O and TLS handshakes happen outside it.
    let shared = Arc::new(Mutex::new(service));
    let diagnostics = Arc::new(Diagnostics::default());
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, event_loop.clone(), Some(system_nvs))?,
        event_loop,
    )?;
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
                log::warn!("Initial Wi-Fi connection: {error}; retrying");
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
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

    server::start(server::Context {
        shared: Arc::clone(&shared),
        diagnostics: Arc::clone(&diagnostics),
    })?;
    let mut mdns = EspMdns::take()?;
    // The legacy standalone UI board must not advertise this name concurrently.
    mdns.set_hostname("nanacoin")?;
    mdns.set_instance_name("NanaCoin Rust household ledger")?;
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
    // Core 0 owns live probes. Only a small snapshot copy uses the independent
    // diagnostics mutex; ledger work and sockets never hold that mutex.
    let sampler = Arc::clone(&diagnostics);
    // Applies to the next thread spawned on this task, then is restored so it
    // does not leak onto anything spawned later.
    ThreadSpawnConfiguration {
        name: Some(c"nanacoin-diag"),
        pin_to_core: Some(Core::Core0),
        ..Default::default()
    }
    .set()?;
    let _sampler = std::thread::Builder::new()
        .stack_size(4096)
        .spawn(move || sampler.run())?;
    // Independent of HTTP traffic and potentially blocking Wi-Fi reconnection.
    let scheduler = Arc::clone(&shared);
    ThreadSpawnConfiguration {
        name: Some(c"nanacoin-loans"),
        pin_to_core: Some(Core::Core1),
        ..Default::default()
    }
    .set()?;
    let _scheduler = std::thread::Builder::new()
        .stack_size(24 * 1024)
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let mut service = scheduler.lock().unwrap();
            let failed_before = service.storage_failed();
            if let Err(error) = service.tick() {
                log::warn!("Scheduled payments: {error:?}");
            }
            if !failed_before && service.storage_failed() {
                incidents::record(nanacoin::incidents::Kind::StorageFailed, 0);
            }
        })?;
    ThreadSpawnConfiguration::default().set()?;
    // SAFETY: monotonic timer query has no pointer arguments.
    diagnostics.boot_ready_ms.store(
        unsafe { esp_idf_svc::sys::esp_timer_get_time() / 1000 } as u32,
        Ordering::Relaxed,
    );
    incidents::record(nanacoin::incidents::Kind::Ready, 0);
    log::info!(
        "Ready at https://nanacoin.local; bundled UI + API; HTTP/API core 1 (8 TLS + 4 HTTP clients), TLS handshakes/Wi-Fi/diag core 0"
    );
    loop {
        std::thread::sleep(Duration::from_secs(10));
        if !wifi.is_connected()? {
            incidents::record(nanacoin::incidents::Kind::Reconnect, 0);
            log::warn!("Wi-Fi disconnected; reconnecting");
            if let Err(e) = wifi.connect().and_then(|_| wifi.wait_netif_up()) {
                incidents::record(nanacoin::incidents::Kind::ReconnectFailed, e.code());
                log::warn!("Reconnect: {e}");
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
}
