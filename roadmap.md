# NanaCoin roadmap

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
   record, checkpoint format, and replay path. Default missing values from older
   journals to `HOUSEHOLD`.
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
   member, Nana, old journal records, reversals, offers, and retained-history
   eviction. Include a regression test proving all viewers calculate identical
   public balances and economy series.
