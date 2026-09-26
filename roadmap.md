# NanaCoin roadmap

Implemented API foundation, September 26, 2026: binary journal/checkpoints,
bounded committed archive, immutable monetary history, correction annotations,
partial refunds, sanitized business audit and exact per-epoch totals. Gift
requests and unique digital-art mint/list/buy/gift/equip APIs are available.
See [storage/API contract](nanacoin_rs/spec/STORAGE_V2.md) and
[commerce API](nanacoin_rs/spec/COMMERCE_API.md). UI features, company accounts,
federation, memo privacy and art returns remain future work. The proposals below
remain guidance where they go beyond that implemented scope.

## Storage and transaction foundations — proposed September 26, 2026

See [binary storage review](nanacoin_rs/spec/FIX_JSON_REVIEW.md) for findings,
recovery invariants and the implementation waterfall. These are recommendations,
not shipped features. Pre-release development data is disposable: no migration
or old-format compatibility work. Schema freeze/migrations begin with the first
real user under AGENTS.md, not merely because storage becomes binary.

Prioritize the offer-undo classification and forex-reversal reference gaps,
immutable historical money units, complete reversal lookup and explicit history
coverage. Some business actions need audit records without money movements:
fulfillment/dispute transitions, loan agreements/accruals, lotto draw selection,
currency reform and account authority changes. Keep credentials out of the
public business audit. An archived payment must remain understandable after
its listing, quote or other live object has been recycled.

Before committing the new archive schema, decide these foundations:

- Separate login actor, owner/party and ledger account. Keep local IDs compact
  and capacities bounded; corporations do not need login passwords of their own.
- Asset ID and currency epoch instead of a USD boolean; retain historical
  amounts and exact reform definitions. Initially support only current coins/USD.
- Transaction ordinal plus event/group and leg identity; remove offset IDs.
  Persist purpose, original correction provenance and typed business references.
- Economy incarnation for reset-safe IDs/cursors and future external references.
  Checkpoint generation remains a storage/retry concept, not federation identity.
- Memo visibility with complete authorization/redaction, not just a dormant flag.
- Explicit noncash audit records and a defined retention policy. Preserve live
  obligations outside an evictable history cache.

These foundations do not require an arbitrary extensible JSON metadata column,
unbounded posting lists, or placeholder implementations of distant features.
Use small common fields and bounded typed feature records. Account separation
is a significant refactor and should be reviewed independently of the codec.

## Future features and their data-model consequences

| Feature | Useful first version | Required model |
|---|---|---|
| Corporations | A shared treasury operated by authorized members; payments identify the human actor. | Party kind, separate accounts, membership/capability grants, ownership and authority-change audit. Keep historical authorization attribution after a member leaves. |
| Digital art / profile bling | Sell or gift a collectible edition and equip it on a profile. | Asset/edition ID, creator, content digest and bounded locator, license/edition terms, current owner, transfer provenance and equipped-item reference. Buying a license does not inherently transfer copyright. |
| Gift requests (cyberbegging) | Ask for a gift, receive contributions, close the request. | Request ID, requester/beneficiary, bounded description, optional target/expiry, status, gift payments referencing the request and refunded/net contribution totals. Gifts remain outside production/GDP. |
| Federation | Share public profiles/listings first; money settlement later. | Stable issuer identity, namespaced external references, authenticated peer keys, provenance, durable inbox/outbox, remote-operation deduplication and settlement obligations. Foreign currency is not local issuance. |
| Partial refunds and disputes | Refund part of a sale without pretending the original never happened. | Original payment/group reference, refunded amount, remaining eligibility, reason and fulfillment/ownership consequences. Reject duplicate or excessive refunds. |
| Durable allowances / subscriptions | Scheduled gifts/payments that work while browsers are closed. | Standing mandate, payer/beneficiary accounts, recurrence/amount/asset/epoch, next occurrence, cancellation and durable occurrence identity. Link each payment to its mandate/occurrence. |
| Loan forgiveness / write-off | Explicitly close a debt without fabricating a cash repayment. | Principal/interest adjustment events, authority/reason, changed obligation and separate noncash reporting. Define creditor forgiveness versus accounting write-off. |
| USD withdrawal/reconciliation | Record real cash leaving the tracked household, with a receipt. | Explicit recorded-USD retirement/adjustment purpose and reference. A correction of an erroneous entry is distinct from a new cash withdrawal. |
| Long-term reports and backup | Preserve monthly aggregates and export complete recoverable state. | Opening/period summaries with units/coverage; a separate private consistent backup including checkpoint, archive, obligations and retry state. Existing HTML export is a re-entry aid, not this backup. |

### Corporations

Start with company accounts and permissions. Defer shares, dividends, payroll,
voting and dissolutions to explicit later designs. Those later features need
ownership/capital and distribution records; a corporation is not merely a user
with a different display name. Debit/credit authorization must check the actor's
authority over the paying account, and privacy must define which company members
may read company descriptions. Review member-indexed loan/lotto arrays and
credit masks before allowing company participation there.

### Digital art

Keep image bytes off the accounting journal; store bounded metadata/digest and
a reference to delivery storage. Define exclusive ownership versus licensed
copies and edition supply before implementing sales. A sale should atomically
move payment and local entitlement; a failed request must not charge without
granting ownership. Profile equipment is a preference referencing an owned
entitlement, not proof of ownership. Defer royalties until grouped multi-party
settlement and refund rules are designed. Media deletion/availability is a
separate lifecycle from immutable purchase evidence.

### Gift requests

An ordinary request is not an invoice, loan or reservation of the donor's money.
Start with direct gifts linked to the request; targets display progress without
promising all-or-nothing funding. Later conditional fundraising requires escrow,
release/refund rules and expiry events. Recurring sponsorship can reuse durable
mandates. Decide whether contributions may exceed the target, and count refunds
against progress without deleting the original gift. No separate currency or
new account type is needed just for a gift request.

### Federation

Stage discovery/read-only sharing before payments. Local atomicity does not make
two boards atomic. Payment federation needs a protocol for funds reserved,
remote acceptance, settlement, failure/timeout and reconciliation; timeout is
not proof that the peer failed to pay. Bound inbox/outbox and outstanding holds.
Persist replay protection independently of today's evictable HTTP retry ring,
so an old signed delivery cannot become a new payment after compaction.

Decide issuer/trust rules and remote currency representation before treating
another board's NanaCoin as interchangeable. Currency display names and URLs
are not identities. Canonical signed protocol messages should have an explicit
schema independent of internal postcard layout. Do not build transport, crypto
protocol or cross-board escrow merely to ship binary local storage.

### Suggested delivery order

1. Repair history/correction semantics and agree the small shared foundations.
2. Binary/trimmed storage, then archive/recovery, paging and exact summaries.
3. Gift requests and durable allowances, which mostly reuse gift payments.
4. Corporation treasury/authority and local digital-art ownership/settlement.
5. Federation discovery, followed by a separately reviewed settlement protocol.

Feature-specific tables can arrive with their features. The point of deciding
identity, units, provenance and authority now is to avoid misrepresenting today's
records, not to predict every future field or retain unused compatibility layers.

## Mastodon subscriptions and sharing intents

- Subscribe to all household offers and all forex offers through Mastodon. This
  is deliberately separate from the first direct-message integration: it needs
  clear opt-in, deduplication, reconnect/backoff behavior, and a decision about
  whether a browser must remain open.
- Add platform intents in Send for the major networks so a member can post about
  NanaCoin from their own account, optionally in ALL CAPS. Facebook is the first
  intent; the other platforms remain deferred.

The shipped direct-message boundary stays private: only `visibility=direct`,
and only household members whose Mastodon ID is registered in NanaCoin may be
selected as recipients. OAuth tokens remain browser-local.

## Private transaction descriptions on a shared ledger

NanaCoin's ledger remains publicly visible to anyone who can reach the board: amounts,
accounts, transaction kinds, timestamps, references, reversals, and the economy
derived from them stay auditable. Privacy applies only to human-written memo or
description text.

### Product rules

- Add a per-transaction description visibility flag: `HOUSEHOLD` (default) or
  `PARTIES_ONLY`.
- A private description is readable by an authenticated transaction party and
  Nana. Anonymous and uninvolved readers see a stable placeholder such as
  “Private description.”
- Never hide postings or amounts. Balance, circulation, GDP, and ledger
  invariants must produce the same result for every member.
- Issuance, retirement, reversal reasons, and system-generated audit text stay
  household-visible unless the domain model explicitly proves a narrower rule
  is safe.
- Record the visibility choice in the append-only transaction itself. It cannot
  be retroactively changed without an explicit correcting/audit event.

### Delivery plan

1. Add the compact visibility field to the Rust domain transaction, journal
   record, checkpoint format, and replay path. Use the current schema and reseed
   disposable development data; do not add an old-journal defaulting decoder.
2. Accept the flag on transfer, purchase/offer settlement, and other eligible
   commands. Validate visibility server-side; do not trust the client to redact.
3. Render transaction views for the requesting actor. Replace private text for
   non-parties while returning the complete postings and all non-private fields.
4. Add “Household can read this” / “Only the people involved and Nana” beside
   memo fields in the Angular client. Default to household-visible and show the
   selected privacy state in confirmations and receipts.
5. Make ledger, economy, search, logs, exports, diagnostics, and error messages
   use the redacted view. Ensure private text never enters browser/server logs.
6. Add authorization and replay tests covering sender, recipient, uninvolved
   member, Nana, current-schema restart/replay, reversals, offers, and retained-history
   eviction. Include a regression test proving all viewers calculate identical
   public balances and economy series.
