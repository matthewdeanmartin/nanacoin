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
                http_core: if cfg!(feature = "board-s2")
                    || (cfg!(feature = "cobol-core") && !cfg!(feature = "board-p4"))
                {
                    0
                } else {
                    1
                },
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

    pub fn run(
        &self,
        temperature: impl Fn() -> Option<f32>,
        station: impl Fn() -> Option<miniframework::sys::StationSample>,
    ) {
        let mut snapshot = Snapshot {
            schema: 1,
            sampler_core: 0,
            http_core: if cfg!(feature = "board-s2")
                || (cfg!(feature = "cobol-core") && !cfg!(feature = "board-p4"))
            {
                0
            } else {
                1
            },
            ..Snapshot::default()
        };
        loop {
            snapshot.sampled_at_ms = super::incidents::now();
            snapshot.uptime_seconds = snapshot.sampled_at_ms / 1000;
            snapshot.internal = heap(sys::MALLOC_CAP_INTERNAL | sys::MALLOC_CAP_8BIT);
            snapshot.psram = heap(sys::MALLOC_CAP_SPIRAM);
            snapshot.free_heap = snapshot.internal.free;
            snapshot.largest_free_block = snapshot.internal.largest;
            snapshot.minimum_free_heap = snapshot.internal.minimum;
            snapshot.psram_free = snapshot.psram.free;
            snapshot.temperature_c = temperature();
            let network = station();
            snapshot.rssi_dbm = network.map(|n| n.rssi);
            snapshot.wifi_channel = network.map(|n| n.channel);
            snapshot.ip = network.and_then(|n| n.ip);
            snapshot.gateway = network.and_then(|n| n.gateway);
            snapshot.netmask = network.and_then(|n| n.netmask);
            // SAFETY: IDF's thread-safe query APIs, with valid local output
            // structures. No handle crosses threads; no peripheral is reconfigured.
            unsafe {
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
        platform: nanacoin::board::PLATFORM,
        board: nanacoin::board::ID,
        hostname: nanacoin::board::FQDN,
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
        result.reset_reason = super::reset_code(esp_idf_svc::hal::reset::ResetReason::get()) as _;
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
