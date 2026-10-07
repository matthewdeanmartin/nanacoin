//! Independent recorder sampling; never takes application or Wi-Fi-owner locks.
use esp_idf_svc::{hal::cpu::Core, sys};
use nanacoin::incidents::{Kind, LOG};

pub fn now() -> u64 {
    esp_idf_svc::timer::EspTimerService::new()
        .expect("task timer service")
        .now()
        .as_millis() as u64
}
pub fn record(kind: Kind, code: i32) {
    LOG.record(now(), kind, code, 0);
}

pub fn boot() -> Result<(), Box<dyn std::error::Error>> {
    let mut id = [0; 4];
    getrandom::getrandom(&mut id).map_err(|e| format!("incident boot ID: {e}"))?;
    LOG.boot(
        u32::from_ne_bytes(id),
        now(),
        super::reset_code(esp_idf_svc::hal::reset::ResetReason::get()),
    );
    Ok(())
}

pub fn start(
    station: impl Fn() -> Option<miniframework::sys::StationSample> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    miniframework::esp::spawn_task(
        c"incident-sample",
        8 * 1024,
        false,
        Some(Core::Core0),
        3,
        move || loop {
            let rssi = station().map(|n| n.rssi);
            // SAFETY: local output record; thread-safe IDF queries, no reconfiguration.
            unsafe {
                let caps = sys::MALLOC_CAP_INTERNAL | sys::MALLOC_CAP_8BIT;
                LOG.sample(
                    now(),
                    sys::heap_caps_get_free_size(caps) as u32,
                    sys::heap_caps_get_largest_free_block(caps) as u32,
                    rssi,
                );
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        },
    )?;
    Ok(())
}
