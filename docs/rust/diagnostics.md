# Diagnostics and verification

The shared Angular **Machine health** page reads two unauthenticated endpoints:
`GET /api/v1/diag` and `GET /api/v1/diag/static`. The page is shown to Nana,
but the endpoints intentionally do not require login, so login failures remain
diagnosable. They contain machine metadata, not household accounts or tokens.
Rust enforces its allowed-origin policy; TinyGo's existing CORS middleware
withholds browser access headers for unapproved origins. CORS is not network
access control for non-browser clients.

## Rust: collect, publish, copy

[The sampler](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/bin/esp32/diagnostics.rs)
runs every two seconds on core 0. It gathers heap, die temperature, Wi-Fi,
address, clock, task and stack readings. NVS entry accounting runs every
15 samples (roughly 30 seconds). Only publication takes the snapshot lock:

```rust
snapshot.samples = snapshot.samples.wrapping_add(1);
*self.snapshot.lock().unwrap() = snapshot;
std::thread::sleep(std::time::Duration::from_secs(2));
```

The HTTP side takes a copy:

```rust
let mut result = *self.snapshot.lock().unwrap();
```

`Snapshot` is `Copy`: this copies a small scalar structure, not the ledger or
a collection of strings. The temporary guard drops at the end of that
statement. Server counters use atomics and are overlaid separately, so their
observation time can differ from the last periodic sensor sample. Probes, JSON
encoding and socket writes are not performed under the snapshot lock.

The [wire structs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/diagnostics.rs)
derive `Serialize`, allowing `serde_json_core` to encode directly into a
caller-owned slice. For example:

```rust
pub temperature_c: Option<f32>,
pub rssi_dbm: Option<i8>,
```

`Some(value)` is a measurement; `None` becomes JSON `null`. Do not display
missing temperature as zero degrees. JSON overflow returns an error rather
than a successful truncated response. Each diagnostic request uses a 4 KiB
local output array, bypassing the large regular API response buffer.

`/diag/static` queries chip/build/reset information and at most 16 partitions
on request. It does not run a flash benchmark, scan Wi-Fi, reset the board or
force clock synchronization. The temperature handle and small sampler state
persist; the per-request output does not.

## TinyGo: same dashboard, different capabilities

[machine_diag.go](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/cmd/nanacoin-esp32/machine_diag.go)
adds a current heap reading when an existing HTTP worker serves `/diag`:

```go
h := readHeap()
machineDiagState.mu.Lock()
machineDiagState.count++
count := machineDiagState.count
machineDiagState.mu.Unlock()
```

There is no additional sampler goroutine or stack. Both diagnostic and HTTP
work use the target's single active core, reported as core 0; the chip still
physically has two cores. The only new shared sampling state is a mutex and
counter. JSON streams through the existing fixed-buffer writer, whose encoder
scratch is 192 bytes (not the total HTTP response/buffer budget).

Original crash and trend fields remain intact. `samples` is still the existing
trend-ring count; `machine_samples` counts on-request readings and `sampling`
is `on_request`. `internal.total/free` cover the TinyGo managed heap, not all
SRAM. PSRAM is explicitly reported disabled. Largest block, lifetime minimum,
hardware probes and flash accounting are `null` when unavailable, not invented
measurements. Static partition enumeration is marked unavailable rather than
claiming the board has no partitions. Builds with `nanacoin_nodiag` omit both
routes; hosts without a static provider return 404 for `/diag/static`.

## Browser and measurement limits

[The Angular page](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/angular/src/app/pages/diagnostics.ts)
keeps at most 120 samples, polls without overlapping requests, pauses in hidden
tabs and aborts requests on teardown. Missing values do not create zero-valued
graph points. Rust's stale-sampler warning detects a snapshot that stops
advancing; an on-request TinyGo sample cannot independently prove background
progress while its HTTP server is stuck.

Neither free memory nor task counts measure CPU utilization. Die temperature
is not room temperature. NVS entries are not erase-cycle counters. TinyGo and
ESP-IDF heap definitions differ, so do not compare totals as identical measures.

Use host API/serialization tests, Angular tests and firmware compilation before
hardware work. A build proves type/link compatibility, not radio reliability,
timing or power-loss recovery. The separate [load-test guidance](../tinygo/diagnostics.md)
applies to workload design, but Rust writes persistently: load-test economies
are not disposable RAM state. No automatic flash/reset is part of verification.
