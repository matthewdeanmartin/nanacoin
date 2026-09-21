# Write memory and CPU use (2026-09-18)

Final firmware is flashed (948,944 bytes). **9/9 E2E checks passed.** The final
60-second, two-user market test made **502 requests: 501 succeeded and one
purchase had a transport failure**. There were no listing 404/507 failures,
serial OOMs, or observed resets. The board remained responsive and reported
`ledger_balanced: true`. Post-run diagnostic free heap was 5,904 bytes; transient
response samples went as low as 128 bytes. This is not an OOM-free guarantee.

[Final HTML report](../nanacoin_load/reports/20260918-164339-market-9b4c/index.html)
| [Final E2E report](../nanacoin_load/reports/20260918-164328-e2e-c68c/index.html)


## Implemented

Money handlers borrow one of four startup record buffers. Each includes a
transaction, two postings, JSON scratch, and a 2,560-byte encoded response.
Replies remain owned by that request until transmission finishes. A regression
test with escaped maximum-length fields found that 2,048 bytes was too small;
the strengthened test uses 2,288 bytes. Adapter plus record byte-buffer budget
is now capped at 18 KiB; that budget excludes object metadata and other tables.

The retry cache reserves 16 descriptors and 4,096 response bytes at startup.
It copies keys into fixed 80-byte fields, compares identity components separately,
and compacts out the oldest receipts when either limit fills. No growing map,
key concatenation, order slice, or per-receipt response allocation is needed.
A cache hit copies into the requesting worker's buffer while holding the service
lock, so later eviction cannot corrupt a response still being transmitted.
Matching keys are serialized through eight existing lock stripes.

Request parsers share a mutex-protected 640-byte escape scratch. Parsed strings
still own their bytes. Recording HTTP events stores status, method, and path
without assembling a new message each time. Reading diagnostics assembles the
same readable status/route text; errors retain their descriptions.

## Retry cache in plain language

If a browser sends a transfer and loses the reply, it can send the same request
again with the same Idempotency-Key. While the receipt remains in the cache,
the server returns the original result instead of moving money again. Identity
is (user, endpoint, key). This is a recent-retry window, not permanent duplicate
protection: old receipts are evicted, and this board loses RAM on reboot.

A persistent backend can restore receipts from its journal. The money event
and its receipt are currently separate commits, however: a power loss between
them leaves a deduplication gap. Future persistence must address that explicitly.
No flash writes were introduced here.

## Commit processing

A commit is how a validated change becomes recorded state. Under the service
mutex, the service creates a typed event, appends it through the journal, and
then applies it to balances, ledger, users, or listings. A journal rejection
prevents application. Previously live commits encoded the event and decoded it
again before applying it; now live commits apply the original typed event.
Replay decodes stored events and uses exactly the same application function.

Retaining journals still receive encoded events, with one bounded 4,352-byte
startup scratch buffer and rejection before append if the event cannot fit.
The board uses a discard journal: it has no persistence and formerly framed,
copied, decoded, and discarded every event. An explicit optional discard
capability now validates size and advances sequence/byte accounting without
serializing those unused bytes. It needs no wire scratch. Persistent backends
must not advertise this capability. They retain the ordinary append contract,
compatible with a future fixed batch staging buffer and flash flush policy.

These are allocation reductions, not a claim that writes allocate nothing.
Remaining work includes owned request strings, IDs, user/account/listing
snapshots, typed event objects that TinyGo may put on the heap, diagnostic
rendering, and network/runtime allocations. Host allocation counts are not
proof of TinyGo behavior.

## The two CPU cores

This firmware does **not** dedicate one CPU core to Wi-Fi and the other to the
application. In this installed TinyGo ESP32-S3 target, startup enters CPU0 and
the target selects the `tasks` scheduler with 8 KiB goroutine stacks. The radio
adapter's `espradio_task_create_pinned_to_core` ignores `core_id` and launches
an ordinary Go goroutine. Wi-Fi tasks and application tasks therefore share
CPU0; this firmware does not start CPU1 for application work.

Four HTTP workers mean concurrent requests can overlap while others wait for
I/O. They do not mean four CPU threads or use of the second core. Domain changes
are protected by the service mutex. Using CPU1 would require runtime/radio
integration and synchronization work, not just increasing the worker count.

Verified local sources: `third_party/espradio/radio.go` (task creation),
`third_party/espradio/radio.c` (`wifi_task_core_id = 0`), and installed TinyGo
`targets/esp32s3.json`, `src/device/esp/esp32s3.S`,
`src/runtime/runtime_esp32s3.go`, and `src/runtime/scheduler_tasks.go`.

## Validation

Internal Go race tests and vet pass. Parser fuzzing completed 69,087 cases.
Tests cover cache eviction against an independent FIFO model, response
ownership after eviction, compound-key collisions, allocation-free cache churn,
maximum escaped purchase encoding, live/replay agreement, exact codec sizes,
readable log wrap, and failed commits leaving balances/sequence/bytes unchanged.
TinyGo build and flash verification passed. DWARF inspection found a largest
individual frame of 592 bytes among the inspected core/API/ledger/eventlog/
memory-journal functions (commit: 384 bytes). Individual frames do not prove a
whole call chain fits, but no oversized fixed-array frame was introduced.
Hardware results follow.


## Board results, allocation build before listing-capacity fix

Tested attached upstairs on the weak Wi-Fi link, with no reboot between these
runs. Native COM9 capture is in
`../nanacoin_load/reports/20260918-write-reuse/serial-final.log`; allocation
compiler output, inspected frames, and firmware hash are in the same directory.

- E2E: 9/9 checks passed (`20260918-161426-e2e-e7d5`).
- Issuance ramp: 1,008 writes, zero failures, 1/2/4 users for 30 seconds each,
  no think time. Median 171 ms, p95 403 ms, aggregate 11.06 requests/second.
  This crosses the 365-record ledger more than twice. The after-run diagnostic
  reported 7,184 free bytes; the lowest response sample was 352 bytes.
  Report: `../nanacoin_load/reports/20260918-161445-write-aa4e/index.html`.
- Retry contention: 328 requests, zero failures or duplicate-ID anomalies;
  each journey sends four simultaneous requests with one key. Median 253 ms,
  p95 502 ms. Lowest response sample: 176 bytes. Report:
  `../nanacoin_load/reports/20260918-161630-replay-89dd/index.html`.

The revised discard path recovered 4,352 bytes versus the first iteration of
these changes. It does not increase startup free memory versus the older
capacity-only firmware: previously dynamic storage is now paid for up front.
Serial deltas put this build's startup baseline at 13,904 bytes (older build:
24,128). These passing runs demonstrate improved exercised write endurance,
not an established maximum or an OOM cure. Transient headroom remains very low;
weak-link latency and radio drops also confound pure application throughput.


The marketplace workload then exposed another bounded-storage bug: 306 requests
included 26 create-listing 404s and one purchase transport failure. The board
remained responsive, with no serial OOM. Locust also logged a CSV-writer shutdown
exception; that is a harness error, not evidence of a board crash. Report:
`../nanacoin_load/reports/20260918-161711-market-12c6/index.html`.

A regression reproduced the domain bug: when the shared 12 KiB text arena ran
out of space, listing storage could truncate an ID, commit the event, and then
fail to find the newly created listing. Listing creation and editing now
preflight three text fields against a fixed, allocation-free placement plan.
Replacement is atomic: all strings fit in full, or existing slots remain
unchanged. Failure returns HTTP 507 `storage_full` before the journal changes.
This does not expand the text arena or evict active offers. Capacity refusal
under enough text pressure is expected; silently losing IDs was not.

Closed listings can now also be recycled when text fills before the 48-row
listing table does. Previously recycling only triggered at the row limit,
which left reclaimable closed offers occupying the scarce text space. Active
listings remain protected. Regression tests cover early closed-slot recycling,
full-arena refusal before journaling, and preservation of existing text on a
failed update. The placement algorithm reserves no additional arena or bitmap.


## Follow-up capacity validation

The next image passed 9/9 E2E checks. A four-user, 60-second issuance run made
489 requests with four transport failures (no HTTP application errors); the
board stayed responsive with no serial OOM/reset. Median 417 ms, p95 896 ms.
Report: `../nanacoin_load/reports/20260918-162549-write-d9bc/index.html`.

Its marketplace run made 301 requests: 21 creations and 21 purchases succeeded;
259 creations received explicit 507 capacity refusals, with no missing-ID 404s.
The final status was responsive and ledger_balanced=true. However, no offers
were active: inspection showed the oldest closed listing was too small to free
enough space while later closed listings could. Recycling now selects the
oldest *fitting* closed slot. This maintains active-offer protection and avoids
a small closed offer blocking all future larger offers. A regression covers
that exact ordering. Final validation of that refinement follows.


The first placement refinement still produced 507s after 42 successful
create/purchase pairs (`20260918-163018-market-da68`). A failing unit test then
isolated a fragmented-layout case: placing a short title in a larger freed ID
block could prevent replacing equal-sized fields that already fit. The planner
now reserves each existing field's blocks when they fit before placing fields
that need more space. The fragmented-layout regression fails before this change
and passes afterwards. This affects fixed-arena placement, not GC heap behavior.


A one-time, allocation-free serial capacity snapshot showed an additional
board/host discrepancy: closed slots with field lengths 11/200/20 were reported
unable to fit new fields of exactly 11/200/20, despite host tests accepting the
same replacement. See `serial-capacity-diagnostic.log`. The placement helper now
uses a caller-owned `TextReplacement` scratch object in the service store,
passed by pointer, instead of array arguments and an array return. It clears
borrowed string references after use. This avoids the aggregate calling path;
the specific compiler/runtime cause has not been established.

The serial snapshot is retained as diagnostics: once per boot on listing
capacity refusal, it prints requested lengths, arena usage, and occupied slot
sizes/statuses. It does not print user text or credentials. HTTP diagnostic log
entries still have readable methods, paths, statuses, and error messages.


## Final pointer-plan validation

The pointer-based plan removed the board-only false-capacity behavior: 251
listing creations succeeded, and 250 of 251 purchase requests succeeded in
60 seconds at two users. One purchase had a transport failure (HTTP status 0);
its cause is not established by this run. Median latency was 199 ms, p95 457 ms,
and aggregate throughput 8.22 requests/second. The response-header minimum free
heap was 128 bytes; post-run GC diagnostics recovered to 5,904 bytes. Serial
showed no OOM/reset. The device was still upstairs on the weak Wi-Fi link.

Final status: 258 lifetime transactions, 45 retained, 365 record capacity,
one active listing, and ledger_balanced=true. Shared text pressure evicts ledger
history earlier than the record ceiling; the 365-slot capacity is unchanged.
No data reset or recovery flash followed this validation.

Final evidence: `../nanacoin_load/reports/20260918-write-reuse/` contains
`serial-pointer-plan.log`, `build-pointer-plan.json`,
`stack-frames-pointer-plan.json`, `tinygo-allocs-pointer-plan.txt`, and
`pointer-plan-after-status.json` / `pointer-plan-after-diag.json`.
The largest inspected application frame remains 592 bytes. Internal Go race
tests and vet pass, including sustained market churn following 1,000 issuances,
production-length listing IDs, and the fragmented equal-size replacement case.
