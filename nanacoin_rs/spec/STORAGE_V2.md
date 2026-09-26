# Storage v2 and ledger APIs

Implementation: September 26, 2026. This describes the code, superseding the
proposal details in FIX_JSON.md and FIX_JSON_REVIEW.md. No migration or old
decoder: existing development ledger data must be reset for this format.

## Three implementation increments

1. Immutable transaction facts, correction annotations, partial refunds,
   business audit, currency epochs and commerce APIs.
2. Postcard journal/checkpoints, used-length NVS writes, committed archive ring,
   bounded paging and exact lifetime counters.
3. Fault/replay/API/allocation tests, firmware build and authorized board rollout.

HTTP remains JSON. Journal frames use NCR2, checkpoint rows NCS2 and archive
pages NCA2. CRCs include interpretation-critical headers and payloads. Decoders
reject extra/truncated bytes. Postcard uses caller-owned buffers. Credentials
remain private checkpoint fields; command fingerprints now hash postcard bytes.
The current Rust schema is positional: changing field/enum order requires a
storage schema change. There is no promise of compatibility with these bytes
before the first real user.

## Immutable facts and corrections

Stored transactions never rescale or gain a mutable reversed flag. Metadata
records ordinal, event group, money epoch, decimal precision, gift/art reference
and original refund units. Event sequence and historical public tx IDs remain
opaque; the existing disjoint second-leg ID scheme is retained in this release.
Both forex and loan legs carry the same group. Account/person separation and
generalized assets remain future work; current native/USD accounts still apply.

Currency reform rescales current balances, obligations and commerce prices,
not historical payments. Up to 32 currency epochs store exact conversion
exponents. Cash postings in transaction responses are in the transaction's
original units; API consumers must honor metadata.epoch/decimals and the
conversion definitions in totals. current_postings is an exact current-unit
projection (null when conversion would round/overflow); current_money_epoch
identifies that view. The existing Angular client uses this projection and
preserves original_postings, rejecting an inexact view rather than displaying
misleading totals. A cross-epoch refund specifies original
minor units, converts exactly into current units, and rejects fractional or
overflowing conversions before writing. No silent rounding.

Corrections live in a separate checkpointed table, so a cold original's
reversed_by remains available after the correcting transaction leaves RAM.
Capacity is 4,096 distinct corrected originals; exhaustion rejects another
new correction, never evicts protection against duplicate refunds. Partial
refund amounts are accumulated in the original units. Complete refund marks
the original reversed and updates fulfillment/offer status. Partial refunds
leave fulfillment pending. Offer undo retains original economic classification
and amount outside history; forex corrections preserve the quote reference.
Forex legs remain individually correctable by Nana; grouped whole-trade undo
is not introduced here.

`POST /api/v1/transactions/tx-ID/refund` accepts:

```json
{"amount":2500,"reason":"Returned half the order"}
```

Authentication and the usual Idempotency-Key are required. Amount is positive
and expressed in the original payment's minor units. The recipient or Nana
can initiate it, but an ordinary refund cannot overdraw the recipient. The
sum cannot exceed the original payment. Refunds cannot themselves be refunded.
Issuance, USD, forex, loan, lotto and art payments use their specialized rules,
not this partial-refund endpoint. Generic art corrections are rejected because
ownership must also be returned; negotiated returns remain future work.

Refund authorization requires the original in the 3,000-record cache or a
retained fulfillment obligation. Reading cold history does not grant indefinite
refund rights. Offer undo uses its separately retained settlement and deadline.
Missing originals return not_found. The correction table remains authoritative
even after both original and refund have left the cache.

## Archive, checkpoints and recovery

On real file/NVS adapters, checkpoints omit transaction and audit history rows.
Archive pages are sealed at rotation. A checkpoint stores first/next page,
exclusive archived transaction count, archived event sequence and incarnation.
Only pages selected by the committed checkpoint head are visible. Restore loads
that history and replays the subsequent journal; loading archive rows does not
apply balances, increment totals or consume retries.

The ring has 1,024 physical slots, each at most 4,096 bytes. A committed head
retains at most 768 pages, leaving 256 for staging. Staging refuses to overwrite
any page still referenced by the committed head. Pages written before a failed
head publication are ignored. A lost acknowledgement after publication restores
the new head. Committed page corruption or a missing suffix fails closed.
Ring reuse verifies logical page ID, incarnation, length and CRC.
The committed transaction retention floor also trims the live cache. A command
is revalidated after automatic rotation, so it cannot depend on an original
that was just pruned and would be unavailable during restart/replay.

Rotation occurs before a new event when journal records reach 2,048, unarchived
transactions reach 1,024, or pending audits reach 1,024. The RAM history holds
3,000 transactions; audit holds 1,024 events. Two-leg events are never split
across checkpoints. HTTP/scheduler writes use the same commit path. Normal
payments remain durable before acknowledgement, not merely at rotation.

NVS uses ncarch in the existing 8 MiB ledger partition. Logical page capacity
is not a claim about physical erase/write cost; NVS overhead and both checkpoint
banks must fit too. File storage uses a bounded, synchronized .archive companion
of up to 4 MiB. Back up all companions consistently while stopped. Test-only
Journal adapters without archive support retain bounded history in checkpoints.

Economy reset publishes an empty checkpoint with a new incarnation before
reusing archive slots. Old pages are ignored, not secure-erased. Old cursor
incarnations and normal retired-generation retry keys are rejected. A physical
ledger erase also erases its identity; this is development reset behavior,
not a federation identity protocol.

## History and audit APIs

`GET /api/v1/transactions?limit=100` returns transaction views, circulation,
state_sequence, snapshot_upper, next_before, next_cursor, history_truncated and
archive_first_page and oldest_available_ordinal. Continue with
`?cursor=<next_cursor>&limit=100` until null.
Each response contains at most 100 rows and scans at most eight archive pages;
an empty page can still have a continuation. The cursor fixes the upper
transaction ordinal and excludes later payments. Correction annotations and
circulation are current at state_sequence, not a historical snapshot of mutable
state. Follow next_cursor rather than reconstructing cold cursors from IDs.

Account history accepts the same cursor, keeps account and balance fields,
filters postings to that account, and retains existing owner/Nana authorization.
The archive is finite: history_truncated signals pruning; stale cursors return
stale_request instead of silently crossing a reset or reclaimed page range.
The raw `/state` response no longer embeds history; use bounded history routes.
Individual transaction-by-ID lookup remains a recent-cache lookup.

`GET /api/v1/audit` is Nana-only. It retains business commands including offer,
fulfillment, loan, lotto, reform and commerce changes. Credential-bearing
commands become sanitized identity-change records. The response has audit,
next_before, next_page, incarnation and truncated; continue with before, page
and incarnation together. Up to 16 audit rows/eight archive pages per request.
This is bounded retained audit, not a permanent off-board backup. Audit does
not create cash flows or inflate transaction counts.

## Exact totals

`GET /api/v1/totals` is authenticated. It returns categories, epoch rows,
treasury (member 1) and state sequence. Each epoch has decimals, conversion
exponent and values aligned with categories. Values are signed decimal strings
from checked i128 counters. No floating-point arithmetic or saturation occurs.
The bounded event sequence, maximum amount and maximum legs keep lifetime
flows within i128. Issuance corrections remain separate; other refunds subtract
from the original economic category. Zero-value messages do not count as cash
transactions. Cash interest is distinct from accrued loan obligations.

Counters are checkpointed and replayed independently of archive retention.
Consequently all-time totals survive pruning, but a retained archive suffix
alone cannot recompute them. Tests compare against independently known complete
workloads before/after pruning/restart. This release does not add monthly or
retention-floor opening summaries. Cross-epoch totals must be converted exactly
before adding; native totals from different epochs are not interchangeable.

## Commerce and UI scope

See [COMMERCE_API.md](COMMERCE_API.md) for gift requests, unique art editions,
atomic purchase, gifting and profile equipment. These are API-only features.
No new screens/buttons are added. The existing ledger client follows cursors
for requests above 100 rows, bounded to 3,650, and rejects a currency reform
between pages. General corporations, federation, memo
visibility and art-return/royalty protocols remain roadmap work. Clients doing
historical accounting must use the new units/coverage metadata; raw original
amounts must not be treated as current-epoch amounts after reform.

## Diagnostics and validation

`/diag/database` identifies the codec, archive boundaries/capacity, trigger and
JSON/postcard sizes of a retained sample transaction. Reserved model memory now
includes corrections, epochs, audits, commerce and paging buffers. A blank board
has no sample; host/board timing and NVS consumption remain separate metrics.

Tests cover CRC/length corruption, all existing command families through
postcard, no-allocation hot paths, refunds and ownership permissions, replay and
checkpoint persistence, old offer/fulfillment corrections, currency reform,
archive ring wrap, staged orphans, lost checkpoint acknowledgement, corruption,
cursor snapshots and exact totals after pruning. Physical repeated power-cut
endurance testing is not implied by passing those deterministic fault tests.

### Board rollout — September 26, 2026

The ESP32-S3 on COM9 was deployed with the current Rust server and Angular
bundle. The verified partition layout permitted a ledger-only erase at
`0x410000`, length `0x800000`; Wi-Fi/TLS storage and the partition table were
preserved. The application was flashed at `0x10000` with hash verification.
Firmware size is 3,157,168 bytes of the 4,194,304-byte application partition;
SHA-256: `DB00FD70DFB4322EF40790C6A38EAE9E96F190F93678FAB2F913BAA9BE9DF726`.

The strict probe passed certificate/hostname validation, exact Angular bundle,
balanced empty ledger, notebook, health and CA checks. Six read-only performance
requests returned HTTP 200. The board was left unprovisioned with zero
transactions for manual setup and re-entry. Live payments, archive/checkpoint
writes and physical power-cut recovery were not exercised on this empty board.

Observed free PSRAM was 5,247,208 bytes of 8,385,712 bytes. Internal heap free
was 37,311 bytes initially and 32,687 after read-only requests, with a 14,895-byte
low-water mark. These are observations for this workload, not an endurance or
worst-case memory guarantee.
