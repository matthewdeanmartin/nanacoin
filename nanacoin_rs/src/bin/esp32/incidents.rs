//! Independent recorder sampling; never takes application or Wi-Fi-owner locks.
use esp_idf_svc::{
    hal::{cpu::Core, task::thread::ThreadSpawnConfiguration},
    sys,
};
use nanacoin::incidents::{Kind, LOG};

pub fn now() -> u64 {
    // SAFETY: monotonic clock query has no pointer arguments.
    unsafe { (sys::esp_timer_get_time() / 1000) as u64 }
}
pub fn record(kind: Kind, code: i32) {
    LOG.record(now(), kind, code, 0);
}

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: IDF is initialized, queries have no output pointers.
    unsafe {
        LOG.boot(sys::esp_random(), now(), sys::esp_reset_reason() as i32);
    }
    ThreadSpawnConfiguration {
        name: Some(c"incident-sample"),
        priority: 3,
        pin_to_core: Some(Core::Core0),
        ..Default::default()
    }
    .set()?;
    std::thread::Builder::new()
        .stack_size(8 * 1024)
        .spawn(|| loop {
            // SAFETY: local output record; thread-safe IDF queries, no reconfiguration.
            unsafe {
                let mut ap = sys::wifi_ap_record_t::default();
                let rssi =
                    (sys::esp_wifi_sta_get_ap_info(&mut ap) == sys::ESP_OK).then_some(ap.rssi);
                let caps = sys::MALLOC_CAP_INTERNAL | sys::MALLOC_CAP_8BIT;
                LOG.sample(
                    now(),
                    sys::heap_caps_get_free_size(caps) as u32,
                    sys::heap_caps_get_largest_free_block(caps) as u32,
                    rssi,
                );
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        })?;
    ThreadSpawnConfiguration::default().set()?;
    Ok(())
}
