# Checkpoints, journal retirement and economy reset

The board no longer has a 4,096-change lifetime limit. Before appending a new
change when the active log contains 2,048 records, the service checkpoints
its current state and starts a fresh log. Nana can also close the journal
explicitly from **Household → Journal and economy**. Normal financial changes
remain durable before acknowledgement; this is not daily write batching.

## What survives closing the books

The checkpoint contains both currencies' balances and issuance accounts,
household settings, users and private password verifiers, per-user retry
watermarks, all listing/offer/quote slots (including outstanding settlement
deadlines), monotonic IDs and time, lifetime transaction count, the recent
365-transaction ring, and up to 4,096 recent HTTP retry receipts. The private
storage serializer is separate from the public API serializer: credentials
are never added to HTTP state responses.

Old detailed journal events are reclaimed. Closing books is **not an audit
archive or backup**. The recent history window and balances survive; the app
cannot reconstruct older descriptions after retirement. There is no historical
export/import feature in this change.

## Publication and recovery

There are two storage generations. Under the existing service mutex:

1. Clear only the inactive generation.
2. Serialize bounded, versioned rows with checksums into that generation,
   checking each row by reading it back. Flush/commit the checkpoint.
3. Durably publish a small checksummed head record selecting the generation.
4. Reclaim the old generation's log and checkpoint.

No live state is changed before durable publication. Failures, including
ambiguous acknowledgements, latch the running service unavailable until
restart. Restart follows the committed head and fails closed if its data is
missing or corrupt; it never silently rolls back to retired history.

The ESP32 uses the existing 8 MiB `ledger` NVS partition, namespaces
`nanacoin` (legacy/bank zero), `ncnext` and `ncmeta`. No partition layout
change is needed. NVS provides the atomic blob publication and physical
page reclamation. Snapshot rows share a batch commit before the head is
published, although NVS may program flash before that commit. Normal event
appends still commit individually. Every 32 checkpoint rows the firmware
allows other tasks to run.

Desktop storage retains the original journal as its exclusive lock anchor
and bank zero. Companion files are `.bank1`, `.checkpoint0`, `.checkpoint1`
and `.head`; `.head.next` is an unpublished staging file. The checkpoint and
empty new log are flushed before atomic head replacement (directory sync on
Unix, write-through replacement on Windows). Back up the original journal
**and its companions together while the server is stopped**. Do not delete
the head to "repair" a household: that discards its generation identity.

Old journals open unchanged as generation zero. Once checkpointed, do not
downgrade to firmware that knows only the old journal format. No existing
journal or connected board is automatically migrated by building the code.

## Memory and IDs

Checkpoint rows have a 2,048-byte bound; no full-state JSON buffer or second
State is allocated. Serialization and reset use existing capacities. Reset
clears State and authentication arrays in place rather than copying State
through the HTTP stack. The retry ring is allocated once for 4,096 receipts
(hashes, actor, sequence and timestamp); it overwrites oldest receipts.
NVS/HTTP may still allocate internally. Flash capacity accommodates the old
generation while the replacement is being written; rotating at 2,048 rather
than 4,096 records preserves room for checkpoints and NVS overhead.

Transaction IDs remain opaque. Legacy forex cash-leg IDs were event ID +
4,096. Event sequences now skip alternating blocks of 4,096 IDs so new events
never collide with old or future cash legs. IDs stay within the browser's
exact integer range and do not reset on checkpoint. A full economy reset
starts IDs over in a new generation.

## HTTP retries across retirement

Use `Idempotency-Key: g<generation>:<random-key>` (maximum 80 bytes). Obtain
the generation from public `/status`'s `journal_generation` or the exposed
`X-Nanacoin-Generation` response header. Capture it when creating an operation;
never change the key when retrying that operation. Angular implements this.

A retained receipt still deduplicates an old-generation key and checks the
command hash. If the response depends on history that has already expired,
the server can return a stale/not-found response without moving money again.
An unknown key from a retired generation is rejected as `stale_request`, not
treated as a new payment. Refresh the client before starting a new operation;
do not turn an uncertain old payment into a new key automatically. Legacy
unprefixed keys are generation zero, so older clients must be updated after
the first checkpoint. The 2,048-record rotation interval is smaller than the
4,096-receipt ring, preventing eviction of a current-generation receipt.

## Reset economy

`POST /api/v1/admin/reset` is Nana-only. It requires the exact confirmation
`RESET ECONOMY` plus the generation and sequence the administrator reviewed.
Any intervening change rejects the request. Reset publishes an empty
checkpoint using the same protocol, then clears all sessions, pending login
codes and authentication failure counters. Accounts, coins/USD balances,
listings, offers, quotes, settings and history are removed. Provisioning is
available again. Firmware, network configuration and TLS certificates remain.

This is a logical reset, not forensic secure erasure. A disconnected response
can leave the browser uncertain even if reset committed; reloading and reading
public status resolves whether setup is needed. The UI does not blindly retry
an ambiguous reset. No reset is performed by tests against a physical board.

## Verification and remaining hardware validation

Host tests cover 9,000 changes with automatic rotation, restart across both
file banks, balance/credential/history/offer-deadline preservation, retry
deduplication and expiry, corrupt checkpoints, injected failures before each
row/publication and after publication, reset authorization/concurrency/session
revocation, and allocation-free application checkpoint/reset paths. Angular
tests cover confirmation cancellation/mismatch, guarded reset payloads,
account clearing only on success, and generation-aware keys.

These tests and an ESP32 release build do not substitute for physical
power-cut tests or runtime stack/latency measurements. No board was flashed,
reset or used for these tests.
