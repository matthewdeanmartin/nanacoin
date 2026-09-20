//! Bounded diagnostics wire format. No ledger references and no heap allocation.
use serde::Serialize;

pub const RESPONSE_BYTES: usize = 4096;

#[derive(Clone, Copy, Default, Serialize)]
pub struct Heap {
    pub total: u32,
    pub free: u32,
    pub largest: u32,
    pub minimum: u32,
    pub allocated_blocks: u32,
    pub free_blocks: u32,
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct Storage {
    pub used_entries: u32,
    pub free_entries: u32,
    pub available_entries: u32,
    pub total_entries: u32,
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct Snapshot {
    pub schema: u8,
    pub uptime_seconds: u64,
    pub sampled_at_ms: u64,
    pub samples: u32,
    // Retain the original /diag fields for soak scripts and older clients.
    pub free_heap: u32,
    pub largest_free_block: u32,
    pub minimum_free_heap: u32,
    pub psram_free: u32,
    pub sampler_core: u8,
    pub http_core: u8,
    pub internal: Heap,
    pub psram: Heap,
    pub temperature_c: Option<f32>,
    pub rssi_dbm: Option<i8>,
    pub wifi_channel: Option<u8>,
    pub ip: Option<[u8; 4]>,
    pub gateway: Option<[u8; 4]>,
    pub netmask: Option<[u8; 4]>,
    pub unix_seconds: Option<u64>,
    pub tasks: u32,
    pub sampler_stack_free_min_bytes: u32,
    pub ledger_storage: Option<Storage>,
    pub storage_sampled_at_ms: u64,
    pub boot_ready_ms: u64,
    pub requests: u32,
    pub errors: u32,
}

#[derive(Default, Serialize)]
pub struct Partition {
    pub name: heapless::String<17>,
    pub kind: u32,
    pub subtype: u32,
    pub offset: u32,
    pub size: u32,
    pub encrypted: bool,
}

#[derive(Default, Serialize)]
pub struct SystemInfo {
    pub platform: &'static str,
    pub firmware: &'static str,
    pub idf: heapless::String<64>,
    pub chip_model: u32,
    pub chip_revision: u16,
    pub cores: u8,
    pub cpu_mhz: u32,
    pub reset_reason: u32,
    pub flash_bytes: Option<u32>,
    pub partitions: heapless::Vec<Partition, 16>,
    pub partitions_truncated: bool,
    pub snapshot_bytes: usize,
    pub response_bytes: usize,
}

/// Fail closed on overflow; never emit truncated JSON with HTTP 200.
pub fn response(value: &impl Serialize, out: &mut [u8]) -> (u16, usize) {
    match serde_json_core::to_slice(value, out) {
        Ok(len) => (200, len),
        Err(_) => crate::api::error_response(crate::domain::Error::Capacity, out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_snapshot_and_wire_format() {
        assert!(core::mem::size_of::<Snapshot>() <= 256);
        let mut out = [0; RESPONSE_BYTES];
        let snapshot = Snapshot {
            schema: 1,
            samples: u32::MAX,
            uptime_seconds: u64::MAX,
            temperature_c: Some(85.5),
            ..Snapshot::default()
        };
        let (status, len) = response(&snapshot, &mut out);
        assert_eq!(status, 200);
        let json: serde_json::Value = serde_json::from_slice(&out[..len]).unwrap();
        assert_eq!(json["samples"], u32::MAX);
        assert!(json["rssi_dbm"].is_null());
        assert_eq!(json["temperature_c"], 85.5);
        let mut info = SystemInfo::default();
        info.idf.push_str(&"x".repeat(64)).unwrap();
        for _ in 0..16 {
            info.partitions
                .push(Partition {
                    name: "12345678901234567".try_into().unwrap(),
                    kind: u32::MAX,
                    subtype: u32::MAX,
                    offset: u32::MAX,
                    size: u32::MAX,
                    encrypted: true,
                })
                .ok()
                .unwrap();
        }
        assert_eq!(response(&info, &mut out).0, 200);
    }

    #[test]
    fn overflow_is_an_error() {
        let mut out = [0; 128];
        assert_ne!(response(&Snapshot::default(), &mut out).0, 200);
    }
}
