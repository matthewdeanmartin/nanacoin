# Initial execution — 2026-09-18

## Physical board: setup itself became unresponsive

Target: `http://192.168.1.158`. No firmware edits, flash, or reset were made.
The board initially reported an unprovisioned household and 512 ledger slots.

| Request, in order | Result | Header free heap | GC count |
|---|---:|---:|---:|
| Status | 200 | 9,696 | 5 |
| Provision Nana | 201 | 9,232 | 6 |
| Authorize Nana | 200 | 7,184 | 30 |
| Exchange Nana token | 200 | 5,904 | 58 |
| List users | 200 | 5,312 | 59 |
| Create first member | 201 | 5,152 | 60 |
| Authorize member | ConnectionError after 19.2 seconds | unavailable | unavailable |

The six successful requests took about two seconds together. Setup was
sequential: this did not require concurrent users. Later diagnostic connection
attempts timed out too, including the readiness check for a read-only Locust
run. That run did not start sending load. The board had already been running
before this experiment; this is not a fresh-boot benchmark.

The heap decline and rapid collection count increase make memory pressure a
plausible explanation. Without serial evidence or a surviving crash record,
this does **not** establish OOM, a leak, fragmentation, or a thermal cause.
No ESP32 serial port was attached on this PC (only COM3 was visible).
The user was asked to reboot before a read-only baseline.

Evidence: `reports/20260918-130858-prepare-a9cf/` and
`reports/20260918-131555-status-54fa/`. These are local, gitignored artifacts;
this note retains the result if reports are moved or removed.

## Harness validation, separate desktop target

Against an isolated desktop build at `http://127.0.0.1:18080`:

- All nine end-to-end assertions passed: balance changes, reversal, purchase
  retry, four concurrent issuance retries, authorization, history and invariants.
- All seven Locust workload profiles completed short two-user runs without
  request failures. A separate 1→2-user browsing ramp completed 120 requests
  without failures. These are harness checks, not ESP32 performance claims.
- A deliberate desktop-server shutdown verified three failed diagnostic
  probes stop the shape, preserve Locust HTML/CSV and raw evidence, and perform
  bounded recovery probes. It exposed a double-shutdown race in the initial
  harness; shutdown is now owned solely by the load shape, and the repeated
  fault test completed without that exception.
- The HTML overview was rendered and visually inspected in headless Edge.

Next board runs after reboot: public status baseline first; then fixture setup,
authenticated reads, browser bursts, writes, and authentication churn separately.
Keep fresh-boot and aged-board results separate. Do not assume the throughput
ceiling is I/O or thermals until the memory evidence supports that conclusion.

## Authentication allocation work: attached board, weak Wi-Fi (2026-09-18)

The owner confirms the attached board is four floors above the router. These
runs measure network plus application resilience; repeat near the router for
an application-focused comparison. Native USB logs are now available on COM9.

The first new firmware reserved the entire prior auth ceilings up front:
64 sessions / 16 codes / 64 failure counters, 9,088 bytes of slot storage.
This left about 4.2 KiB at startup. During the first status profile the serial
heap samples fell through 3,920, 2,400, 1,088, 1,376 and 1,248 bytes. Serial then
printed `fatal error: out of memory` and `abort called`. This is confirmed OOM,
not an inference from network timeouts. Five Locust requests were recorded;
the five serial connections also include the readiness/diagnostic traffic.

Evidence: `reports/20260918-135329-status-64fc/` and
`reports/20260918-auth-memory-hardware/serial.log`.

The revised board profile reserves 32 sessions / 8 codes / 16 failure counters
(3,776 bytes). Full pools refuse new logins; they never replace live sessions.
This leaves the HTTP workers, TCP pool and ledger capacities unchanged. The
second flash is a revised intended build, not an availability rollback.

The second profile passed 27 status requests (0 failures, median about 67 ms)
but then OOMed during provisioning after that warm-up:
`reports/20260918-135733-status-a051/` and `serial-v2.log` in the hardware folder.
The third image used fixed SHA compression scratch and got through provisioning,
Nana authorization and token exchange. It OOMed on the first member creation;
`reports/20260918-140713-prepare-1eae/` and `serial-v3.log` retain that evidence.
Nana authorize/token no longer caused the earlier 52-GC jump, but fixed headroom
was still insufficient. A fourth intended build saves 3,840 bytes by batching
the ledger invariant scratch calculation; transaction capacity remains 512.

## Final auth image and isolated verification

Final image size: 935,472 bytes. COM8 disappeared; flashing and hash verification
succeeded through the same board's native USB Serial/JTAG COM9. No older firmware
was restored. Boot reports 20,928 free bytes and the auth self-check reports:
`hash allocations 3 verify allocations 0 verify bytes 0`.

- Setup passed: `reports/20260918-142144-prepare-1981/`.
- E2E: `reports/20260918-142226-e2e-b57c/`. Transfer debit, reversal restoration,
  purchase retry identity and seller paid once all passed (four checks). The
  following four concurrent same-key `/admin/issue` requests produced two 201s
  and two connection failures/timeouts. Serial confirmed OOM. The last successful
  issuance header sampled only 3,168 free bytes. Remaining E2E checks did not run.
  Serial: `reports/20260918-auth-memory-hardware/serial-v5.log`.
- The same image was reset once for an **isolated authentication experiment**,
  not restored for availability. Setup passed again in
  `reports/20260918-142610-prepare-ebc7/`.
- Auth ramp: `reports/20260918-142612-auth-c8b6/`, 1 -> 2 -> 4 users, 20 seconds
  per stage, default 0.3-1 second journey waits, diagnostics every five seconds.
  100 authorization + 100 token + 100 logout requests, **zero failures**. Overall
  4.90 requests/s; p50 201 ms, p95 653 ms, p99 1,210 ms. Lowest sampled free heap
  1,440 bytes. No monitor anomalies or serial OOM in this isolated run. Serial
  ended around 8,864 free bytes after connection 330. These are different sample
  points than response headers, not competing measurements of the same instant.
  Serial: `reports/20260918-auth-memory-hardware/serial-auth-isolated.log`.

All runs were with the board attached four floors above the router. Repeat near
the router before treating latency or connection failures as application-only
limits. Authentication now has a measured zero-allocation verifier and a passing
short churn test; the server still has a reproducible concurrent-write OOM.
Next experiment: isolate four-request idempotent issuance, capture allocation
size/peak request ownership, and compare cold versus warm receipt caches.

The captured ROM boot log also printed an image SHA comparison warning before
entering the application. Esptool's flash-content verification succeeded and the
app booted; this warning is separate from the application's SHA implementation
(which has not started at that point). Its cause was not investigated here.

### Unexpected restart after the auth run

After the successful auth run ended at approximately 14:27:15 EDT, a later status
check found an unprovisioned household. Diagnostic uptime places the new boot at
approximately 14:27:38. The owner confirmed they had not touched the device.
This is an unexpected restart, with no captured cause; do not classify the auth
experiment as an endurance pass or attribute this restart to OOM without evidence.

The estimated boot time coincides with the 180-second serial watcher finishing.
However, an isolated native USB open/close check did not reproduce a reset:
uptime advanced from 228 to 229 to 232 seconds across open and close, then to 417
seconds after that Python process had exited. Evidence is in
`reports/20260918-auth-memory-hardware/serial-control-test.json`. The timing
correlation alone does not establish that the watcher caused the restart.

This firmware has no surviving crash record: `crashed:false` and `boots:0` after
a reboot cannot rule out a crash. A follow-up experiment must keep serial capture
and uptime polling running beyond workload completion, recording monitor shutdown
times separately, to capture either a panic or the next ROM reset banner. Keep
this unexplained restart separate from the serial-confirmed concurrent-write OOM.

## Near-router repeat (2026-09-18, 14:40 EDT)

The owner subsequently clarified that they had picked up the board during the
earlier session, then unplugged it from the development machine and moved it
four floors downstairs, near the router. Movement could interrupt the weak
office link; it does not by itself establish the cause of the earlier uptime
reset. This repeat uses independent power and HTTP telemetry only, with no USB
serial capture. Firmware was not changed or flashed for this repeat.

Preparation succeeded in `reports/20260918-144018-prepare-db9e/`. E2E in
`reports/20260918-144041-e2e-8984/` passed the same four transfer/reversal/purchase
checks, then became unresponsive at four concurrent same-key issuance requests.
All four requests failed: one ConnectTimeout and three ConnectionErrors. The
last successful response sampled 7,712 free bytes. The last diagnostic before
the burst reported uptime 71 seconds. Subsequent diagnostic probes also failed.

This reproduces the failing workload near the router, making weak office Wi-Fi
an insufficient explanation for the repeated failure. Unlike the earlier USB
run, this run cannot confirm OOM or a restart. No responses from the concurrent
writes means their commit status is unknown; remaining E2E checks did not run.
Independent before/after probes are retained in
`reports/20260918-near-router-observation.jsonl`. Auth/read stress awaits a
user-operated power cycle so it can be measured separately.

## Near-router auth and read sequence after power cycle (14:42–14:49 EDT)

The owner power-cycled the downstairs board. Preparation succeeded in
`reports/20260918-144245-prepare-6971/`. No firmware changes, USB connection or
further resets were made. Workloads below ran sequentially on this same boot,
with default 0.3–1 second journey waits and diagnostic polling every five seconds.

| Workload | Schedule | Requests / failures | p50 / p95 | Minimum sampled free heap |
|---|---|---|---|---|
| Auth | 1, 2, 4 users; 20 s each | 330 / 0 | 182 / 543 ms | 1,184 bytes |
| Status | 1, 2, 4, 8 users; 20 s each | 398 / 0 | 87 / 210 ms | 192 bytes |
| Browse | planned 1, 2, 4 users; stopped early | 85 / 35 | failures dominate later latency | 48 bytes |

Auth completed 110 full authorization/token/logout cycles. Seven post-auth
snapshots over 60 seconds showed uptime 131–191 seconds and free_now consistently
8,032 bytes. Status also passed, followed by seven idle snapshots over 60 seconds;
uptime continued to 356 seconds without a reset. These short, paced passes do not
establish endurance or maximum throughput. Reports:
`reports/20260918-144254-auth-5ecb/` and
`reports/20260918-144519-status-22b1/` include the idle snapshots.

Browse launches five overlapping reads per user (me, users, listings, status,
history). It completed 50 requests, then all five requests in the next burst
timed out. Last successful load response was 10.34 seconds into the run; the
first failed requests began around 11.31 seconds, still in the **one-user stage**.
The first diagnostic failure completed at 18.41 seconds, before the two-user
stage began at 20.41 seconds. Do not attribute the initial failure to two users.
The harness subsequently recorded 35 failures and stopped after three failed
diagnostic probes. Three recovery probes after load stopped also failed, the last
about 18 seconds after subprocess exit. Report:
`reports/20260918-144756-browse-bdb7/`.

The 48-byte free-heap sample strongly suggests memory pressure but cannot confirm
OOM without serial evidence. This is a cumulative-workload reproduction: browsing
started at uptime 357 seconds after auth/status traffic, with 5,200 free_now bytes.
Next diagnostic target is concurrent request memory and retained state after
earlier traffic. Compare a fresh-boot browse-only run, then isolate each of the
five read endpoints and their overlap. User count alone is insufficient: one
page-load user already makes five simultaneous requests, plus diagnostic traffic.

### Fresh-boot browse-only reproduction (14:52 EDT)

After the owner's next reboot, the first preparation probe timed out; a second
preparation succeeded (`reports/20260918-145239-prepare-8c22/`). Browse alone then
ran with one virtual user, five overlapping reads per journey, a planned 60-second
duration and five-second diagnostics. No auth/status load profiles preceded it.

`reports/20260918-145249-browse-a666/` recorded 65 successful requests followed by
20 failures. The last successful request completed 12.38 seconds into the run.
Minimum sampled free heap was 448 bytes. Before load, uptime was 89 seconds and
free_now was 13,536 bytes; the last responding monitor at uptime 100 seconds
reported 4,752 bytes. Three failed diagnostic probes stopped load, and all three
post-run recovery probes failed. Earlier auth/status stress is therefore not
required to reproduce the browse failure.

Practical diagnosis: concurrent request handling exhausts available RAM. The
earlier serial-confirmed write OOM and effectively zero heap during read bursts
support that conclusion. The remaining engineering question is attribution and
budgeting, not whether 48 bytes constitutes useful headroom. Code inspection
finds that boardhttp preallocates body/output chunks but buildRequest still
creates an http.Request, parsed URL, header map, copied strings and header-value
slices; responseWriter and its header map are also constructed per request.
Concurrent workers keep several such object graphs live together. Reusing these
objects with explicit reset/ownership rules is a concrete next target; their
individual TinyGo peak costs and any retained references still need measurement.

## HTTP reuse image: attached-board repeat, 15:29–15:33 EDT

Flashed the intended 937,152-byte image through COM8, verified by esptool. Device
is attached upstairs again; weak-link latency is not directly comparable with
the preceding downstairs runs. No concurrency or ledger capacity reduction.

- Preparation: `reports/20260918-152921-prepare-45e3/`, passed.
- Browse: `reports/20260918-152930-browse-4b5b/`, one user/five overlapping reads,
  60 seconds, usual waits and five-second diagnostics. **245 requests, 0 failures**,
  p50 400 ms, p95 700 ms, minimum sampled free heap 976 bytes. This exceeds the old
  fresh-boot browse failure after 65 successes; it is a short pass, not endurance.
- E2E on that same boot: `reports/20260918-153041-e2e-8bf0/`, failed on the first
  transfer. Last successful response reported 4,544 bytes free. HTTP remained
  unavailable; serial capture does not include the terminal cause for this run.
  Serial: `reports/20260918-http-reuse-hardware/serial-run.log`.
- Isolated same-image reset: COM8 reset left board silent/unreachable, recorded
  by failed preparations `153148-prepare-0ae6` and `153200-prepare-4c32`. COM9 reset
  restored execution. No additional flash or rollback. Preparation then passed:
  `reports/20260918-153244-prepare-966a/`.
- Fresh E2E: `reports/20260918-153253-e2e-de79/`, transfer debit and reversal passed,
  listing creation passed, then purchase failed. Serial explicitly reports
  **fatal error: out of memory / abort called**. Last successful response sampled
  7,504 bytes free. Concurrent same-key issuance was not reached.
  Serial: `reports/20260918-http-reuse-hardware/serial-isolated-native.log`.

Startup free heap inferred from serial baseline deltas is 15,056 bytes, about
5,872 below the previous image's 20,928. Reusable HTTP state improves the browse
case but spends fixed RAM that writes also need. The next target is write-path
peak memory and the startup budget; the current image is not OOM-free. The board
was left in its observed failed state after preserving evidence, with no recovery
flash or extra reset for availability.

## Reduced history capacities (15:39 EDT)

At the owner's request, reduced ledger capacity 512 -> 365 and event log 64 -> 58.
No other capacities changed. Internal race tests passed with modulo wrap tests.
Flashed the 937,216-byte image through native COM9; esptool verified flash data.
Startup free heap inferred from serial baseline is 24,128 bytes, up 9,072 bytes
from the HTTP-reuse build. Preparation passed in
`reports/20260918-153909-prepare-f001/`. E2E passed in
`reports/20260918-153919-e2e-2239/`, including purchase, four overlapping retries
returning one transaction ID, exactly-once credit and final ledger invariants.
Serial and status evidence: `reports/20260918-capacity-365/`.
Board remains attached upstairs; this is a short correctness pass, not endurance.


## 2026-09-18: bounded write memory and listing text recovery

See [WRITE_MEMORY.md](../nanacoin_go/WRITE_MEMORY.md) for implementation, full
experiment history, retry/commit explanations, and verified CPU-core usage.
Fixed response/retry/parser/transaction storage and direct typed commit
application are flashed. Discard-only journaling no longer encodes/decode-copies
bytes with no consumer. Diagnostic HTTP events remain readable.

An intermediate build completed 1,008 issuances at 1/2/4 users without failure,
then 328 same-key retry requests without duplicate-ID anomalies. Marketplace
stress exposed truncated listing IDs, then false capacity refusal. Atomic text
replacement, early closed-offer recycling, and pointer-owned placement scratch
resolve those exercised failures. These are bounded app-storage issues, not
proof of a network stack leak.

Final firmware: E2E 9/9; market 502 requests in 60s at two users, 501 successes,
one transport failure, no 404/507 responses. Median 199ms, p95 457ms. Board
remained responsive; ledger_balanced=true. Post-GC free heap 5,904 bytes, but
transient minimum 128 bytes: memory is still tight. No serial OOM/reset.
Final report: reports/20260918-164339-market-9b4c/index.html.
Serial/build/frame evidence: reports/20260918-write-reuse/.

These runs were upstairs with the device attached. Transport failures remain
unattributed; a strong-signal comparison is needed to isolate Wi-Fi effects.
Application allocations remain (owned strings, IDs, snapshots and some typed
objects), as do network/runtime allocations. Do not describe this as zero-alloc
or a proven endurance limit. No per-request flash persistence was introduced.

# Foreign exchange — 2026-09-19

Forex adds the first write that produces **two** ledger records: a trade is a
coin leg and a cash leg, because `MaxInlinePostings` is 2 and raising it to 4
costs 7.1 KB across the ring. Two records per request is the reason this
feature got an allocation audit before flashing rather than after.

## What was allocating, and what it cost

Found with `go build -gcflags=-m` and confirmed by profiling
`BenchmarkWriteAllocs` at `-memprofilerate=1`.

| Site | Problem | Fix |
|---|---|---|
| `TakeQuote` | both legs were locals with `[]ledger.Posting{...}` literals — 4 heap objects per trade | both legs moved into the caller's `WriteResult`, the scratch buffer every other write endpoint already used |
| `TakeQuote` | `for _, a := range []ledger.AccountID{...}` allocated a slice per take | a `[4]` stack array |
| `Book.BalanceIn` | `USDAccount()` concatenated a string per call — once per user on every `/me` and household listing | built in a stack buffer, looked up via the new `Strings.FindBytes` |
| `findQuote` / `findOffer` / `findListing` | `arena.Get()` built a string **per occupied slot** just to compare | the new `Arena.Equal`, which compares in place |

Measured per call, at the service layer (`BenchmarkWriteAllocs`):

| Path | Before | After |
|---|---:|---:|
| take-quote (incl. its PostQuote) | 54 | 49 |
| usd-balance | 1 | **0** |
| `GET /offers` (whole request) | 145 | 125 |

The `Arena.Equal` change helps listings and offers too — it was a shared bug,
not a forex one.

### What was deliberately left alone

- The two `USDAccount` calls that build the cash leg's posting accounts. Those
  names are stored in the transaction and outlive the call, so they cannot use
  a stack buffer. Two allocations per trade for retained data is the same
  price the coin accounts pay.
- One heap event per write (`&someEvent{}` at every `commitEvent` site). This
  is every endpoint in the codebase, not forex, and `commitEvent` takes `any`
  so the interface boxing forces it. Changing it is a separate job.
- `unpackQuote` costs 2 allocations per quote when reading the book. Offers
  and listings unpack identically; the fix belongs to all three at once.

## RAM cost on the board

Measured by compiling for `GOARCH=386` (the linker's `-size` figure does not
move, because the record pool is a boot-time heap allocation, not `bss`):

| | Before | After |
|---|---:|---:|
| `core.WriteResult` | 108 B | 216 B |
| `api.recordBuffer` | 2,924 B | 3,032 B |
| pool of 4 | 11,696 B | 12,128 B |

**+432 bytes**, taken once at boot — which is the shape this board needs, since
allocating late is what causes OOM. Free heap at boot after flashing: 17,840
bytes.

## On hardware

`ncload e2e` — 23 checks, all passing, 15 of them new forex ones covering both
directions of money, idempotent replay, the refusals, and book ordering.

Sequential soak, 40 trades, zero failures:

| After | Free heap | Live objects |
|---|---:|---:|
| 10 trades | 12,176 | 521 |
| 20 trades | 11,968 | 521 |
| 30 trades | 11,872 | 521 |
| 40 trades | 11,872 | 521 |

Object count pins at 521 from trade 10 and free heap stops moving at trade 30:
the early dip is the quote table and ledger ring reaching steady state, not a
leak.

Concurrent (`--scenario forex`):

| Run | Requests | Failures | p50 | p99 |
|---|---:|---:|---:|---:|
| 1 user, 60s | 195 | 0 | 79 ms | 210 ms |
| ramp 1→8, 30s stages | 1,017 | 0 | 140 ms | 1,500 ms |

Ledger balanced across both currencies after 814 transactions, 7,728 bytes
free and still serving. The board degrades by getting slower, not by dropping
requests — no 503s and no silent drops at 8 users.

## Keeping it this way

`TestWritePathsStayWithinAllocationBudget` (in `internal/core`) turns the
numbers above into a rule, so a regression fails `go test` instead of waiting
to be found on hardware. The budgets are the measured count plus a small
margin, deliberately tight: the first version allowed 60 for take-quote, and
re-introducing the exact regression it was written to catch still passed it.
Heap postings cost 2 allocations per transaction, so a two-leg regression is
4 — which the 49-vs-53 margin now catches.

Re-run after any change to a write path:

    go test ./internal/core/ -run TestWritePathsStayWithinAllocationBudget
    go test ./internal/core/ -run XXX -bench BenchmarkWriteAllocs -benchmem
    make e2e HOST=http://<board>
    make load SCENARIO=forex USERS=1 DURATION=60
    make ramp SCENARIO=forex

Note: the board's tables fill up. A `507` on `POST /listings` after a load run
means the listing table is full, not that the board is broken — reflash to
clear RAM state before an `e2e` run.

# "Stopped early: invalid or expired token" — 2026-09-19

The demo seeder ran for a few minutes and stopped, reporting a dead token,
with no clue on screen about why. Three separate faults stacked up.

## 1. The session table filled and never emptied

`newSessionLocked` took the first free-or-expired slot and otherwise returned
`ErrTooManySessions`. Nothing in this system logs out: sessions are RAM-only,
last eight hours, and the account switcher holds several at once by design.
The seeder signs in as Nana plus every member on each run, so a few runs
filled the board's 32 slots with live sessions nobody held any more — and the
board then refused every login for the rest of the day.

Measured on the board before the fix: **refused at 17 logins** (the Locust
fixture held the rest), permanently.

**Fix**: when the table is full, reuse the *caller's own* oldest session
rather than refusing. Logging in again as yourself can cost you your stalest
session; it can never cost anyone else theirs, so a full table of other
people still refuses honestly.

After the fix: **29+ consecutive logins all succeed**, and three back-to-back
seed runs (15 logins, 180 writes) complete with zero failures and identical
heap.

A per-user cap was tried first and rejected: it broke
`TestSessionCapacityExpiryAndValueIsolation`, which deliberately asserts that
one user may fill the table and that no live session is ever evicted. The
eviction-when-full form keeps that guarantee.

## 2. The error message was the server's internal wording

`writeError` sends `err.Error()` as the user-facing message, so
`auth.ErrNoSession` surfaced literally as "invalid or expired token" — true,
and useless. Worse, it named the *wrong* failure: the refusal was a 503 on
`/auth/authorize`, but the client had already dropped the account it could
not re-authenticate, so what the user saw was the *next* request failing with
a dead token.

**Fix**: the seeder now maps the error to something actionable
(`too_many_sessions`, 401 and 507 each get their own sentence) and logs the
raw code, status and phase alongside.

Also noted in the code: the `too_many_sessions` 503 deliberately carries no
`Retry-After`. The client only retries a 503 when that header says how long
to wait, and waiting cannot help — sessions free on expiry, hours away.

## 3. The failure was reported inside a panel that closed itself

The only report was a `<p class="warning">` inside a `<details>` that
collapsed on re-render, so the run appeared to stop for no reason. There was
a toast on success and nothing on failure.

**Fix**: a toast on failure like every other error in the app, and the
`<details>` is held open while running and after a failure.

## Reusable

- `TestRepeatedLoginRecyclesYourOwnSession`, `TestLoginNeverEvictsAnotherUser`
  and `TestAFullTableOfOtherPeopleStillRefuses` in `internal/auth` pin all
  three halves of the eviction rule.
- Reproduction scripts are in the session scratchpad, not the repo; the
  behaviour they checked is now covered by the Go tests above.

# Rust firmware on hardware — 2026-09-19

First run of `nanacoin_rs` on the physical ESP32-S3 (N16R8, 16 MB flash,
8 MB octal PSRAM, MAC `ac:a7:04:2c:2c:04`). The board had never been flashed
with the Rust build; the previous sections on this page all describe TinyGo.

Flashed bootloader/partition table/app at `0x0`/`0x8000`/`0x10000` over the
CH343 bridge on COM8. The app image had to be produced with `elf2image`: the
build script deliberately stops at the ELF. Board came up at `192.168.1.158`,
HTTPS on 443, mDNS `nanacoin-rs.local`.

## The result

**The board did not fall over.** 68 minutes of continuous uptime across every
workload below, with no reboot: 352 consecutive serial heap samples with
monotonically increasing tick counts. No panic, no `out of memory`, no
`abort called`, no `Guru Meditation` anywhere in the serial log.

| | TinyGo (2026-09-18) | Rust (2026-09-19) |
|---|---|---|
| Free heap at rest | ~9,700 B | 164,767 B |
| Survived provisioning | no (OOM) | yes |
| Survived auth churn | no (OOM, three images) | yes, 170 requests |
| Heap after ~2,900 requests | died long before | 164,719 B |

Free heap returns to **164,7xx after every single run**, from a floor of
105,299 B under the heaviest mixed load. That is the whole difference: the Go
board spent memory permanently per request, the Rust board borrows and
returns it.

`largest free block` was **63,488 in all 353 samples — one value, never once
varying**, including under 20-way connection bursts. There is no measurable
fragmentation, which is the failure mode that forced the TinyGo rewrites.

## Workloads run

All at 0–50 ms think time against the real board. `seed` is the mix the
Angular "add a year of history" button makes; `forex` is the two-ledger-record
currency trade.

| Scenario | Requests | 5xx | Note |
|---|---:|---:|---|
| seed (6 min) | 455 | 0 | listing/transfer/offer/accept lifecycle |
| forex (5 min) | 401 | 0 | 103 two-record trades |
| forex (10 min) | 646 | 0 | endurance; heap unchanged after |
| write (5 min) | 384 | 0 | crossed the 365-record ring wrap |
| browse / ledger / replay | 342 | 0 | 5×, 1×, 4× concurrency |
| auth / status | 343 | 0 | PBKDF2 churn: killed TinyGo three times |
| mixed r/w (5 min, 3 workers) | 374 | 0 | heterogeneous, see below |
| mixed r/w (10 min, 8 workers) | — | 0 | heaviest pressure applied |

The **heterogeneous read/write mix** is the case that took the TinyGo board
down. `scratchpad/mixed.py` interleaves the 365-record state view, 100-record
ledger pages, account history, listing and quote-book reads against issue,
transfer, listing+purchase and forex writes from concurrent workers, so the
response buffer never sees the same shape twice running. Every operation type
completed. Ledger stayed balanced throughout; the ring buffer wrapped
(752 transactions, `retained_transactions` pinned at 365) without incident.

## What the failures actually were

No workload produced a 5xx. Every failure was one of two things, both correct
behaviour:

- **507 Insufficient Storage** — a bounded pool filled (48 listing slots,
  16 quote slots) and the board refused cleanly while still serving reads.
  Draining the slots restored full throughput: 48 consecutive purchases,
  zero failures.
- **"HTTP 0"** — no response at all. This is *not* a status code; it is the
  harness's marker for a connection that produced nothing.

The HTTP 0s are the **2-socket ceiling**, not instability. `max_open_sockets: 2`
in `src/bin/esp32.rs:109`, ~24 KiB of handler stack each. Above two concurrent
requests the lwIP accept backlog fills and further connections are refused at
the TCP layer. A status code cannot be returned there: the connection is
itself the exhausted resource, so there is nothing to send a 503 *on*.

Measured ceiling, patient clients (30 s connect / 60 s read):

| Burst | Completed | Refused |
|---|---|---|
| 10 | 6 | 4 |
| 10 | 6 | 4 |
| 20 | 12 | 8 |

About 60% queue and complete — the last of a 20-burst took 19 s, exactly the
serialisation you would predict behind two sockets. The other 40% never get a
connection, and patience does not help them. Excess load is refused in
milliseconds rather than hanging, so it degrades safely.

**The socket limit is not what is keeping the board stable.** Under 20-way
bursts the heap floor fell to 105,299 B — 54 KiB deeper than at rest — and
recovered completely, with `largest` unmoved. It is throttling throughput,
not concealing an allocator problem.

### Raising it to 4 does not work (tested)

Built and flashed an otherwise identical image with `max_open_sockets: 4` /
`max_sessions: 4`. **It never finishes starting up.** The task watchdog fires
~11 s in (`IDLE0` starved while `main` runs), then the service tears itself
down and returns from `app_main` with `ESP_ERR_TIMEOUT`:

```
E (10956) task_wdt: Task watchdog got triggered... - IDLE0 (CPU 0)
E (10956) task_wdt: Tasks currently running: CPU 0: main
Error: ESP_ERR_TIMEOUT (error code 263)
```

Four sessions need roughly 48 KiB more than two — 24 KiB of handler stack each
plus mbedTLS in/out buffers (`CONFIG_MBEDTLS_SSL_IN_CONTENT_LEN=16384`) — and
`CONFIG_SPIRAM_MALLOC_RESERVE_INTERNAL` reserves only 64 KiB of internal RAM.
That allocation blocks the main task past the watchdog deadline. So the value
is not an arbitrary throttle: **2 is near the real ceiling for this TLS
configuration.** Going higher needs smaller mbedTLS content buffers, a smaller
handler stack, or more reserved internal RAM — not just a larger number.

The 2-socket image was reflashed and the board recovered, with all 1,036
transactions intact across the reflash and both reboots.

## Harness changes required

`nanacoin_load` targeted the Go board and needed three fixes to reach this one:

- **TLS**: board is HTTPS-only. Added `NANA_CA` (`0` disables verification)
  wired into `client.py`, `cli.py` and `workload.py`.
- **Monitor endpoint**: the Rust firmware has **no `/api/v1/diag`**, so the
  monitor's three-failed-probe guard stopped every run before it started.
  Added `NANA_MONITOR_PATH`, pointed at public `/api/v1/status`, and made
  `uptime_seconds` optional (falling back to the ledger invariant).
- **Seed bug**: `seed` read `buyer["account"]`; the fixture stores
  `buyer["user"]["account"]`. Pre-existing — the scenario had never run past
  its first call. Also capped the `market` description to 96 bytes.

In Git Bash, export `MSYS_NO_PATHCONV=1` or `NANA_MONITOR_PATH` is rewritten
to a Windows path.

## Gaps in the Rust firmware

Real parity gaps, not load failures:

1. **No `/api/v1/diag`.** The Go board had it; the harness depends on it for
   uptime, heap and reboot detection. `status` reports `diag_enabled: false`
   and there is no route behind it. Heap is only observable over serial,
   which means a board that is not physically attached cannot be monitored.
2. **No `X-Nanacoin-Health` response header.** The harness reads free heap,
   in-use bytes and GC count from it for per-response samples at no extra
   traffic cost. Nothing is emitted, so `health` is empty on every request
   and the report's heap plots are blank.
3. **96-byte description limit** vs 200 accepted by the Go board. Correctly
   rejected with 400, but it is a client-visible difference.

(1) and (2) are the ones worth closing — without them the only memory
evidence is a serial cable.
