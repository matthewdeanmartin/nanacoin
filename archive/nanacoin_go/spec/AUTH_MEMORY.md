# Authentication memory and regression checks

Authentication tables now allocate their storage once. Package defaults remain
64 sessions, 16 pending PKCE codes and 64 failed-login counters. The ESP32 profile
reserves 32 sessions, 8 pending codes and 16 failure counters. Active sessions/codes are never evicted
for new logins. Expired/revoked slots are reused. Failure counters retain the
existing soonest-expiring eviction policy at capacity. Tables hold SHA-256
hashes instead of retaining request-owned token, challenge, redirect and username
strings. User IDs still refer to existing household strings.

On ESP32-S3 the three slot arrays occupy 4,096 + 1,920 + 3,072 = 9,088 bytes,
plus Store metadata and shared scratch buffers. This is a startup cost, not free
memory: check the boot heap and concurrent-request low-water mark after flashing.
The first hardware run confirmed OOM after five connections with these full
ceilings. The board now reserves 3,776 bytes (2,048 + 960 + 768) instead. Its
32 sessions allow two sessions per maximum 16-person household. This is a
reduction from the old 64-session ceiling; four HTTP workers, eight TCP
connections, and the 512-entry ledger are unchanged. The smaller failure table
also means username churn can evict failure counters sooner. Configured pool
limits are fixed at startup, never dynamically grown.

Password verification uses fixed decode buffers and a mutex-protected workspace
for the single-block PBKDF2-HMAC-SHA256 format. The SHA compression code is
adapted from Go 1.26.5's generic implementation, with its BSD license retained
in `internal/auth/LICENSE.go-sha256`. The wrapper and message schedule use fixed
shared storage. This avoids heap-backed construction, finalization padding and
state-serialization buffers in the current standard-library/TinyGo combination.
There is no dependency on a private serialized digest layout. Differential tests
compare SHA output with the standard library at every length 0..4095 and compare
PBKDF2 output with x/crypto for
1, 2, 1,000 and 20,000 iterations and passwords on either side of the HMAC block
boundary. The existing 1,000-round work factor and 16-byte random salts are
unchanged. Hash computation is serialized; this bounds scratch memory while
ordinary reads can continue. Hashing a new password still allocates its returned
verifier. Unknown-user checks do equivalent hashing without generating a verifier.

Fixed salt/key lengths and a maximum of 1,000,000 rounds reject damaged stored
verifiers before unbounded work or empty-key acceptance. Existing 20,000-round
verifiers remain supported. Final password comparison remains constant-time.

Token output strings and returned login session snapshots still allocate. The
HTTP bearer lookup uses caller-owned Session storage; it does not expose a live
slot. Short string hashing and token generation share fixed scratch buffers
because TinyGo's escape analysis heap-allocated local buffers that desktop Go
kept on its stack. Inputs over 128 bytes retain a temporary hashing conversion.
New-user creation, string interning, journal framing, and some API/domain copies
still allocate; this change does not claim allocation-free HTTP requests.

Correctness fixes covered by regression tests:

- Expired failed-login counters start a fresh attempt window.
- Sessions/codes expire at the deadline, including exact equality.
- User revocation clears pending codes, atomically with code redemption.
- Concurrent redemption issues at most one session.
- Returned session copies cannot corrupt a reused slot.
- A failed journal write cannot replace a user's password, including after replay.

Additional bug-oriented tests exercise randomized balance conservation and
rejected overdrafts through ledger wraps; all torn-write prefixes; independent
JSON-parser comparison; concurrent event-log snapshots; journal ownership,
failure accounting and callback reentry. Plain data-only marketplace/user types
are exercised through service and API tests instead of field-assignment tests.

Run from this directory:

```text
go test ./internal/...
go test -race ./internal/...
go vet ./internal/...
go test ./internal/api -run=^$ -fuzz=FuzzTransferParserAgainstJSON -fuzztime=10s
go test ./internal/storage -run=^$ -fuzz=FuzzDecodeFraming -fuzztime=10s
go test ./internal/auth -run=^$ -fuzz=FuzzVerifierParsing -fuzztime=10s
```

Desktop allocation assertions are regression alarms, not proof of TinyGo heap
behavior. Also compile with `-print-allocs=internal/auth`, inspect ESP32 stack
frames and run the auth Locust profile with health headers and native USB logs.
The attached board is four floors above its router: label these runs as weak-link
network plus application tests. Repeat near the router to isolate application
resilience. Do not flash old firmware just to restore development-board service.

TinyGo 0.42 / LLVM 22 on Xtensa hit `Incomplete scavenging after 2nd pass`
when failed-login bookkeeping was inlined alongside password verification.
Explicit function boundaries avoid this backend failure. Do not remove the
noinline directives based only on desktop benchmarks. Diagnostic miniature
builds isolated this before flashing; it was a compiler failure, not a board
crash. SHA rounds also use explicit output storage to bound live intermediates.

Validation before flash (2026-09-18): all internal Go tests and race tests pass;
`go vet` passes. Bounded fuzz runs completed 119,343 JSON cases, 2,521,562
record-framing cases and 2,574,157 verifier-parser cases without failure.
Compiled ESP32 CFI frames: RedeemCode 656 bytes, IssueCode 304, session creation
272, hashString and rate-limit methods 208-240, derivePassword 192, shaInto 144.
These are individual frames, not a claim about the entire call-chain stack.
The latest TinyGo auth allocation report lists startup tables, returned session
snapshots, new-password construction, and RandomToken's non-32-byte fallback;
it no longer lists the ordinary token-hashing scratch arrays.

The firmware prints `auth self-check` before constructing Wi-Fi/router pools.
It generates a disposable test verifier and checks it, reporting hash and verify
allocation deltas from TinyGo's runtime counters. This also exercises crypto
initialization before the board's runtime memory is crowded. It neither creates
a household nor writes flash. Capture this line whenever changing authentication.

The third image completed provisioning and Nana login but still ran out of RAM
creating the first member. To make fixed authentication pools fit without
shrinking history, CheckInvariants now reconciles 32 intern references per pass.
Its scratch buffer drops from 4,096 to 256 bytes (3,840 bytes saved). It still
checks all 512 reference slots and all retained postings, in sixteen passes.
Tests cover scratch boundaries, late references, invalid refs and ring wraps.

A preexisting listing heap test proved flaky in five isolated repeats: identical
work alternated between -208 and +4,872 process-wide heap bytes. Its heap figures
remain diagnostic; deterministic assertions now check retained listing count
and that listing IDs/text do not consume permanent intern slots.

The fourth image progressed farther but still OOMed during fixture setup. The
service ledger now reserves 33 compact lifetime account records (32 domain
accounts plus system issuance), instead of balance/opening arrays indexed by
all 512 intern strings. Each record stores its intern reference and two amounts;
the default standalone Book still supports 512 distinct accounts. The service
saves about 7.2 KiB without reducing its account/user limits. Account capacity is
validated before append/replay can evict history. Differential tests compare
compact and full-capacity books through repeated ring wraps and high intern refs.

The on-device self-check isolated two remaining verification allocations:
base64.Strict() cloned its 328-byte encoding twice. The strict decoder is now
constructed once. Subsequent serial radio samples repeat the boot verification
allocation counters so missing the initial USB banner does not lose this data.

## Final attached-board validation

The final image reports 20,928 bytes free after setup. Native USB counters report
password hashing: 3 allocations; password verification: **0 allocations / 0 bytes**.
The final ELF's inspected frames include CheckInvariants 96 bytes, SHA compression
96, SHA wrapper 64, VerifyPassword 176, and RedeemCode 656. These are per-function
frames, not summed call-chain bounds.

Household provisioning, two members and funding now complete. A fresh-boot auth
Locust ramp (1/2/4 users, 20 seconds each) completed 100 full PKCE login/token/logout
cycles: 300 requests, zero failures, about 4.90 requests/s overall, p50 201 ms,
p95 653 ms, p99 1,210 ms. The minimum sampled free heap was 1,440 bytes. This is
a weak-link smoke test, not an endurance or maximum-throughput certification.

A later check found an unexpected restart and an unprovisioned household. The
owner confirmed no manual intervention. Uptime dates the reboot to about 23
seconds after the auth run finished, near serial monitor shutdown. A separate
serial open/close experiment did not reproduce it; the cause remains unknown.
The successful request counts apply only to the measured run, not subsequent
stability. This build cannot retain crash records across resets.

A separate E2E run passed transfer, reversal, purchase-retry and seller-payment
checks, then hit a serial-confirmed OOM during four concurrent same-key issuance
requests. Two requests returned 201 and two timed out. The remaining E2E checks
were not completed. The next target is peak memory during concurrent writes;
this change does not claim the entire server is OOM-free.

Reports and exact workload conditions are in ../nanacoin_load/FINDINGS.md.
