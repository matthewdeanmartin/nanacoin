# Fix the JSON ledger: binary storage, archive, paging, running totals

Status: plan, not started. Written September 26, 2026. Before starting, export
the server state (System Info → Export Server State): the storage format
change needs one erase of the ledger partition.

## What is wrong today

1. **Storage is JSON.** Journal frames (`journal.rs encode/decode`, magic
   `NCR1`) and checkpoint rows (`journal/checkpoint.rs write/read`, magic
   `NCS1`) are `serde_json_core` text. Every record repeats every field name.
   A stored transaction is roughly 250-400 bytes as JSON against 40-80 as
   postcard (estimates; measure in step 1).
2. **Journal frames are padded.** The NVS adapter (`bin/esp32/journal.rs`)
   writes every event as a full 1 KiB blob, even when the event is 200 bytes.
   That is the single biggest waste of flash writes.
3. **History is 365 transactions, not a year.** `HISTORY = 365`
   (`domain.rs`). Older transactions are gone for good: the journal is a
   write-ahead log cleared by checkpoints, and checkpoints keep only
   `state.history`.
4. **Checkpoints copy all history every time.** One NVS blob per retained
   transaction, rewritten on every rotation (every 2,048 writes), in both A/B
   banks. Raising `HISTORY` makes every checkpoint slower, and the service is
   locked while it runs.
5. **The API caps reads at 100.** `/transactions` and account history clamp
   `limit` to 100 (`api.rs`, `client.rs`). The Economy page and the Central
   Bank report ask for 365 but only ever see the newest 100.
6. **No lifetime totals.** Reports can only add up what is retained.

Not a problem: the CPU cost of JSON. API responses are built from in-memory
structs into a different shape (`TransactionView`) and would be re-encoded
regardless of storage format. Decoding is microseconds per record next to
millisecond flash reads and tens-of-milliseconds TLS. The wire stays JSON.

## Decisions

- **Postcard for storage**, JSON for the wire. Postcard is serde-based, `no_std`
  friendly and already in the local cargo cache (1.1.3).
- **No partition-table change.** The archive lives in a new NVS namespace
  (`ncarch`) inside the existing 8 MB `ledger` partition, so `deploy.py`'s
  "application only" rule still holds after the one-time erase.
- **Pre-release policy applies:** no migration, no old-format decoder. Bump the
  magic bytes (`NCR2`, `NCS2`, `NCA1`) so old data is rejected loudly, never
  misread.

## Steps

### 1. Measure first (small)

Add sizes to `/diag/database`: encoded bytes per event and per transaction,
JSON vs postcard, on real data; checkpoint duration; NVS entries per record.
Keep the numbers in this spec.

### 2. Binary storage

- Add `postcard` (default features off, `use-std` or heapless as needed).
- `journal::encode/decode`: postcard, magic `NCR2`. Frame buffer stays 1 KiB.
- Checkpoint `write/read`: postcard, magic `NCS2`.
- Remove `skip_serializing_if` from every type that is stored (`Command`
  fields in `domain.rs`; `EconomicDetails`). Postcard has no field names, so
  a skipped field shifts every later field. The API then shows `null` for
  those fields; check the Angular models tolerate it.
- Check `fingerprint(&command)`: if it hashes the JSON form, keep it on JSON
  or move it to postcard, but decide explicitly (it only needs to be stable
  within one schema).
- Audit for `untagged` / `flatten` / internally tagged enums (unsupported).
- Round-trip property tests for every stored type.

### 3. Write only used bytes

NVS `append` stores `frame[..12 + len]`; `read` accepts 12..=1024 bytes and
zero-fills the rest. The file journal keeps fixed 1 KiB frames (it seeks by
index). Expected: 3-5x fewer journal flash writes on the board.

### 4. Archive (cold storage)

Transactions never change after they are written; the "reversed" flag is
derived from the reversing transaction. So each one is written exactly once.

- **Ordinal:** every recorded transaction gets `ordinal = state.transactions`
  before increment (messages included). Transaction ids are not monotonic
  (forex cash legs use `sequence + MAX_RECORDS`), so ordinals are the key.
- **Pages:** postcard-encoded transactions packed into ≤4 KiB blobs
  `p{page:05x}` in namespace `ncarch`. Only full pages are written, plus the
  final partial page at flush time. A rewrite of the same page key after a
  crash is harmless (same content).
- **Head:** `archived_count` (ordinals below it are durable in the archive),
  stored in the checkpoint header.
- **Flush point:** before writing any checkpoint, archive every unarchived
  transaction. So `archived_count >= checkpoint.transactions` always, and no
  transaction is ever in neither place.
- **Trigger:** rotate when journal records ≥ 2048 **or** unarchived
  transactions ≥ 1024, so RAM always still holds everything not yet archived
  (`HISTORY` must be ≥ the trigger, with margin for two-leg forex events).
- **Restore:** read the checkpoint (no history rows any more), load the last
  `HISTORY` transactions from the archive tail, then replay the journal; skip
  replayed transactions whose ordinal < `archived_count`. Recompute
  `reversed` flags for the loaded window from `reverses`.
- **Capacity:** 8 MB NVS ≈ 258k 32-byte entries. Budget: journal (both
  banks) + checkpoint (both banks) + archive. With binary records and trimmed
  frames, expect tens of thousands of archived transactions. When the archive
  namespace nears its budget, drop the oldest pages (it is a ring) and say so
  in diagnostics. Measure on hardware before fixing numbers.
- **Desktop:** `FileJournal` gets an append-only `.archive` file of the same
  pages.

### 5. History 3,000 as a cache

`HISTORY = 3000` in RAM (a few hundred bytes each: about 1 MB of PSRAM).
Checkpoints stop storing history rows, so their size and duration no longer
depend on `HISTORY`. Reversals and offer undo still need the original in RAM
(as today, just with a bigger window).

### 6. Paging

- `GET /transactions?limit=N&before=<ordinal>` → `{ transactions, next_before,
  circulation }`. `limit` ≤ 100 per page; `next_before` is null at the start of
  the archive. Serve from RAM while the cursor is inside the window, then from
  archive pages.
- Same cursor for `/accounts/{id}/transactions`, which filters; a page may
  return fewer rows than `limit` plus a cursor, bounded by pages scanned per
  request.
- For cold records, `reversed_by` is computed from the reversals in RAM and in
  the same page. A reversal can only be made while its original is in RAM,
  so the gap is small; document it.
- Angular: `ledger(limit, before)`; Economy and Central Bank load pages until
  their period is covered (with a "load older" control), and the demo backend
  implements the same cursor.

### 7. Running totals

Lifetime counters in `State`, updated in `record_transaction` (so replay
rebuilds them) and stored in the checkpoint header: coins issued (to Nana, to
members, for lotto interest), retired, corrections; dollars recorded; Nana's
exchange coins and dollars in and out; interest received and paid; lent and
repaid; bought and sold; paid out and received. Reversals book against the
original category with a minus sign (same rule as
`nanacoin_ui/src/app/economy/central-bank.ts`). Saturating `i64`. Served by an
authenticated `GET /api/v1/totals`; the Central Bank report shows "all time"
next to the period view.

### 8. Board rollout

1. Export server state (System Info → Export Server State).
2. `make check`, then `make firmware`.
3. Erase the `ledger` partition once (`esptool erase_region 0x410000
   0x800000`), with the owner's go-ahead. `deploy.py` refuses erase flags by
   design, so this is a separate, explicit step.
4. `make deploy PORT=…`, provision, re-enter from the export.
5. `make probe-board`, then `scripts/perf-board.py` and
   `scripts/transport-board.py`; record checkpoint duration and NVS use.

## Tests

- Round trips for every stored type; corrupt magic, CRC, length rejected.
- Crash points: after journal append before archive; mid-archive page write;
  after archive before checkpoint commit. Restore must give the same state and
  no duplicate or missing transaction.
- Paging: cursor across the RAM/archive boundary, empty archive, forex
  two-leg ordering, reversals across pages.
- Totals equal a recomputation from the full archive.
- The existing allocation tests must still see zero allocations after startup.
