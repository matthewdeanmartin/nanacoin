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
    /// Board profile (`s3` or `s2`) and the hostname its certificate names.
    pub board: &'static str,
    pub hostname: &'static str,
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

/// Writes into a byte slice; fails (instead of truncating) when full.
struct SliceWriter<'a> {
    out: &'a mut [u8],
    len: usize,
}

impl core::fmt::Write for SliceWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        if end > self.out.len() {
            return Err(core::fmt::Error);
        }
        self.out[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// The snapshot as one InfluxDB line for `GET /metrics`, scraped by
/// housemetrics. Measurement and field names match miniframework boards
/// (`board,app=...,host=... heap_internal_free=...`) so every board in the
/// house graphs side by side. No allocation; fails closed on overflow.
pub fn influx(snapshot: &Snapshot, bank: &str, host: &str, out: &mut [u8]) -> (u16, usize) {
    use core::fmt::Write;
    let mut w = SliceWriter { out, len: 0 };
    let s = snapshot;
    let mut line = || -> core::fmt::Result {
        write!(w, "board,app=nanacoin,bank={bank},host={host} ")?;
        write!(
            w,
            "uptime_s={},heap_internal_free={},heap_internal_min={},heap_internal_largest={},",
            s.uptime_seconds, s.internal.free, s.internal.minimum, s.internal.largest
        )?;
        write!(
            w,
            "heap_psram_free={},heap_psram_min={},heap_psram_largest={},tasks={},",
            s.psram.free, s.psram.minimum, s.psram.largest, s.tasks
        )?;
        write!(
            w,
            "sampler_stack_free_min={},requests={},errors={}",
            s.sampler_stack_free_min_bytes, s.requests, s.errors
        )?;
        if let Some(t) = s.temperature_c {
            write!(w, ",temp_c={t}")?;
        }
        if let Some(rssi) = s.rssi_dbm {
            write!(w, ",rssi={rssi}")?;
        }
        if let Some(nvs) = s.ledger_storage {
            write!(
                w,
                ",nvs_used_entries={},nvs_free_entries={}",
                nvs.used_entries, nvs.free_entries
            )?;
        }
        if s.boot_ready_ms > 0 {
            write!(w, ",boot_ready_ms={}", s.boot_ready_ms)?;
        }
        match s.unix_seconds {
            // Nanosecond timestamps (InfluxDB's default precision).
            Some(t) => writeln!(w, " {t}000000000"),
            None => writeln!(w),
        }
    };
    match line() {
        Ok(()) => (200, w.len),
        Err(_) => (507, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn influx_line_for_housemetrics() {
        let mut out = [0; RESPONSE_BYTES];
        let snapshot = Snapshot {
            uptime_seconds: 42,
            internal: Heap {
                free: 50_000,
                minimum: 40_000,
                largest: 30_000,
                ..Heap::default()
            },
            psram: Heap {
                free: 1_000_000,
                ..Heap::default()
            },
            temperature_c: Some(41.5),
            rssi_dbm: Some(-70),
            unix_seconds: Some(1_790_000_000),
            requests: 7,
            ..Snapshot::default()
        };
        let (status, len) = influx(&snapshot, "s2", "nanacoin-s2.local", &mut out);
        assert_eq!(status, 200);
        let text = std::str::from_utf8(&out[..len]).unwrap();
        assert!(text.starts_with("board,app=nanacoin,bank=s2,host=nanacoin-s2.local uptime_s=42,heap_internal_free=50000,"));
        assert!(text.contains(",temp_c=41.5,rssi=-70"));
        assert!(text.ends_with(" 1790000000000000000\n"));
        assert!(
            !text.contains("nvs_"),
            "no NVS fields before the first storage sample"
        );
        // Without a clock, no timestamp (the scraper uses its own).
        let (_, len) = influx(&Snapshot::default(), "s3", "nanacoin.local", &mut out);
        assert!(std::str::from_utf8(&out[..len])
            .unwrap()
            .ends_with("errors=0\n"));
        assert_eq!(influx(&snapshot, "s2", "h", &mut out[..40]).0, 507);
    }

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
