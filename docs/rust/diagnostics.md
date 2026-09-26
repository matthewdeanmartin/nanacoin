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
than a successful truncated response. Diagnostic wire tests enforce a 4 KiB
size bound. The serving task reuses its serialization buffer for diagnostics
without taking the ledger lock.

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

For read-only board timing, run from `nanacoin_rs`:

```sh
python scripts/perf-board.py --address 192.168.1.158 --samples 3 --http \
  --output .local/perf-board.json
```

Use the board's current IP. HTTPS still verifies `nanacoin.local` against
`certs/home-ca.crt`. The probe reads diagnostics, status and one public
transaction; it does not log in or mutate the economy. It reports TCP connection
(including resolution when given a hostname), TLS handshake, response-header
wait and body-download time separately. Header wait includes server queueing,
processing and network time; it is not a CPU measurement. Diagnostics snapshots
are saved alongside timings. Use an IP when `.local` resolution is unreliable.

The fresh and reuse modes differ only in requested connection policy. The
`reused` field records actual reuse: persistent HTTP/1.1 is enabled unless the
client requests closure. Optional HTTP comparison may return a non-200
response if HTTPS is required; failures are excluded from success statistics.
Small samples are diagnostic smoke evidence, not stable p95 estimates. Run one
probe at a time for an isolated baseline, then measure explicit client bursts.

After deployment, `python scripts/transport-board.py --address <ip> --output
.local/transport-board.json` checks persistent reads, TLS session resumption,
pipelined framing, rejection of ambiguous framing, exact large gzip assets,
and established-client latency during real/stalled TLS handshakes and a partial
HTTP request, plus 200 reads over eight established sessions. It performs only public GETs, including deliberately malformed
GET framing, and never logs in or changes household data. The TLS task runs
on core 0; established requests and nonblocking socket writes run on core 1.

[The Angular page](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_ui/src/app/pages/diagnostics.ts)
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

## Retained incident history (Rust firmware)

`GET /api/v1/diag/events` returns a sanitized RAM-only history under the existing
public diagnostics/origin policy. Board Health retrieves it with its normal
refresh, shows events, counts and recent samples, and offers a local JSON
download. A failed refresh retains the previous history with a stale warning.
No tokens, bodies, URLs or client addresses are recorded.

The recorder holds 48 fixed events and 32 five-second samples (about 160 seconds),
below a compile-time 4 KiB bound. An independent core-0 sampler adds an 8 KiB
task stack plus RTOS overhead. Snapshot vectors and the bounded 32 KiB incident
JSON response use temporary memory during retrieval. Existing machine/static
diagnostics retain their separate 4 KiB wire bound.

Events include TLS failures/deadlines, full admission/handoffs, abnormal socket
errors, request/send deadlines, slow handling/sending, malformed HTTP, Wi-Fi
disconnect reasons and reconnects, storage failure transitions, startup and
worker stalls/recovery. Gaps of at least two seconds emit recovery events even
if the sampler missed the stall. Worker code 0 means TLS and 1 means HTTP;
timeout code 0 means receive and 1 means send. Samples retain heap/largest block,
RSSI, worker gaps and connection counts. Gaps indicate stalled progress, not its
diagnosed cause; the sampler may itself be delayed.

After one-time native mutex initialization at boot, writers never allocate or
wait for the recorder lock. Matching kind/code events
within five seconds coalesce with first/latest uptime, repeat count and maximum
duration. Counters include events dropped on contention; the response reports
dropped updates and overwritten slots. Idle expiry is only a counter. Random
boot ID and uptime identify the history, which disappears on restart or power
loss. Existing reset reasons and configured panic dumps are separate evidence.

The implementation is built and host/UI tested, not yet flashed. Hardware fault
injection and performance comparison await deployment. It uses the same recorder
as Mastomini; keep the two `src/incidents.rs` files in step.

## System Info pages

System Info exposes **Error Log**, **Database**, and **Configuration** without a
login. Error Log provides the retained incident history and JSON download above.
Configuration shows household/currency names, decimal scale, grants, settlement
timing and economy policies; only Nana's existing protected controls change them.

Database lists persistent collections and ephemeral authentication/incident
caches with occupancy, capacity, active counts and retention rules. It reports
journal/checkpoint headroom, retained transaction range, accounting invariants
and storage faults. Payload RAM estimates, logical journal bytes and physical NVS
entry counts are separate measurements; they do not measure flash wear or total
firmware memory. Native networking buffers and browser-local caches are outside
the database inventory. Expired occupied authentication slots remain counted
until ordinary authentication reclaims them; viewing diagnostics does not do so.

The manual read-only benchmark runs the existing status, public ledger, listings
and transaction lookup queries against current data, at most three times each.
It stops starting queries after 250 ms (an in-flight query can exceed that budget).
An empty ledger reports an expected missing transaction. Server timings exclude
network/TLS and service-lock wait; the page also shows total client round-trip
time. No writes, cleanup, checkpointing or synthetic records occur. This measures
individual query work, not concurrency capacity or cold browser connection time.
