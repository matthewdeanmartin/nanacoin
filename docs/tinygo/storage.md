# Storage, commit processing and retries

The Rust board has a different, persistent adapter. See
[NVS keys, recovery and retirement](../rust/storage.md) for its actual namespace
and numbered-key layout, checkpoint publication and durable retry receipts.

Three different records can exist around one request: a transaction describing
money, a journal event describing a state change, and a retry receipt describing
the response already given to a client. They serve different purposes.

## The ledger is a retained window

The ledger reserves 365 transaction slots. It fills them and then overwrites
the oldest, while preserving lifetime balances and increasing transaction IDs.
This is a ring buffer. A power-of-two capacity is not required by its indexing;
365 is a capacity choice, not a mathematical hazard.

Variable text shares a 12 KiB arena with other domain data, so long memos can
force history eviction before all transaction slots are occupied. An old slot
can hold a new transaction, but a sequence check distinguishes the new record
from the former occupant.

Opening-balance checkpoints retain the effects of evicted transactions.
They are also in RAM. They solve history retention, not power-loss persistence.

The diagnostic event log follows the same fill-then-overwrite idea with 58
entries. Live accounts and active listings do not: they must remain available
or a new operation must fail cleanly.

## Commit processing

A commit turns a validated request into a coordinated domain change:

```text
Check permission, inputs and capacity under the service lock
                         |
                  Form a typed event
                         |
           Check encoded size / append to journal
                         |
                 Apply the event to RAM
                         |
                    Return result
```

The journal-first ordering means an append failure must not leave a successful
RAM-only mutation behind on a retaining backend. Validation and capacity
preflight also matter: after an event has been recorded, applying it should
not unexpectedly discover that a required store is full.

For a retaining backend, the service encodes into a startup-allocated buffer
with a 4,352-byte event ceiling. It applies the already-typed event directly;
it need not encode and then decode its own live operation. Replay reads stored
events and feeds the same application logic.

The storage contract provides `Append`, `Replay`, `Size` and `Close`. The
concrete backend determines whether an accepted append is persistent.

| Backend | What survives restart? | Purpose |
|---|---|---|
| Retaining memory journal | Nothing after process loss | Replay and failure tests without disk |
| Desktop `flashlog` file | Committed records in the file | Desktop persistence and framing tests |
| Board discarding journal | Nothing | Run within the current board budget |

The board's discarding backend can skip serialization while still validating
event sizes and maintaining sequence/byte accounting. Its `journal_used`
value describes cumulative logical journal bytes; it is not occupied flash
or a growing RAM allocation.

## What the desktop file protects

Despite its name, `flashlog` is currently a file backend, not an ESP32 flash
driver. Records contain framing, a sequence, a payload, a checksum and a commit
marker. Replay accepts the valid committed prefix and rejects a torn tail.
Tests truncate records at different byte positions to exercise interrupted
writes.

That format is useful groundwork for flash persistence, but does not mean the
board has it. Backend recovery, available capacity and retained history are
separate contracts.

## The retry cache

Suppose Alice sends Bob 10 coins. The board commits the transfer, but WiFi
drops before Alice receives the response. Retrying without an operation
identity could pay Bob twice.

An `Idempotency-Key` gives that intended operation an identity. A successful
request stores a receipt under the user, endpoint and key. A retry within the
retention window returns the recorded result instead of applying the money
operation again.

The current cache has 16 entries and a total 4 KiB response-body budget. Either
limit can evict the oldest receipts. Keys have an 80-byte limit. Overlapping
requests for the same operation are coordinated so the second cannot slip
between the first mutation and publication of its receipt. Reading a receipt
copies its body into caller-owned storage so later eviction cannot alter an
in-flight response.

Clients must reuse a key only for retries of the same intent. Changing the
amount while retaining the key can return the earlier response. Use a new key
for a new action. An omitted key bypasses this retry protection. Failed business
operations are not retained as successful receipts.

This is bounded deduplication, not permanent exactly-once delivery. After
eviction or a board restart the cache cannot recognize the old operation.
Also, the economic commit and receipt recording are separate steps: a future
persistent deployment must account for the crash gap between them. Do not
claim that reboot-safe retry behavior follows simply from having a journal.

## Future flash batches

Flash programming and flash erasing are different operations. Erasing wears
sectors, and one HTTP request is not inherently one erase cycle. The owner's
conservative design budget is 100,000 erase cycles; it is not a verified rating
for the installed part and not a global counter of allowed requests.

The intended direction is batched persistence with rotation, rather than a
flash update on every request. A compatible implementation needs:

- A batch handoff before a RAM ring overwrites unpersisted data.
- Self-contained serialized values, not pointers or arena offsets that can be reused.
- Checkpoints sufficient to restore balances and live state when older events are absent.
- Sequence and commit markers to identify complete batches after interrupted writes.
- An explicit policy for a full or failed persistence queue.
- Sector rotation and measured erase accounting.

Copying a raw ledger array is insufficient: its text references belong to
another mutable RAM store, and the ledger alone does not describe users,
credentials or all marketplace state.

Batching creates a power-loss window. Decide how much recent state can be
lost before selecting a flush interval or a "flush when full" policy. None
of this board persistence is enabled merely by using RAM rings today.

For implementation details, see
[RAM history](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/RAM_HISTORY.md)
and [write memory](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/WRITE_MEMORY.md).
