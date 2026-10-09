//! Bounded, RAM-only incident history. Writers never wait, allocate or format.
//! Keep this implementation in step with NanaCoin's incidents.rs.
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU32, Ordering::Relaxed},
    Mutex,
};

pub static LOG: Recorder = Recorder::new();
const EVENTS: usize = 48;
const SAMPLES: usize = 32;
const KINDS: usize = 20;
pub const STALL_MS: u32 = 2_000;
pub const RESPONSE_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Kind {
    #[default]
    Boot,
    Ready,
    WifiDown,
    WifiUp,
    Reconnect,
    ReconnectFailed,
    TlsInitFailed,
    TlsFailed,
    TlsTimeout,
    HandoffFull,
    AdmissionRejected,
    SocketError,
    RequestTimeout,
    SlowRequest,
    AllocationFailed,
    StorageFailed,
    WorkerStalled,
    WorkerRecovered,
    InvalidRequest,
    IdleExpired,
}
const ALL: [Kind; KINDS] = [
    Kind::Boot,
    Kind::Ready,
    Kind::WifiDown,
    Kind::WifiUp,
    Kind::Reconnect,
    Kind::ReconnectFailed,
    Kind::TlsInitFailed,
    Kind::TlsFailed,
    Kind::TlsTimeout,
    Kind::HandoffFull,
    Kind::AdmissionRejected,
    Kind::SocketError,
    Kind::RequestTimeout,
    Kind::SlowRequest,
    Kind::AllocationFailed,
    Kind::StorageFailed,
    Kind::WorkerStalled,
    Kind::WorkerRecovered,
    Kind::InvalidRequest,
    Kind::IdleExpired,
];

#[derive(Clone, Copy, Default, Serialize)]
pub struct Event {
    pub first_ms: u64,
    pub last_ms: u64,
    pub count: u32,
    pub duration_ms: u32,
    pub code: i32,
    pub kind: Kind,
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct Sample {
    pub at_ms: u64,
    pub free_heap: u32,
    pub largest_block: u32,
    pub tls_gap_ms: u32,
    pub http_gap_ms: u32,
    /// None means the station is disconnected (or unavailable).
    pub rssi: Option<i8>,
    pub pending_tls: u8,
    pub tls_clients: u8,
    pub http_clients: u8,
}

#[derive(Clone, Copy)]
struct State {
    events: [Event; EVENTS],
    samples: [Sample; SAMPLES],
    event_next: usize,
    sample_next: usize,
    event_len: usize,
    sample_len: usize,
    overwritten: u32,
    stalled: [bool; 2],
}

pub struct Recorder {
    state: Mutex<State>,
    counts: [AtomicU32; KINDS],
    dropped: AtomicU32,
    boot_id: AtomicU32,
    beats: [AtomicU32; 2],
    active: [AtomicU32; 2],
    connections: [AtomicU32; 3],
    high_water: [AtomicU32; 3],
    last_handshake_ms: AtomicU32,
    max_handshake_ms: AtomicU32,
}

const _: () = assert!(std::mem::size_of::<Recorder>() < 4096);
const _: () = assert!(std::mem::size_of::<Event>() <= 40);
const _: () = assert!(std::mem::size_of::<Sample>() <= 32);

#[derive(Serialize)]
pub struct Snapshot {
    pub boot_id: u32,
    pub retained_bytes: usize,
    pub volatile: bool,
    pub dropped: u32,
    pub overwritten: u32,
    pub events: Vec<Event>,
    pub samples: Vec<Sample>,
    pub counters: Vec<Counter>,
    /// pending TLS, established TLS, established HTTP.
    pub high_water: [u32; 3],
    pub last_handshake_ms: u32,
    pub max_handshake_ms: u32,
}
#[derive(Serialize)]
pub struct Counter {
    pub kind: Kind,
    pub count: u32,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}
impl Recorder {
    pub const fn new() -> Self {
        const EVENT: Event = Event {
            first_ms: 0,
            last_ms: 0,
            count: 0,
            duration_ms: 0,
            code: 0,
            kind: Kind::Boot,
        };
        const SAMPLE: Sample = Sample {
            at_ms: 0,
            free_heap: 0,
            largest_block: 0,
            tls_gap_ms: 0,
            http_gap_ms: 0,
            rssi: None,
            pending_tls: 0,
            tls_clients: 0,
            http_clients: 0,
        };
        Self {
            state: Mutex::new(State {
                events: [EVENT; EVENTS],
                samples: [SAMPLE; SAMPLES],
                event_next: 0,
                sample_next: 0,
                event_len: 0,
                sample_len: 0,
                overwritten: 0,
                stalled: [false; 2],
            }),
            counts: [const { AtomicU32::new(0) }; KINDS],
            dropped: AtomicU32::new(0),
            boot_id: AtomicU32::new(0),
            beats: [const { AtomicU32::new(0) }; 2],
            active: [const { AtomicU32::new(0) }; 2],
            connections: [const { AtomicU32::new(0) }; 3],
            high_water: [const { AtomicU32::new(0) }; 3],
            last_handshake_ms: AtomicU32::new(0),
            max_handshake_ms: AtomicU32::new(0),
        }
    }
    pub fn boot(&self, id: u32, now: u64, reset_reason: i32) {
        // ESP-IDF lazily allocates native mutex backing. Initialize it here,
        // before workers/subscriptions start, never on an error-recording path.
        drop(self.state.lock().unwrap_or_else(|e| e.into_inner()));
        self.boot_id.store(id, Relaxed);
        self.record(now, Kind::Boot, reset_reason, 0);
    }
    pub fn record(&self, now: u64, kind: Kind, code: i32, duration_ms: u32) {
        self.counts[kind as usize].fetch_add(1, Relaxed);
        if kind == Kind::IdleExpired {
            return;
        }
        let Ok(mut state) = self.state.try_lock() else {
            self.dropped.fetch_add(1, Relaxed);
            return;
        };
        // Coalesce by kind/code even with interleaved events. Keep first/latest
        // and the worst duration; a noisy fault cannot allocate more memory.
        for event in &mut state.events {
            if event.count > 0
                && event.kind == kind
                && event.code == code
                && now.saturating_sub(event.last_ms) <= 5_000
            {
                event.last_ms = now;
                event.count = event.count.saturating_add(1);
                event.duration_ms = event.duration_ms.max(duration_ms);
                return;
            }
        }
        let at = state.event_next;
        state.events[at] = Event {
            first_ms: now,
            last_ms: now,
            count: 1,
            duration_ms,
            code,
            kind,
        };
        state.event_next = (at + 1) % EVENTS;
        if state.event_len == EVENTS {
            state.overwritten = state.overwritten.saturating_add(1);
        }
        state.event_len = (state.event_len + 1).min(EVENTS);
    }
    /// Worker 0 = TLS; worker 1 = HTTP. Called even when idle.
    pub fn beat(&self, worker: usize, now: u64) {
        // One writer per worker/connection slot. Simple atomic loads/stores
        // also avoid Xtensa codegen issues with atomic max/swap under LTO.
        let old = self.beats[worker].load(Relaxed);
        self.beats[worker].store(now as u32, Relaxed);
        let active = self.active[worker].load(Relaxed);
        self.active[worker].store(1, Relaxed);
        let gap = (now as u32).wrapping_sub(old);
        if active != 0 && gap >= STALL_MS {
            self.record(now, Kind::WorkerRecovered, worker as i32, gap);
        }
    }
    pub fn connections(&self, slot: usize, count: usize) {
        let count = count.min(u8::MAX as usize) as u32;
        self.connections[slot].store(count, Relaxed);
        self.high_water[slot].store(self.high_water[slot].load(Relaxed).max(count), Relaxed);
    }
    pub fn workers_healthy(&self, now: u64) -> bool {
        (0..2).all(|i| {
            if self.active[i].load(Relaxed) == 0 {
                return false;
            }
            let gap = (now as u32).wrapping_sub(self.beats[i].load(Relaxed));
            gap < STALL_MS || gap > i32::MAX as u32
        })
    }

    /// Records one of miniframework's transport events (installed as its
    /// observer by the firmware). Turns are heartbeats: worker 0 is the TLS
    /// handshake task, worker 1 the connection multiplexer.
    pub fn apply(&self, now: u64, event: &miniframework::events::Event) {
        use miniframework::events::{Event as E, Task};
        match *event {
            E::Turn {
                task: Task::Handshake,
                tls,
                ..
            } => {
                self.beat(0, now);
                self.connections(0, tls.into());
            }
            E::Turn {
                task: Task::Serve,
                tls,
                http,
            } => {
                self.beat(1, now);
                self.connections(1, tls.into());
                self.connections(2, http.into());
            }
            E::Handshake { ms } => self.handshake(ms),
            E::TlsInitFailed { code } => self.record(now, Kind::TlsInitFailed, code, 0),
            E::TlsFailed { code, ms } => self.record(now, Kind::TlsFailed, code, ms),
            E::TlsTimeout { ms } => self.record(now, Kind::TlsTimeout, 0, ms),
            E::HandoffFull => self.record(now, Kind::HandoffFull, 0, 0),
            E::AdmissionRejected { secure } => {
                self.record(now, Kind::AdmissionRejected, i32::from(secure), 0)
            }
            E::SocketError { code } => self.record(now, Kind::SocketError, code, 0),
            E::RequestTimeout { ms } => self.record(now, Kind::RequestTimeout, 0, ms),
            // Code 1 tells a stalled reader from a slow sender, as before.
            E::WriteStalled { ms } => self.record(now, Kind::RequestTimeout, 1, ms),
            E::SlowRequest { status, ms } => {
                self.record(now, Kind::SlowRequest, i32::from(status), ms)
            }
            E::InvalidRequest { status } => {
                self.record(now, Kind::InvalidRequest, i32::from(status), 0)
            }
            E::IdleExpired => self.record(now, Kind::IdleExpired, 0, 0),
            E::AllocationFailed => self.record(now, Kind::AllocationFailed, 1, 0),
            E::WifiDown { reason, .. } => self.record(now, Kind::WifiDown, i32::from(reason), 0),
            E::WifiUp => self.record(now, Kind::WifiUp, 0, 0),
            E::Reconnect => self.record(now, Kind::Reconnect, 0, 0),
            E::ReconnectFailed { code } => self.record(now, Kind::ReconnectFailed, code, 0),
            // Newer framework events; miniframework's own history at
            // /.well-known/incidents keeps them with their own kinds.
            E::TlsConnectionFailed { code } => self.record(now, Kind::SocketError, code, 0),
            E::PeerReset => {}
        }
    }

    pub fn count(&self, kind: Kind) -> u32 {
        self.counts[kind as usize].load(Relaxed)
    }
    pub fn handshake(&self, ms: u32) {
        self.last_handshake_ms.store(ms, Relaxed);
        self.max_handshake_ms
            .store(self.max_handshake_ms.load(Relaxed).max(ms), Relaxed);
    }
    /// Called by an independent sampler without any application/network lock.
    /// Throttled to five seconds even if the caller samples more frequently.
    pub fn sample(&self, now: u64, free_heap: u32, largest_block: u32, rssi: Option<i8>) {
        let gaps: [u32; 2] = std::array::from_fn(|i| {
            if self.active[i].load(Relaxed) == 0 {
                0
            } else {
                let gap = (now as u32).wrapping_sub(self.beats[i].load(Relaxed));
                // A worker can beat after the sampler captured `now`.
                if gap > i32::MAX as u32 {
                    0
                } else {
                    gap
                }
            }
        });
        let Ok(mut state) = self.state.try_lock() else {
            self.dropped.fetch_add(1, Relaxed);
            return;
        };
        if state.sample_len > 0
            && now.saturating_sub(state.samples[(state.sample_next + SAMPLES - 1) % SAMPLES].at_ms)
                < 5_000
        {
            return;
        }
        // RSSI can be unavailable while the station lock is busy. Connectivity
        // changes come from framework Wi-Fi events, never missing telemetry.
        let stalled = gaps.map(|gap| gap >= STALL_MS);
        let new_stalls: [bool; 2] = std::array::from_fn(|i| stalled[i] && !state.stalled[i]);
        state.stalled = stalled;
        let at = state.sample_next;
        state.samples[at] = Sample {
            at_ms: now,
            free_heap,
            largest_block,
            tls_gap_ms: gaps[0],
            http_gap_ms: gaps[1],
            rssi,
            pending_tls: self.connections[0].load(Relaxed) as u8,
            tls_clients: self.connections[1].load(Relaxed) as u8,
            http_clients: self.connections[2].load(Relaxed) as u8,
        };
        state.sample_next = (at + 1) % SAMPLES;
        state.sample_len = (state.sample_len + 1).min(SAMPLES);
        drop(state);
        for i in 0..2 {
            if new_stalls[i] {
                self.record(now, Kind::WorkerStalled, i as i32, gaps[i]);
            }
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        // Copy only under the lock. Allocation and serialization happen later.
        let state = *self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut events: Vec<_> = state
            .events
            .iter()
            .filter(|e| e.count > 0)
            .copied()
            .collect();
        events.sort_by_key(|e| e.last_ms);
        let samples = (0..state.sample_len)
            .map(|i| state.samples[(state.sample_next + SAMPLES - state.sample_len + i) % SAMPLES])
            .collect();
        Snapshot {
            boot_id: self.boot_id.load(Relaxed),
            retained_bytes: std::mem::size_of::<Self>(),
            volatile: true,
            dropped: self.dropped.load(Relaxed),
            overwritten: state.overwritten,
            events,
            samples,
            counters: ALL
                .iter()
                .map(|&kind| Counter {
                    kind,
                    count: self.counts[kind as usize].load(Relaxed),
                })
                .collect(),
            high_water: std::array::from_fn(|i| self.high_water[i].load(Relaxed)),
            last_handshake_ms: self.last_handshake_ms.load(Relaxed),
            max_handshake_ms: self.max_handshake_ms.load(Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flood_is_bounded_and_coalesced() {
        let log = Recorder::new();
        for i in 0..10_000 {
            log.record(i, Kind::TlsFailed, -42, i as u32);
        }
        let snapshot = log.snapshot();
        assert!(snapshot.retained_bytes < 4096);
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.events[0].count, 10_000);
        for i in 0..60 {
            log.record(20_000 + i * 6_000, Kind::TlsFailed, i as i32, 0);
        }
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 48);
        assert_eq!(snapshot.overwritten, 13);
    }
    #[test]
    fn contention_never_waits_and_counts_loss() {
        let log = Recorder::new();
        let _held = log.state.lock().unwrap();
        log.record(1, Kind::TlsTimeout, 0, 0);
        log.sample(2, 100, 50, None);
        assert_eq!(log.dropped.load(Relaxed), 2);
        assert_eq!(log.counts[Kind::TlsTimeout as usize].load(Relaxed), 1);
    }
    #[test]
    fn history_survives_recovery_and_wraps_in_order() {
        let log = Recorder::new();
        log.boot(123, 0, 7);
        log.beat(0, 1);
        log.beat(1, 1);
        log.sample(5_001, 100, 40, Some(-60));
        log.beat(1, 6_001);
        assert!(log
            .snapshot()
            .events
            .iter()
            .any(|e| e.kind == Kind::WorkerRecovered && e.duration_ms == 6_000));
        for i in 2..40 {
            log.sample(i * 5_000, 90, 30, None);
        }
        let snapshot = log.snapshot();
        assert_eq!(snapshot.samples.len(), 32);
        assert!(snapshot.samples.windows(2).all(|w| w[0].at_ms < w[1].at_ms));
        assert_eq!(snapshot.boot_id, 123);
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        assert!(bytes.len() < 32_768);
    }
    #[test]
    fn heartbeat_handles_u32_millisecond_wrap() {
        let log = Recorder::new();
        log.beat(1, u32::MAX as u64 - 100);
        log.beat(1, u32::MAX as u64 + 50);
        assert!(log.snapshot().events.is_empty());
    }

    #[test]
    fn status_requires_both_workers_to_make_progress() {
        let log = Recorder::new();
        assert!(!log.workers_healthy(0));
        log.beat(0, 100);
        assert!(!log.workers_healthy(100));
        log.beat(1, 101);
        assert!(log.workers_healthy(100)); // concurrent newer beat
        assert!(!log.workers_healthy(2100));
        log.beat(0, u32::MAX as u64 - 100);
        log.beat(1, u32::MAX as u64 - 100);
        assert!(log.workers_healthy(u32::MAX as u64 + 50));
        assert!(!log.workers_healthy(u32::MAX as u64 + 1900));
    }

    #[test]
    fn concurrent_new_heartbeat_is_not_a_four_billion_ms_stall() {
        let log = Recorder::new();
        log.beat(0, 1001);
        log.sample(1000, 100, 50, Some(-60));
        assert_eq!(log.snapshot().samples[0].tls_gap_ms, 0);
        assert!(!log
            .snapshot()
            .events
            .iter()
            .any(|e| e.kind == Kind::WorkerStalled));
    }

    #[test]
    fn full_history_fits_a_bounded_wire_response() {
        let log = Recorder::new();
        for i in 0..60 {
            let now = u64::MAX - (60 - i) * 6000;
            log.record(now, Kind::AdmissionRejected, i32::MIN + i as i32, u32::MAX);
            log.sample(now, u32::MAX, u32::MAX, Some(i8::MIN));
        }
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 48);
        assert_eq!(snapshot.samples.len(), 32);
        assert!(serde_json::to_vec(&snapshot).unwrap().len() < 32768);
    }

    #[test]
    fn missing_rssi_does_not_invent_or_mask_wifi_transitions() {
        use miniframework::events::Event as E;
        let log = Recorder::new();
        for (i, rssi) in [None, Some(-60), None, Some(-70), None]
            .into_iter()
            .enumerate()
        {
            log.sample(i as u64 * 5_000, 100, 50, rssi);
        }
        assert_eq!(log.count(Kind::WifiDown), 0);
        assert_eq!(log.count(Kind::WifiUp), 0);
        assert_eq!(log.snapshot().samples.len(), 5);

        log.apply(
            25_000,
            &E::WifiDown {
                reason: 201,
                rssi: -90,
            },
        );
        log.sample(25_000, 100, 50, Some(-60));
        log.apply(30_000, &E::WifiUp);
        log.sample(30_000, 100, 50, None);
        assert_eq!(log.count(Kind::WifiDown), 1);
        assert_eq!(log.count(Kind::WifiUp), 1);
        let events = log.snapshot().events;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].code, 201);
    }

    #[test]
    fn framework_events_land_as_incidents_and_heartbeats() {
        use miniframework::events::{Event as E, Task};
        let log = Recorder::new();
        assert!(!log.workers_healthy(100));
        log.apply(
            100,
            &E::Turn {
                task: Task::Handshake,
                tls: 1,
                http: 0,
            },
        );
        log.apply(
            100,
            &E::Turn {
                task: Task::Serve,
                tls: 3,
                http: 2,
            },
        );
        assert!(log.workers_healthy(200));
        assert!(!log.workers_healthy(100 + u64::from(STALL_MS)));
        for event in [
            E::TlsFailed {
                code: -0x7780,
                ms: 900,
            },
            E::TlsTimeout { ms: 4000 },
            E::AdmissionRejected { secure: true },
            E::SlowRequest {
                status: 200,
                ms: 750,
            },
            E::WriteStalled { ms: 15_000 },
            E::RequestTimeout { ms: 5000 },
            E::WifiDown {
                reason: 201,
                rssi: -90,
            },
            E::ReconnectFailed { code: 12 },
            E::IdleExpired,
        ] {
            log.apply(300, &event);
        }
        log.apply(400, &E::Handshake { ms: 640 });
        let snapshot = log.snapshot();
        assert_eq!(snapshot.high_water, [1, 3, 2]);
        assert_eq!(snapshot.last_handshake_ms, 640);
        let find = |kind: Kind, code: i32| {
            snapshot
                .events
                .iter()
                .find(|e| e.kind == kind && e.code == code)
                .copied()
        };
        assert_eq!(find(Kind::TlsFailed, -0x7780).unwrap().duration_ms, 900);
        assert_eq!(find(Kind::AdmissionRejected, 1).unwrap().count, 1);
        assert_eq!(find(Kind::SlowRequest, 200).unwrap().duration_ms, 750);
        assert_eq!(find(Kind::RequestTimeout, 1).unwrap().duration_ms, 15_000);
        assert_eq!(find(Kind::RequestTimeout, 0).unwrap().duration_ms, 5000);
        assert!(find(Kind::WifiDown, 201).is_some());
        // Idle expiry is counted, never logged as an incident.
        assert_eq!(log.count(Kind::IdleExpired), 1);
        assert!(find(Kind::IdleExpired, 0).is_none());
    }
}
