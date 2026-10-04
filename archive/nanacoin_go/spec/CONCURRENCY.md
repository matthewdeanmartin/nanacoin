# Board concurrency

The router and adapter both use four workers. The TCP pool remains eight
connections, with 2 KiB transmit and 1 KiB receive storage per connection.
The adapter uses 8 KiB total; four 2 KiB API record buffers add another 8 KiB.
All of these buffers are allocated at startup. Router metadata, goroutine
stacks and transient request allocations are additional costs.

The first four-worker boot with the fixed ledger exhausted RAM when creating
the old 4 KiB-per-connection transmit buffers. Reducing them to 2 KiB saves
16 KiB while retaining all eight connections and the 1 KiB streaming chunks.

Transfers, purchases and other state mutations still validate and commit
under one service mutex. Eight fixed idempotency lock stripes cover cache
lookup, execution and receipt publication. Matching user/endpoint/key tuples
cannot execute concurrently; hash collisions merely serialize unrelated
operations. Callbacks must not nest Idempotent. Cache retention is still
bounded (16 receipts / 4 KiB); old evicted keys are not deduplicated forever.
Failed operations remain retryable. If receipt persistence fails after a
successful operation, the receipt is still cached in RAM. The operation and
receipt are separate journal events, so crash-atomic deduplication remains a
future persistence concern.

Users, listings and transaction lists encode one record into owned storage
under the service mutex, then release it before sending any record bytes to
the network. The next record reuses the same buffer. No arena aliases survive
into the socket write. Encoding overflow stops the stream and is logged;
the buffer never grows. Current field-limit and escaping tests cover capacity.

These are live lists with individually consistent records, not whole-page
snapshots. Balances alongside a list can reflect a different instant.
Transaction traversal captures its initial sequence range, excludes later
appends, and skips records evicted before they can be copied. Listing traversal
skips slots recycled since the walk started and rechecks the status filter.
Users added after a walk starts are excluded. Callbacks that prepare records
still run under the service lock and may use only the Locked accessors;
the optional send callback runs outside the lock.

Browser and Angular request concurrency is unchanged. Tests cover overlapping
retries, writes during blocked streams, ring reuse during traversal, and HTTP
socket blocking on all four affected list routes. Desktop Go race checks cover
the shared code; they do not instrument TinyGo or the board network driver.

Hardware deployment exposed a stack issue in the earlier ring implementation:
CheckInvariants compiled to a 12,336-byte frame on an 8,192-byte stack. Keeping
the same startup capacities behind pointers reduced that frame to 112 bytes.
Index iteration similarly reduced Arena.freeRun/Put frames from 5,904/5,936
bytes to 32/48 bytes. These sizes came from the matching ESP32 ELF's DWARF
frame information; TinyGo's built-in print-stacks failed on DWARF version 3.

## Hardware smoke check, 2026-09-18

The corrected 928,864-byte firmware was flashed and verified on COM8 and
booted at 192.168.1.158. Setup reported 13,072 free heap bytes. Two waves of
four simultaneous status requests all returned 200 with valid JSON in
125–172 ms. The status reports a 512-transaction capacity and balanced ledger.
The final diagnostics reported zero allocation failures. Observed response
header free heap reached 4,656 bytes; RAM remains a likely next bottleneck.
This is a short unprovisioned-board smoke check, not a sustained authenticated
load certification. The household was left unprovisioned for interactive testing.

All internal Go packages passed `go test -race ./internal/...` using the local
UCRT GCC toolchain. Raw HTTP evidence is in the gitignored
`tools/boardprobe/runs/concurrency-2026-09-18/http-smoke/events.jsonl`.
