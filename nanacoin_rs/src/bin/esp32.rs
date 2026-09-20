#[cfg(not(target_os = "espidf"))]
compile_error!(
    "build firmware with --target xtensa-esp32s3-espidf --no-default-features --features esp32"
);

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{cpu::Core, peripherals::Peripherals, task::thread::ThreadSpawnConfiguration},
    http::{
        server::{Configuration as HttpConfiguration, EspHttpServer},
        Method,
    },
    io::{Read, Write},
    mdns::EspMdns,
    nvs::{EspDefaultNvsPartition, EspNvsPartition, NvsCustom},
    tls::X509,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use nanacoin::{api, domain::Error, journal::Service};
use std::{
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};

#[path = "esp32/diagnostics.rs"]
mod diagnostics;
use diagnostics::Diagnostics;

// Per-worker response storage.
//
// The handler closure is shared by every httpd worker, so a single buffer
// would have to live behind the service mutex and would keep that mutex held
// for the whole of serialisation and the socket write. One buffer per worker
// thread means the mutex covers only the ledger call itself.
//
// Allocated on first use by each worker and reused for that worker's life.
// PSRAM is what is plentiful here, so this trades 512 KiB per worker for a
// materially shorter critical section.
thread_local! {
    static RESPONSE: std::cell::RefCell<Box<[u8]>> =
        std::cell::RefCell::new(vec![0u8; api::RESPONSE_LIMIT].into_boxed_slice());
}

#[path = "esp32/journal.rs"]
mod nvs_journal;
use nvs_journal::NvsJournal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;
    let event_loop = EspSystemEventLoop::take()?;
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
    // Response storage is per worker (see RESPONSE); the mutex guards only
    // the ledger service, so it is held for the domain call and nothing else.
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
    wifi.connect()?;
    wifi.wait_netif_up()?;
    // Keep the SNTP service alive. Timed offers refuse writes until wall time
    // is valid; persisted deadlines must not restart when the board reboots.
    let mut time_config = esp_idf_svc::sntp::SntpConf::default();
    if let Some(server) = option_env!("NANACOIN_NTP_SERVER") {
        time_config.servers.fill(server);
    }
    let _sntp = esp_idf_svc::sntp::EspSntp::new(&time_config)?;

    let config = HttpConfiguration {
        https_port: 443,
        core: Some(Core::Core1),
        // Release parsing frames reach 7.5 KiB before the HTTP/TLS caller
        // frames. Keep headroom for the full call chain, not just one frame.
        stack_size: 24 * 1024,
        max_open_sockets: 4,
        max_sessions: 4,
        max_uri_handlers: 5,
        uri_match_wildcard: true,
        session_timeout: Duration::from_secs(10),
        server_certificate: Some(X509::pem_until_nul(
            concat!(include_str!("../../certs/nanacoin-ca-signed.crt"), "\0").as_bytes(),
        )),
        private_key: Some(X509::pem_until_nul(
            concat!(include_str!("../../certs/nanacoin-ca-signed.key"), "\0").as_bytes(),
        )),
        ..Default::default()
    };
    let mut server = EspHttpServer::new(&config)?;
    let mut http_server = EspHttpServer::new(&HttpConfiguration {
        http_port: 80,
        ctrl_port: 32769,
        core: Some(Core::Core1),
        stack_size: 24 * 1024,
        max_open_sockets: 2,
        max_sessions: 2,
        max_uri_handlers: 5,
        uri_match_wildcard: true,
        session_timeout: Duration::from_secs(5),
        ..Default::default()
    })?;
    for (server, tls) in [(&mut server, true), (&mut http_server, false)] {
        for (method, method_name) in [
            (Method::Get, "GET"),
            (Method::Head, "HEAD"),
            (Method::Post, "POST"),
            (Method::Patch, "PATCH"),
            (Method::Options, "OPTIONS"),
        ] {
            let shared = Arc::clone(&shared);
            let diagnostics = Arc::clone(&diagnostics);
            server.fn_handler::<esp_idf_svc::io::EspIOError, _>("/*", method, move |mut req| {
                let locked_http = !tls && shared.lock().unwrap().https_only();
                let static_reply = if locked_http {
                    nanacoin::web::onboarding(method_name, req.uri(), true)
                } else {
                    nanacoin::web::respond(
                        method_name,
                        req.uri(),
                        req.header("Accept-Encoding").unwrap_or(""),
                        req.header("If-None-Match").unwrap_or(""),
                    )
                };
                if let Some(reply) = static_reply {
                    // Flash-backed immutable bytes: no ledger lock, no 512 KiB
                    // response allocation, no compression or filesystem at runtime.
                    let mut headers = heapless::Vec::<(&str, &str), 10>::new();
                    headers.extend_from_slice(&reply.headers).unwrap();
                    // esp-idf-svc streams with chunked transfer encoding; do not
                    // advertise a conflicting Content-Length.
                    headers.push(("Connection", "close")).unwrap();
                    let mut response = req.into_response(reply.status, None, &headers)?;
                    if method_name != "HEAD" {
                        for chunk in reply.bytes.chunks(2048) {
                            response.write_all(chunk)?;
                        }
                    }
                    return Ok(());
                }
                let origin = heapless::String::<256>::try_from(req.header("Origin").unwrap_or(""));
                let allowed = origin.as_ref().is_ok_and(|o| {
                    o.as_str() == "https://nanacoin.local"
                        || o.as_str() == "http://nanacoin.local"
                        || api::origin_allowed(
                            o,
                            option_env!("NANACOIN_ORIGINS").unwrap_or(api::DEFAULT_ORIGINS),
                        )
                });
                let origin = origin.unwrap_or_default();
                let mut generation_text = heapless::String::<24>::new();
                let mut headers = heapless::Vec::<(&str, &str), 10>::new();
                for pair in [
                    ("Content-Type", "application/json"),
                    ("Cache-Control", "no-store"),
                    ("Connection", "close"),
                    ("Vary", "Origin"),
                ] {
                    headers.push(pair).unwrap();
                }
                if allowed && !origin.is_empty() {
                    headers
                        .push(("Access-Control-Allow-Origin", origin.as_str()))
                        .unwrap();
                    headers
                        .push(("Access-Control-Allow-Methods", "GET, POST, PATCH, OPTIONS"))
                        .unwrap();
                    headers
                        .push((
                            "Access-Control-Allow-Headers",
                            "Authorization, Content-Type, Idempotency-Key",
                        ))
                        .unwrap();
                    headers
                        .push(("Access-Control-Expose-Headers", "X-Nanacoin-Generation"))
                        .unwrap();
                }
                diagnostics.requests.fetch_add(1, Ordering::Relaxed);
                let path = req.uri().split('?').next().unwrap_or("");
                if allowed
                    && method_name == "GET"
                    && matches!(path, "/api/v1/diag" | "/api/v1/diag/static")
                {
                    // Small request-local buffer, reclaimed on return. In
                    // particular, diagnostics never initialize the 512 KiB
                    // ledger response buffer or hold a lock during socket I/O.
                    let mut output = [0; nanacoin::diagnostics::RESPONSE_BYTES];
                    let (status, len) = if path.ends_with("/static") {
                        nanacoin::diagnostics::response(&diagnostics::system_info(), &mut output)
                    } else {
                        nanacoin::diagnostics::response(&diagnostics.snapshot(), &mut output)
                    };
                    if status >= 400 {
                        diagnostics.errors.fetch_add(1, Ordering::Relaxed);
                    }
                    return req
                        .into_response(status, None, &headers)?
                        .write_all(&output[..len]);
                }
                RESPONSE.with(|cell| {
                    let mut generation = None;
                    let mut borrowed = cell.borrow_mut();
                    let output = &mut **borrowed;
                    let (status, len) = if !allowed {
                        api::error_response(Error::Forbidden, output)
                    } else if method_name == "OPTIONS" {
                        output[..2].copy_from_slice(b"{}");
                        (200, 2)
                    } else {
                        let length = if method_name == "POST" || method_name == "PATCH" {
                            req.header("Content-Length")
                                .and_then(|v| v.parse::<u64>().ok())
                                .unwrap_or(u64::MAX)
                        } else {
                            0
                        };
                        if length > api::BODY_LIMIT as u64 {
                            let (_, len) = api::error_response(Error::Capacity, output);
                            (413, len)
                        } else {
                            let mut body = [0; api::BODY_LIMIT];
                            if req.read_exact(&mut body[..length as usize]).is_err() {
                                api::error_response(Error::InvalidInput, output)
                            } else {
                                let mut service = shared.lock().unwrap();
                                let result = api::handle_keyed_on(
                                    &mut service,
                                    method_name,
                                    req.uri(),
                                    req.header("Authorization").unwrap_or(""),
                                    req.header("Idempotency-Key").unwrap_or(""),
                                    &body[..length as usize],
                                    output,
                                    tls,
                                );
                                generation = Some(service.generation());
                                result
                            }
                        }
                    };
                    if let Some(value) = generation {
                        core::fmt::Write::write_fmt(&mut generation_text, format_args!("{value}"))
                            .unwrap();
                        headers
                            .push(("X-Nanacoin-Generation", generation_text.as_str()))
                            .unwrap();
                    }
                    // Outside the mutex: serialising to the socket is the slow part
                    // and no longer blocks other requests' ledger access.
                    if status >= 400 {
                        diagnostics.errors.fetch_add(1, Ordering::Relaxed);
                    }
                    req.into_response(status, None, &headers)?
                        .write_all(&output[..len])?;
                    Ok(())
                })
            })?;
        }
    }
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
        stack_size: 4096,
        pin_to_core: Some(Core::Core0),
        ..Default::default()
    }
    .set()?;
    let _sampler = std::thread::Builder::new().spawn(move || sampler.run())?;
    ThreadSpawnConfiguration::default().set()?;
    // SAFETY: monotonic timer query has no pointer arguments.
    diagnostics.boot_ready_ms.store(
        unsafe { esp_idf_svc::sys::esp_timer_get_time() / 1000 } as u32,
        Ordering::Relaxed,
    );
    log::info!(
        "Ready at https://nanacoin.local; bundled UI + API; HTTPS/ledger core 1 (4 sockets), Wi-Fi/lwIP/diag core 0"
    );
    loop {
        std::thread::sleep(Duration::from_secs(10));
        if !wifi.is_connected()? {
            log::warn!("Wi-Fi disconnected; reconnecting");
            if let Err(e) = wifi.connect().and_then(|_| wifi.wait_netif_up()) {
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
