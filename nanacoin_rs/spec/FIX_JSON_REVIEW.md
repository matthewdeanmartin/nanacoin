# Binary storage review and implementation waterfall

September 26, 2026. Design proposal, not implemented. This review supersedes
conflicting details in FIX_JSON.md. Root AGENTS.md applies: no migration,
legacy decoder or mandatory export/re-entry. Reset/reseed development data
when needed; this review performs no hardware erase/flash.

## Recommendation

Yes to postcard for storage, JSON for HTTP, used-length NVS writes, bounded
archive pages, paging and durable totals. Separate the codec optimization from
the archive protocol. Correct the transaction semantics before freezing the
new storage schema. A codec switch alone does not preserve old history.

Do not promise specific CPU or flash-endurance savings yet. The original
40-80-byte estimate cannot cover arbitrary 140-byte memos. Flash erase savings
depend on NVS overhead, garbage collection, fill level and checkpoint frequency.
Measure payload bytes, physical write/erase behavior where observable, CPU,
RAM, boot time and worst-case latency separately.

## Source-backed corrections and missing records

| Finding | Evidence | Required design change |
|---|---|---|
| History is mutable today. | money.rs::apply_reform rewrites retained amounts; Reverse changes reversed. | Immutable original asset/epoch amounts; derived reversal annotations. |
| Reversals can outlive RAM history. | domain.rs::Reverse also reads fulfillment.payment; offers.rs retains settlement parties. | Durable reversal lookup and pinned obligation facts, independent of cache length. |
| Offer undo loses economic classification after eviction. | UnacceptOffer reads economic from history and otherwise defaults. | Save the original classification with the accepted settlement/payment. |
| Forex correction loses its quote reference. | Reverse copies usd/economic/listing but creates quote: None; central-bank.ts classifies exchange flows by quote reference. | Carry original purpose and all applicable references on corrections. |
| Multi-leg events are not only forex. | loans.rs emits separate principal and interest payments with a 4096 offset. | Bound maximum legs for every command and group them explicitly. |
| Noncash audit is lost at compaction. | Fulfillment keeps four updates; other lifecycle commands often emit no Transaction. | Retain selected business audit records separately from cash flows. |
| Live referenced objects are recycled. | Listings, things, offers, quotes and terminal loans have bounded recyclable slots. | Retain compact definitions or bounded historical facts needed to explain old payments. |
| Export is not a full backup. | export-state.ts explicitly excludes passwords and transaction history. | Do not describe export/re-entry as lossless recovery or require it for this reset. |

The monetary paths reviewed include issuance/retirement, grants, transfers,
market purchases/offer settlement, forex, lending, lotto and reversal. The
findings above are concrete classification/audit gaps; they do not establish
that every noncash operation is missing a balance posting. Loan acceptance,
accrual, fulfillment disputes/completions, quote/listing changes, lotto draw
selection, role changes and currency reform warrant business audit records,
not invented cash transfers. Archive sanitized business records, not raw
authentication commands containing password verifiers.

Other corrections to the original proposal:

- Checkpoint rows already write used bytes. Replacement checkpoint and current
  bank coexist during publication; each rotation does not write a checkpoint
  into both banks. The old bank is reclaimed after head publication.
- Conditional serialization occurs in Event.timestamp/client_key as well as
  Command::List.details and Configure.offer_settles_after. EconomicDetails
  currently has defaults, but no conditional skips. Audit actual stored types.
- A namespace is not a capacity reservation. ncarch shares the partition with
  journal, checkpoints and metadata. Reserve capacity explicitly.
- RAM/page-local lookup cannot answer reversed_by exactly for cold history.
  Even a short reversal distance can cross a page, and this app permits a
  much longer distance. Do not accept a knowingly incorrect null.
- Saturating accounting totals are lossy: saturation followed by reversal
  cannot recover the correct total. Use checked arithmetic.
- A ring archive cannot provide all-time detail forever. Totals cannot be
  recomputed from the retained suffix alone after pruning.

## Schema decisions worth making now

These are recommendations for agreement before archive implementation, not
approval to build every roadmap feature at once.

1. **Separate actors, owners and accounts.** MemberId currently represents both
   login identity and money ownership; balance/usd_cents live on Member.
   Introduce bounded local PartyId/AccountId and a small AssetId, initially
   native coins and recorded USD. Issuance and escrow are typed system accounts.
   This enables corporate treasuries without fake users/passwords. Keep the
   16-login limit unless separately changed; audit member-indexed arrays and
   masks instead of merely widening MemberId. This is a real refactor and
   should be its own increment if accepted.
2. **Separate transaction order from command identity.** Give cash transactions
   contiguous ordinals and group related legs by event/group ID plus leg index.
   Remove the +4096 second-leg trick in this reset schema. Noncash audit needs
   a separate stream position or explicit record kind, not fake cash entries.
   Public IDs stay opaque and browser-safe.
3. **Persist an economy incarnation.** A random ID at provisioning/reset plus
   local IDs prevents collisions across reset and future board boundaries.
   Checkpoint generation changes too often to be household identity. Invalidate
   old cursors and retry namespaces on reset. Federation signing identity and
   key rotation are a future protocol, distinct from storage incarnation.
4. **Preserve purpose and original units.** Store asset/epoch, transaction
   purpose, economic classification, typed business reference, group/leg,
   correction/refund reference and bounded memo. Purpose distinguishes loan
   draw/repayment and gifts from sales; economic classification serves GDP.
   Use a small common record plus bounded typed payloads where necessary,
   not large empty fields for every speculative feature.
5. **Make reforms historical events.** Persist currency definitions and exact
   reform factors. Historical records retain original amounts/units; active
   balances and obligations still rescale atomically. Converted history is a
   view with defined exact/fractional behavior. Do not silently round or make
   reform legality depend on whichever historical rows remain cached. Pin
   definitions needed by retained history, live obligations and summaries.
6. **Add memo privacy with enforcement.** Persist HOUSEHOLD/PARTIES_ONLY and
   implement redaction on every read path in the same increment. Corporate
   parties are owners, not just the human who clicked Pay. Never hide amounts.
7. **Define correction semantics.** Preserve provenance and original category.
   Decide atomic whole-forex-trade undo versus intentional single-leg correction
   before generalizing groups. Partial refunds need remaining refundable amount
   and an original reference; one reversed boolean is insufficient.

## Binary format

Prefer private stored DTOs separate from HTTP views, so changing an API field
does not silently alter flash bytes. Keep intentional credential exclusion;
do not remove all serde skip attributes indiscriminately. Use postcard with
default features off and caller-provided bounded to_slice buffers. Conditional
skips must not occur in positional stored types. Audit tagged/untagged/flatten
usage and every stored enum/option, including nested checkpoint objects.

Specify magic, schema/kind, length and CRC covering interpretation-critical
header fields as well as payload. Reject unknown accounting kinds, bad lengths
and trailing bytes. Schema identification does not imply legacy readers today.
Postcard positional fields/enum indices require documented ordering and golden
byte fixtures; declaration reordering is a format change.

Recommendation: hash canonical stored command bytes for idempotency in the
reset schema. Today's fingerprint hashes JSON in a 2 KiB scratch buffer.
Test altered retries and stable command encoding. No old fingerprints need
preservation. Keep fixed desktop frames initially if useful for indexed seeks.
For trimmed NVS frames validate actual blob length against the declared length
before zero-filling: padding alone can hide truncation of zero-valued bytes.

## Archive publication and replay

Initially archive only at rotation. The durable journal already protects
acknowledged payments; no second per-payment archive write is necessary.
Seal the final partial page at rotation, then start a fresh page next time.
Avoid rewriting a partial page on every payment. Repeated manual checkpoints
can reduce packing efficiency; measure that case too.

The committed checkpoint/manifest must identify incarnation, event replay
watermark, transaction count C, first retained ordinal, exclusive archived_end,
page directory/root, business-audit boundary and reversal/definition roots.
At a checkpoint archived_end = C; its retained archive interval is
[first_retained, C). Physical staged pages beyond that boundary are not committed
history. Retention loss is explicit.

Under the existing service lock:

1. Preflight worst-case space for staged archive/index pages, replacement
   checkpoint, retry receipts and NVS collection headroom, while keeping the
   current generation recoverable. Bound output bytes/legs for an entire
   command. Rotate before any unarchived cash or audit record can leave RAM,
   including scheduler paths; transaction count alone cannot bound audit bytes.
2. Persist and verify immutable staged pages with identity, ordinal range/count,
   length and checksum. Write staged indexes and the replacement checkpoint
   referencing those pages. Keep every page required by the committed head.
3. Durably publish one head selecting that checkpoint and its archive roots.
   Only afterward reclaim old journal/checkpoint data and pages outside the
   newly committed retention interval. Advancing the retention floor belongs
   in publication, not an unrelated low-space delete.
4. Latch unavailable on failed/ambiguous storage, as today. Missing/corrupt
   committed pages fail closed. Unpublished pages are orphans: ignore/reclaim
   them, or verify identical content before reuse. Never overwrite a committed
   ring slot. Cleanup after publication must be restartable.

Restore checkpoint state, load archive tail only through C, then replay every
journal event after its event watermark exactly once. Replay must update
balances, totals, obligations and retries normally. Do not skip financial
record_transaction calls based on a physical archive high-water mark: an
interrupted rotation may have written pages that the head never committed.
Loading historical rows itself must not apply postings or increment totals.
Commands with no cash transaction still need normal replay.

Long-lived/reversible obligations keep complete payment/classification facts in
the checkpoint independent of history retention. Restore must supply facts
needed for history-dependent replay; remove such dependencies for live promises.

Recommended exact reversal lookup: annotations grouped by original archive
page, with copy-on-write updates published through the same manifest and a RAM
overlay for uncheckpointed reversals. Retain annotations while their originals
are retained, and pin live obligation facts separately. Specify and budget the
page directory/annotation layout before implementing this phase; there is no
free arbitrary reverse lookup in a write-once stream.

Desktop needs equivalent sync/head-publication and torn-tail behavior. A single
append-only .archive file does not provide bounded disk retention; choose
segments or copy/compact publication and test interrupted reclamation.

## Cache, paging and totals

3,000 cached transactions is a candidate, not a requirement. Measure actual
struct size, PSRAM/internal heap placement, largest free block, stack, replay
and latency. sdkconfig already requests NVS cache in PSRAM. The rotation margin
must cover every multi-leg event. Archive retention is independent of cache size.

Use exclusive before ordinal, snapshot upper bound and economy incarnation in
cursors. Bound response bytes and pages scanned as well as the 100-row limit.
Filtered pages may be empty with a continuation: advance from the last scanned
position. Return retention floor and explicit truncation/expired-cursor status.
Never label the retention boundary as the beginning of all history. Use a
bounded page directory to find cursors, and bounded work under the service lock.
Define consistency for changing totals/circulation/reversal views; expose state
sequence rather than mixing different states into an allegedly exact report.

Update both HTTP surfaces, Angular models/client, demo backend, Economy, Central
Bank, history and export consumers. Redact private text identically in warm and
cold history, diagnostics, logs and exports. Do not assemble full history on
the board to serve one report.

Lifetime counters update on normal apply/replay and survive checkpoints.
Classify using original purpose/reference, not memo text, recycled objects or
current role. Prefer stable treasury/account identity for Nana's reports.
Cash flow is separate from accrued interest and other promises. Port the
central-bank categories after correcting provenance gaps.

Prefer checked i128 accumulators with bounds validated before durable append,
not saturating i64. Send large JSON totals as decimal strings and use exact
client arithmetic. Define units/epoch conversion and capacity of any retained
epoch table. After pruning, verify opening summary at retention floor + retained
deltas + journal tail = lifetime totals, in matching units. A full-history
reducer is only available while the full history exists. Period reports must
state missing coverage; bounded monthly summaries can preserve older aggregates.
Define corrections as flows at correction time unless another policy is chosen.

## Waterfall and acceptance gates

1. **Semantics and measurements.** Agree account-refactor scope, epochs, grouped
   corrections, privacy and audit retention. Measure JSON/postcard on short and
   maximum UTF-8 memos, keyed/credential events, loan legs, offers, lotto,
   fulfillment and full checkpoints. Record size, CPU, NVS entries, RAM and
   latency; host timings do not establish board wear.
2. **Semantic repairs and accepted foundations.** Preserve undo classification
   and forex references; add regression tests after eviction/restart. Implement
   agreed identity, epoch and privacy changes as separate reviewable increments.
3. **Codec and used-length frames.** New stored types/magic and canonical hashes,
   postcard round trips and strict frame validation. Keep history in checkpoints
   for this increment; no archive dependency is needed to gain smaller writes.
4. **Archive/recovery.** Implement bounded directories, reversal annotations,
   retention and reset protocol. Remove history rows only after fault-injection
   proves complete state/retry/replay equivalence. Pick budgets from worst cases.
5. **Paging and totals.** UI/demo consumers, coverage, exact counters/opening
   summaries and independent reference-reducer checks.
6. **Board validation.** Repository-required checks, firmware build, then an
   explicitly authorized development reset/deploy and power-cut/performance
   measurements. Fresh reseed; no migration. No hardware action in this review.

Tests must cover every stored variant/option, numeric/string limits, Unicode,
credentials, golden bytes, corrupt header/CRC/length and exact consumption.
Inject failures at every archive/index/checkpoint/head/cleanup operation;
exercise ambiguous acknowledgements, repeated restart, full storage, ring wrap,
reset/stale cursors and keys, two-leg boundary events, zero-cash replay, reform
with cold history, long-distance reversals, old fulfillment refunds and offer
undo. Compare complete state and retry results, not balances alone. Preserve
allocation checks and cover new hot paths. Hardware measurements remain an
exit gate, not a claim established by host tests.

## External references checked

- [Postcard wire format](https://postcard.jamesmunns.com/wire-format): positional
  encoding and enum discriminants; stable codec does not freeze application schema.
- [Postcard to_slice](https://docs.rs/postcard/latest/postcard/fn.to_slice.html):
  caller-provided serialization buffers.
- [ESP-IDF 5.5.3 NVS](https://docs.espressif.com/projects/esp-idf/en/v5.5.3/esp32s3/api-reference/storage/nvs_flash.html):
  NVS already wear-levels. Documentation estimates 22 KB RAM per MB of partition,
  another 5.5 KB per 1,000 keys and about 0.5 s initialization per 1,000 keys.
  Include those costs in measurements. A 4 KiB blob is not one physical sector
  write, and raw partition bytes are not usable application payload capacity.
