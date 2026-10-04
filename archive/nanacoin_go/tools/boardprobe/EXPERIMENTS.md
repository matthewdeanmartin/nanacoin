# Finding the next bottleneck

Use `bottleneck.py` for isolated experiments with evidence saved on the host.
Keep `torture.py` for mixed household traffic after the isolated cases pass.
No allocation optimizations or firmware changes are included in this tooling.

The first hardware run captured a real OOM at the inferred 257th transaction:
see [the findings and next experiment](FINDINGS-2026-09-18.md). Ledger backing
slice growth is now the leading suspect, ahead of general request cleanup.
The [follow-up experiment](LEDGER-EXPERIMENT-2026-09-18.md) identifies that
growth as the failing step. Build with `deploy.ps1 -LedgerTrace` to reproduce
the diagnostic markers; ordinary builds omit them.

## What is already available

| Source | Useful evidence | Limits |
|---|---|---|
| `X-Nanacoin-Health` | Free/in-use bytes, objects, average blocks/object, GC count on each response | Taken before the handler; not its peak. OPTIONS may omit it. |
| `/api/v1/diag` | Uptime, recent heap trend, failed-response count | Same workers and locks as load; only endpoints of a 32-sample history. |
| `/api/v1/status` | Transaction/listing/user counts, journal size, ledger balance check | The discarding journal's byte count is not retained heap usage. |
| `/api/v1/logs` | Last 64 routing/status decisions | RAM-only; extra requests overwrite old entries. Captured at start/failure/recovery. |
| Native USB serial | Per-connection heap, cumulative allocations/frees, accepted/refused counts, radio counters, fatal/boot messages | Must capture before failure. USB disconnect or silence is not a diagnosis. |
| `blackbox.go` | Current-run bookkeeping | This build explicitly says nothing survives reboot. `crashed=false` is not evidence of no crash. |

Two misleading names must not guide a fix: `alloc_failures` counts failed
response writes, not failed allocations; `frag_now`/`blk` is average allocation
size, not the largest contiguous free block. The old capture script's confident
conclusions from silence or a low `blk` are not justified. Missing fatal output
does not rule out OOM. The new report keeps these conclusions uncertain.

GC runs in the accept loop after dispatching a connection. Worker requests can
still be live, so its samples are not guaranteed to be quiescent post-request
measurements. The `--settle` interval standardizes comparisons but cannot
force GC without introducing another connection.

## Run an experiment

From this directory (Python 3.11+; only serial capture needs `pyserial`):

```powershell
python -m unittest discover -p test_bottleneck.py -v
python bottleneck.py --ip 192.168.1.158 --scenarios status --operations 5000 --seconds 1200 --serial auto
```

Serial capture auto-discovers the ESP native USB VID `303A`, timestamps it in
the same event stream as requests, and retries when USB disappears. It does
not reset or flash. Close other serial readers first. Use `--serial COM9` to
select a port explicitly. Without serial, HTTP evidence still works, but the
last fatal message cannot be recovered later. `serial_unavailable` is recorded.

The default is status-only, with no login or provisioning. Authenticated
scenarios log in as `nana/nana-pin`; override `--username` / `--password` for a
different test household. Setup is recorded separately from measured load.
Use `--provision --allow-writes` only for a disposable unprovisioned board.
An existing household is preserved; the tool never resets it automatically.

Each run creates a unique `runs/` directory containing:

- `events.jsonl`: flushed start/header/end records, UTC and monotonic timestamps,
  operation/phase identifiers, connection/header/total latency, bytes, expected
  and actual status, transport stage/error, parsed health, serial, snapshots.
- `summary.json`: per-phase status counts, p95, minimum free bytes, before/after
  state and quiet heap deltas, first anomaly and recovery outcome.
- `report.md`: serial allocation rates, free-memory windows, unfinished requests,
  last snapshots and serial context. Regenerate even after killing the host:
  `python evidence_report.py runs/NAME`.

Tokens, authorization codes, passwords and response bodies from normal load
are not persisted. Status/log snapshots and serial may contain household
metadata; run files are gitignored. Supply `--firmware HASH-OR-LABEL` to identify
the actual flashed build. The tool cannot infer it from the source checkout.

## Experiments and comparisons

Run paired experiments from comparable initial board state. Sequential phases
share accumulated state; their order is not a clean A/B experiment. Record the
starting transaction count and heap before comparing. No other client should
be using the board during an attribution run.

| Scenario | What it isolates | Next investigation if it fails |
|---|---|---|
| `status` | Adapter, middleware, status encoder; no auth | Request scaffolding, health/log allocation, network path |
| `me` | Adds authentication and unpacking one user | User reconstruction, verifier copies, session lookup |
| `ledger` | Packed rendering at `--page-size` | Compare page sizes 1 and 30 on the same ledger; buffer/interface escapes |
| `listings` | Unpacking/rendering listing text | Compare empty vs populated market, short vs long descriptions |
| `preflight` | OPTIONS + authenticated GET | Compare against `me`; pool pressure from extra connections |
| `concurrency` | Status at `--levels 1 2 4 8` | Worker/pool saturation; inspect refused count, latency stage and recovery |
| `missing-fixed` / `missing-unique` | Same error repeatedly vs new absent account IDs | Lookup calls `Intern`; unique inputs can retain strings even on 404 |
| `issue-replay` | One transaction with repeated identical key | Cache-hit path; transaction count must increase by exactly one |
| `issue-new` | New key and committed transaction each time | Idempotency eviction, ledger capacity, journal encoding; count must match |
| `listing-churn` | Create/cancel, default 500-byte description | Arena consumption, recycling, text copies; verifies returned ID/text |
| `auth-churn` | Login/logout without leaving live sessions | Auth code/session maps, hashing allocations; 429/503 remain distinct |

Examples:

```powershell
python bottleneck.py --scenarios me ledger listings --operations 1000 --seconds 1200 --serial auto
python bottleneck.py --scenarios concurrency --operations 200 --levels 1 2 4 8 --serial auto
python bottleneck.py --scenarios issue-replay --operations 100 --allow-writes --serial auto
python bottleneck.py --scenarios issue-new --operations 100 --allow-writes --serial auto
python bottleneck.py --scenarios listing-churn --operations 100 --allow-writes --serial auto
python bottleneck.py --scenarios missing-unique --operations 100 --allow-writes --serial auto
```

Missing-ID tests require `--allow-writes` because the current lookup
implementation interns unknown IDs. They can exhaust the intern table and
affect later legitimate operations despite using GET. Run them last or on a
fresh disposable board. Listing churn can expose silent text loss before a
network failure; that is a failure worth stopping for.

## Stop rules and recovery

The first unexpected HTTP status, timeout, malformed JSON, body truncation,
integrity mismatch, boot banner/uptime decrease, or `free < --min-free` stops
new load. Already-started requests have real socket deadlines and are recorded.
Default headroom guard is 8192 bytes; `--min-free 0` explicitly disables it for
a crash run. Even a recovered 503 fails the experiment: it marks a capacity
boundary, not a crash. Deliberate 404s are expected only in missing-ID tests.

After load drains, wait `--settle` seconds (default 12, longer than the board's
10-second connection deadline), collect snapshots, and make three spaced
status probes. Keep serial open throughout. A responsive board afterward is
different from a persistently unreachable one; neither outcome alone identifies
the cause. No automatic reset destroys the evidence.

`--seconds` bounds the run, including setup. A bounded recovery window may
extend it by at most `3 * timeout + settle + 10` seconds. Exit code 2 denotes an
anomaly/interruption, including recovered capacity failures; zero means only
that the requested experiment finished without its checked anomalies.

Monitoring is load too. The default `/diag` interval is five seconds and those
requests are tagged separately. Compare with `--monitor-interval 0` while
keeping passive headers and serial capture to quantify the observer effect.
Startup/final snapshots still run. A monitor timeout during saturation is
evidence of shared capacity, not an independent CPU liveness check.

## Deferred implementation ideas

The improvement from roughly 40 requests to roughly 4000 after allocation
reduction is the working motivation, not proof that the next failure is OOM.
Preserve these candidates until measurements identify which path matters:

1. Preallocate per-worker HTTP request/URL/body-reader/response objects and
   header storage; avoid constructing them in `boardhttp.buildRequest`.
2. Put encoder/decoder scratch in worker-owned storage. A local array passed
   through an interface is not guaranteed to stay on TinyGo's stack.
3. Read authenticated identity and render views without allocating unpacked
   domain objects or copied arena strings.
4. Separate intern lookup from insertion; use fixed intern storage.
5. Replace idempotency's allocating map/result/FIFO churn with bounded storage
   while preserving retry semantics.
6. Reduce diagnostic string and middleware logging allocations after measuring
   their observer effect.

If those are still ambiguous, the next instrumentation should expose current
arena/intern/cache occupancy, per-worker phase and network-pool counters, and
the allocator's largest free run. None is currently fully observable over
HTTP. Add fixed-size counters only, measure their overhead, and distinguish
that firmware change from an optimization experiment.
