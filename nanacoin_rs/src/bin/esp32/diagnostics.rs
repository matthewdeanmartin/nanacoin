//! Only the core-0 sampler owns the sensor. The lock protects a <=256-byte
//! copy, never a probe, JSON formatting, ledger operation, or socket write.
use esp_idf_svc::sys;
use nanacoin::diagnostics::{Heap, Partition, Snapshot, Storage, SystemInfo};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

pub struct Diagnostics {
    snapshot: Mutex<Snapshot>,
    pub requests: AtomicU32,
    pub errors: AtomicU32,
    pub boot_ready_ms: AtomicU32,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(Snapshot {
                schema: 1,
                sampler_core: 0,
                http_core: 1,
                ..Snapshot::default()
            }),
            requests: AtomicU32::new(0),
            errors: AtomicU32::new(0),
            boot_ready_ms: AtomicU32::new(0),
        }
    }
}

const _: () = assert!(core::mem::size_of::<Diagnostics>() <= 288);

impl Diagnostics {
    pub fn snapshot(&self) -> Snapshot {
        let mut result = *self.snapshot.lock().unwrap();
        result.requests = self.requests.load(Ordering::Relaxed);
        result.errors = self.errors.load(Ordering::Relaxed);
        result.boot_ready_ms = self.boot_ready_ms.load(Ordering::Relaxed) as u64;
        result
    }

    pub fn run(&self) {
        let sensor = Temperature::new();
        let mut snapshot = Snapshot {
            schema: 1,
            sampler_core: 0,
            http_core: 1,
            ..Snapshot::default()
        };
        loop {
            // SAFETY: IDF's thread-safe query APIs, with valid local output
            // structures. No handle crosses threads; no peripheral is reconfigured.
            unsafe {
                snapshot.sampled_at_ms = (sys::esp_timer_get_time() / 1000) as u64;
                snapshot.uptime_seconds = snapshot.sampled_at_ms / 1000;
                snapshot.internal = heap(sys::MALLOC_CAP_INTERNAL | sys::MALLOC_CAP_8BIT);
                snapshot.psram = heap(sys::MALLOC_CAP_SPIRAM);
                snapshot.free_heap = snapshot.internal.free;
                snapshot.largest_free_block = snapshot.internal.largest;
                snapshot.minimum_free_heap = snapshot.internal.minimum;
                snapshot.psram_free = snapshot.psram.free;
                snapshot.temperature_c = sensor.read();
                let mut ap = sys::wifi_ap_record_t::default();
                let connected = sys::esp_wifi_sta_get_ap_info(&mut ap) == 0;
                snapshot.rssi_dbm = connected.then_some(ap.rssi);
                snapshot.wifi_channel = connected.then_some(ap.primary);
                let netif = sys::esp_netif_get_handle_from_ifkey(c"WIFI_STA_DEF".as_ptr());
                let mut ip = sys::esp_netif_ip_info_t::default();
                let has_ip = connected
                    && !netif.is_null()
                    && sys::esp_netif_get_ip_info(netif, &mut ip) == 0;
                snapshot.ip = has_ip.then_some(ip.ip.addr.to_ne_bytes());
                snapshot.gateway = has_ip.then_some(ip.gw.addr.to_ne_bytes());
                snapshot.netmask = has_ip.then_some(ip.netmask.addr.to_ne_bytes());
                snapshot.tasks = sys::uxTaskGetNumberOfTasks();
                snapshot.sampler_stack_free_min_bytes =
                    sys::uxTaskGetStackHighWaterMark(std::ptr::null_mut());
                // NVS accounting can take its own IDF lock; only do it every
                // 30s, outside our snapshot lock. It never reads journal values.
                if snapshot.samples % 15 == 0 {
                    let mut stats = sys::nvs_stats_t::default();
                    snapshot.ledger_storage = (sys::nvs_get_stats(c"ledger".as_ptr(), &mut stats)
                        == 0)
                        .then_some(Storage {
                            used_entries: stats.used_entries as u32,
                            free_entries: stats.free_entries as u32,
                            available_entries: stats.available_entries as u32,
                            total_entries: stats.total_entries as u32,
                        });
                    snapshot.storage_sampled_at_ms = snapshot.sampled_at_ms;
                }
            }
            snapshot.unix_seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
                .filter(|s| *s >= 1_700_000_000);
            snapshot.samples = snapshot.samples.wrapping_add(1);
            *self.snapshot.lock().unwrap() = snapshot;
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
}

fn heap(caps: u32) -> Heap {
    let mut info = sys::multi_heap_info_t::default();
    // SAFETY: initialized output; IDF serializes allocator metadata access.
    unsafe { sys::heap_caps_get_info(&mut info, caps) };
    Heap {
        total: (info.total_free_bytes + info.total_allocated_bytes) as u32,
        free: info.total_free_bytes as u32,
        largest: info.largest_free_block as u32,
        minimum: info.minimum_free_bytes as u32,
        allocated_blocks: info.allocated_blocks as u32,
        free_blocks: info.free_blocks as u32,
    }
}

struct Temperature(sys::temperature_sensor_handle_t);
impl Temperature {
    fn new() -> Self {
        let mut handle = std::ptr::null_mut();
        let config = sys::temperature_sensor_config_t {
            range_min: 10,
            range_max: 80,
            ..Default::default()
        };
        // SAFETY: handle stays on the sampler thread for its lifetime.
        unsafe {
            if sys::temperature_sensor_install(&config, &mut handle) != 0 {
                return Self(std::ptr::null_mut());
            }
            if sys::temperature_sensor_enable(handle) != 0 {
                sys::temperature_sensor_uninstall(handle);
                return Self(std::ptr::null_mut());
            }
        }
        Self(handle)
    }
    fn read(&self) -> Option<f32> {
        let mut value = 0.0;
        // SAFETY: only the owning task accesses this enabled handle.
        (!self.0.is_null()
            && unsafe { sys::temperature_sensor_get_celsius(self.0, &mut value) } == 0
            && value.is_finite())
        .then_some(value)
    }
}
impl Drop for Temperature {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the sole owner is releasing its handle.
            unsafe {
                sys::temperature_sensor_disable(self.0);
                sys::temperature_sensor_uninstall(self.0);
            }
        }
    }
}

fn fixed_string<const N: usize>(bytes: &[u8]) -> heapless::String<N> {
    let mut result = heapless::String::new();
    for &byte in bytes.iter().take_while(|b| **b != 0) {
        // IDF identifiers are ASCII. Replace unexpected bytes rather than
        // trusting an unterminated C string or returning invalid JSON.
        let _ = result.push(if byte.is_ascii() {
            byte as u8 as char
        } else {
            '?'
        });
    }
    result
}

pub fn system_info() -> SystemInfo {
    let mut result = SystemInfo {
        platform: "ESP32-S3 / Rust",
        firmware: env!("CARGO_PKG_VERSION"),
        snapshot_bytes: core::mem::size_of::<Diagnostics>(),
        response_bytes: nanacoin::diagnostics::RESPONSE_BYTES,
        ..SystemInfo::default()
    };
    // SAFETY: chip/flash queries have local outputs, app description is static,
    // partition pointers are consumed before advancing their iterator. The
    // iterator is freed by next at EOF or explicitly on bounded truncation.
    unsafe {
        let mut chip = sys::esp_chip_info_t::default();
        sys::esp_chip_info(&mut chip);
        result.chip_model = chip.model;
        result.chip_revision = chip.revision;
        result.cores = chip.cores;
        result.cpu_mhz = (sys::esp_clk_cpu_freq() / 1_000_000) as u32;
        result.reset_reason = sys::esp_reset_reason();
        let mut size = 0;
        result.flash_bytes =
            (sys::esp_flash_get_size(std::ptr::null_mut(), &mut size) == 0).then_some(size);
        let app = sys::esp_app_get_description();
        if !app.is_null() {
            result.idf = fixed_string(&(*app).idf_ver);
        }
        let mut iterator = sys::esp_partition_find(
            sys::esp_partition_type_t_ESP_PARTITION_TYPE_ANY,
            sys::esp_partition_subtype_t_ESP_PARTITION_SUBTYPE_ANY,
            std::ptr::null(),
        );
        while !iterator.is_null() {
            if result.partitions.is_full() {
                result.partitions_truncated = true;
                sys::esp_partition_iterator_release(iterator);
                break;
            }
            let p = &*sys::esp_partition_get(iterator);
            let _ = result.partitions.push(Partition {
                name: fixed_string(&p.label),
                kind: p.type_,
                subtype: p.subtype,
                offset: p.address,
                size: p.size,
                encrypted: p.encrypted,
            });
            iterator = sys::esp_partition_next(iterator);
        }
    }
    result
}
