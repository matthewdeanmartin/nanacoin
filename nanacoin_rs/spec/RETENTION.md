# Checkpoints, archive retention and economy reset

The current contract is [STORAGE_V2.md](STORAGE_V2.md). HTTP remains JSON;
persisted events, checkpoints and archive records use bounded postcard encoding.
NCR2 frames, NCS2 rows and NCA2 pages checksum critical headers and payloads.
Old development data requires reset; there is no migration or old-format decoder.

## Rotation and retained state

Before a new event, the service rotates when journal records reach 2,048,
unarchived transactions reach 1,024, or pending audits reach 1,024. Nana can
also checkpoint from Household. Every accepted mutation is durable before
acknowledgement; rotation does not defer payment persistence.

Checkpoints retain balances, private credentials, settings, business objects,
monotonic IDs/time, 4,096 bounded retry receipts, correction annotations and exact
per-epoch lifetime counters. File/NVS checkpoints omit history rows: committed
archive pages restore up to 3,000 transactions and 1,024 sanitized business audits
without reposting balances or totals. Test adapters without archive support keep
bounded history rows in checkpoints.

Transactions retain original units, epoch, precision and classification.
Corrections retain original amounts and cumulative refunded units separately.
Currency reform changes current balances and obligations, not archived facts.
Transaction APIs provide original postings and exact current-unit projections;
inexact projections are null. Lifetime counters survive history pruning.

## Publication and recovery

Under the service mutex:

1. Stage immutable archive pages outside the committed ring interval.
2. Write and verify bounded checkpoint rows in the inactive bank. Its header
   records archive page interval, transaction retention floor, archived counts,
   audit sequence and incarnation.
3. Durably publish the head selecting the new bank.
4. Retire the old bank and trim cached originals below the committed floor.

The archive has 1,024 slots of at most 4,096 bytes. At most 768 pages are retained;
256 slots remain for staging. Capacity is checked before page writes. Pages
written without head publication are invisible. An ambiguous acknowledgement
after publication restores the published state. Missing or corrupt committed
data fails closed; errors latch the live service until replay. Checks include
logical page ID, incarnation, length and CRC.

Automatic rotation revalidates the pending command after pruning. A refund
cannot depend on an unpinned original that restart could no longer restore.
Fulfillment and reversible offer obligations retain their own required facts.

The existing 8 MiB ledger NVS partition uses `nanacoin`/`ncnext` for banks,
`ncmeta` for publication/policy and `ncarch` for archive pages. NVS event blobs
store only used bytes, up to 1,024; checkpoint rows are at most 4,096 bytes.
Archive writes commit before checkpoint publication. Physical capacity also
includes NVS overhead and both banks; logical capacity is not an endurance or
latency measurement.

Desktop storage keeps the original file as lock anchor and bank zero. Companions
include `.bank1`, `.checkpoint0`, `.checkpoint1`, `.head`, `.transport` and a
bounded `.archive` of at most 4 MiB. `.next` files are unpublished staging files.
Back up the journal and companions together while stopped. Archive writes and
replacement checkpoints are synchronized before atomic head publication.

## Reads and retries

The raw `/state` response omits history. Transaction/account routes return at
most 100 rows and scan at most eight archive pages per request. Follow
`next_cursor`, including after an empty page. Cursors fix an upper transaction
ordinal; annotations and balances remain current. Coverage metadata identifies
pruning. Reset or reclaimed-page cursors are rejected as stale. The Nana-only
audit API returns up to 16 rows with bounded page continuation. The archive is
finite and is not an off-board backup or export/import system.

Use `Idempotency-Key: g<generation>:<random-key>` (maximum 80 bytes). Obtain the
generation from `/status` or `X-Nanacoin-Generation` when creating an operation;
never replace its key to retry an uncertain payment. Retained receipts deduplicate
and verify the command hash across rotation. Unknown retired-generation keys
return `stale_request`. Expired response history may produce stale/not-found
without moving money again.

## Reset and validation

Nana-only reset requires `RESET ECONOMY` and the reviewed generation/sequence.
It publishes empty state with a new archive incarnation, revokes sessions and
returns to provisioning. Firmware, network configuration and TLS credentials
remain intact. Old archive pages become unreachable; this is not secure erasure.
Disposable pre-release data needs no compatibility migration.

Tests cover rotation, restart, exact totals, retries, archive wrap/pruning,
committed corruption, staged orphans, lost publication acknowledgements,
automatic-rotation refund revalidation, cursor continuation and bounded
allocation paths. These do not establish physical power-cut endurance or latency.
