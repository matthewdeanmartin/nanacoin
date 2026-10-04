# Bounded history and future persistence

The ledger preallocates 365 packed transactions and reversal links. Sequence
numbers continue increasing across wraps; replacing a slot never reuses an ID.
Evicted postings are folded into opening balances, so current balances and
circulation include all transactions. Invariant checks reconcile opening
balances plus retained postings without allocating a history-sized buffer.

History endpoints return only the retained window. Evicted records cannot be
looked up or reversed. Status reports `transactions` (lifetime total),
`retained_transactions`, `transaction_capacity`, and `oldest_transaction`.

The shared 12 KiB text arena recycles blocks when their owning transaction is
evicted or a domain field is replaced. Long memos can shorten the retained
window below 365 entries. Domain text remains live; if domain data alone
fills the arena, the existing text-truncation policy still applies. Numeric
reversal references avoid consuming an intern-table slot for every reversal.

The event/error log fills 58 slots and overwrites the oldest. The board
uses a discarding RAM journal and still forgets everything on reboot. The
desktop/test memory journal is a replay fixture, not the board history buffer.
No flash writing or durability is introduced by this change.

## Capacity/headroom adjustment (2026-09-18)

Reduced transaction history from 512 to 365 and event history from 64 to 58
(9.375%, the nearest whole-entry reduction to 10%). Both rings use modulo
indexing and support arbitrary capacities; there is no power-of-two requirement.
Interned identities, live domain tables and the 12 KiB text arena are unchanged.
Historical measurements in AUTH_MEMORY.md/HTTP_MEMORY.md describe earlier builds.

The flashed board recovered 9,072 startup bytes: free RAM rose from 15,056 to
24,128. The ledger's packed records plus reversal links are roughly 30 KiB at
512 entries and 21.4 KiB at 365. The event slots are only about 2.5 KiB before
and 2.3 KiB after, excluding their retained strings. Other large fixed consumers
include TCP payload buffers (24 KiB), the shared text arena (12 KiB), and API
record buffers (8 KiB), in addition to task stacks and radio storage.

Full internal Go race tests passed, including repeated ring wrap and invariant
checks. The flashed image passed the E2E suite including purchase and four
concurrent same-key issuances. This short validation does not establish endurance.
See ../nanacoin_load/reports/20260918-153919-e2e-2239/index.html and
../nanacoin_load/reports/20260918-capacity-365/serial.log.

## Future full-buffer flash batches

Batch **self-contained encoded journal events**, not raw PackedTransaction
memory: intern references and text slots point into reusable RAM. The existing
service commit boundary supplies complete events before applying them. A
future fixed byte staging ring can append those frames, flush when the next
frame will not fit, and reuse the batch only after a successful durable commit.
This also covers domain events and text pressure, which may evict history
before the transaction ring reaches 365. Do not flush every rolling overwrite.

Give batches monotonic sequences, integrity checks and a commit marker; define
what happens on failed writes or power loss before acknowledging durability.
Restoring from a retained suffix needs a checkpoint of balances, sequence,
users, accounts, listings and reversal state. The current opening balances
are RAM accounting checkpoints, not a persistent snapshot implementation.
Until snapshots exist, replay requires the complete journal from its beginning.

Track erase counts per physical erase block, rotate archive blocks, and reserve
a wear margin under the owner's conservative 100,000-cycle design budget.
Capacity, actual device endurance and write amplification must be established
before enabling persistence. Batching reduces write frequency but does not
by itself guarantee a lifetime. No automatic flash flush is enabled now.

Invariant reconciliation uses a fixed 32-account scratch window (256 bytes)
and sixteen passes across retained postings. All 512 intern references remain
covered; this saves 3,840 bytes without changing history or account capacity.

The service's lifetime balances/checkpoints use a preallocated compact table of
33 account records, covering the existing 32-account domain ceiling plus system
issuance. Account references may appear anywhere in the 512-entry intern table;
they no longer force two 512-amount arrays. Standalone books retain their full
512-account default. Capacity failure is checked before history is evicted.

## Write-buffer follow-up

See [WRITE_MEMORY.md](WRITE_MEMORY.md) for fixed retry responses, typed commit
application, and the discard-journal fast path. Listing text replacement now
refuses insufficient space atomically instead of truncating identifiers, and
closed listings can be recycled when text pressure precedes the row limit.
Active listings are never evicted to make room.
