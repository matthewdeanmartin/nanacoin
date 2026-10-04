# Ledger growth experiment: OOM at transaction 257

## Result

The transaction backing slice fails while growing from capacity 256 to 512.
This is now reproduced on a fresh board, and diagnostic markers identify the
failing append in `internal/ledger/book.go`, not just the endpoint in flight.

| Run | Starting transactions | Last successful transaction | Failure |
|---|---:|---:|---|
| Earlier aged household (`write-isolation`) | 235; replay phase took it to 236 | 256 | OOM on next issuance |
| Fresh reset, original flashed firmware (`ledger-fresh-270-warm`) | 0 | 256 | Serial OOM on attempt 257 |
| Fresh diagnostic build (`ledger-trace-270`) | 0 | 256 | `txns before len 256 cap 256`, then OOM, no `after` |

All runs used one sequential issuance workload. The fresh runs requested 270
operations, stopped at the first anomaly, and never reached 270. Monitoring
used the same five-second `/diag` interval. No capacity, GC policy, record
layout or allocator optimization was made.

The first cold setup attempt (`ledger-fresh-270`) encountered the known
connection-startup problem and ran no stress operations. Recovery succeeded;
the subsequent measured run began at zero transactions. A separate warm-up
on the diagnostic build also hit the roughly 21-second TCP connection failure
before subsequent requests succeeded. These setup failures are recorded and
are distinct from the later serial-confirmed OOMs.

## Exact growth evidence

TinyGo prints `uintptr` values in hexadecimal. The target measured a packed
transaction at **0x38 = 56 bytes**; older source comments claiming 40 or 48
bytes do not describe the measured layout in this build.

```text
ledger-growth txns before len 128 cap 128 elem 0x00000038 buffer-bytes 0x00003800 free 42176 inuse 239968 mallocs 25128 gc 143
ledger-growth txns after len 129 cap 256 elem 0x00000038 buffer-bytes 0x00003800 free 27824 inuse 254320 mallocs 25129 gc 143
ledger-growth revBy before len 128 cap 128 elem 0x00000004 buffer-bytes 0x00000400 free 27824 inuse 254320 mallocs 25129 gc 143
ledger-growth revBy after len 129 cap 256 elem 0x00000004 buffer-bytes 0x00000400 free 26784 inuse 255360 mallocs 25130 gc 143
...
ledger-growth txns before len 256 cap 256 elem 0x00000038 buffer-bytes 0x00007000 free 31760 inuse 250384 mallocs 41478 gc 273
fatal error: out of memory
abort called
```

The final OOM was captured at **2026-09-18 16:00:54.320 UTC**. The HTTP trace
records `txn-256` as the last completed issuance. The new `txns` buffer needed
**512 × 56 = 28,672 bytes**, plus allocator overhead, in one contiguous run.
The old **256 × 56 = 14,336-byte** buffer is still needed during allocation
and copying. Its memory is already included in the reported in-use count;
it should not be subtracted from free memory a second time.

The reversal index never reached its corresponding `before` marker at this
boundary: the earlier transaction-slice append did not return. Earlier
capacity transitions have paired before/after markers for both slices.

This identifies the failing growth step. It does not directly measure the
largest free run or which retained objects separate the free regions. The
reported aggregate 31,760 free bytes cannot guarantee a 28,672-byte contiguous
allocation. The installed runtime retries allocation after collection before
aborting. No conclusion depends on the misleading `blk` fragmentation proxy.

## Instrumentation and validation

- `growthtrace_on.go` is opt-in with the `ledgertrace` build tag. It prints
  slice name, before/after, length, capacity, measured element size, predicted
  TinyGo buffer size, free/in-use bytes, allocation count and collection count.
- It uses `println` and stack-local `runtime.MemStats`, with no formatting
  library, explicit allocation, forced GC or capacity change. TinyGo's
  `-print-allocs=traceLedgerGrowth` reported no allocation sites. Successful
  growth marker pairs showed exactly one additional allocation for each new
  buffer, with unchanged malloc counts between adjacent diagnostic markers.
- The default build compiles out the hooks. `deploy.ps1 -LedgerTrace` enables
  them; without that switch its behavior remains unchanged.
- The evidence report now decodes hexadecimal target values and pairs growth
  markers, highlighting a missing after-marker without assuming every missing
  marker is a crash.
- All internal Go tests passed; ledger/core tests also passed with
  `-tags=ledgertrace`. All 13 Python evidence-tool tests passed.

Toolchain: TinyGo 0.42.0, Go 1.26.5, LLVM 22.1.4, `esp32s3-generic`.
Diagnostic binary SHA-256:
`103cb551c6fe95470b1d29c7a0b24c1060e6e707aaa812012b2d64708f29a8b0`.
Flashing used bridge COM8, offset zero; esptool verified the written hash.
The diagnostic binary was 924,928 bytes. Both observed boot heap baselines
reported 75,120 free bytes and 265 live objects.

Raw evidence and generated summaries/reports are preserved under:

- `runs/ledger-fresh-boot/`
- `runs/ledger-fresh-270/` (cold setup/recovery)
- `runs/ledger-fresh-270-warm/` (uninstrumented reproduction)
- `runs/ledger-trace-boot/`
- `runs/ledger-trace-warmup/`
- `runs/ledger-trace-270/` (identified growth failure)
- `runs/ledger-restored/` (reset and final liveness check)

These local artifacts are gitignored. Firmware binaries contain linked WiFi
credentials and must not be published with the diagnostic report.

## Next implementation target

Avoid reallocating the whole transaction slice after startup. Decide an
explicit ledger memory budget, then either reserve its bounded storage at
boot (including the reversal index) or use bounded fixed-size storage blocks.
Capacity exhaustion must be reported before committing a mutation, rather
than falling through to `append` and OOM.

The old-buffer/new-buffer overlap and contiguous allocation are the measured
problem. Reducing unrelated small request allocations may improve headroom,
but it is not the first intervention supported by this experiment. The
remaining allocation candidates stay recorded in `EXPERIMENTS.md`.

No storage fix has been implemented as part of this experiment.

## Board left responsive

After both failure captures and their unsuccessful recovery probes completed,
the board was reset without another flash. At 16:04:33 UTC, `/status` returned
200, with zero users/transactions, `provisioned=false`, a balanced ledger and
73,344 free bytes in its status-body health reading. The diagnostic firmware
remains installed. The disposable RAM-only household was cleared by reset.
