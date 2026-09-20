# Foreign exchange

NanaCoin holds dollars as well as coins, and members trade one for the other
at rates they quote each other. USD today; the design is not specific to it.

This is the first thing the system holds that it **cannot create**. A coin's
provenance is `account:system-issuance` and the invariant that every posting
sums to zero. Dollars come from outside the household, so they need the same
construction or "where did this $5 come from" has no answer.

## What is built

**Step 1 — currency in the ledger.** Done, at zero RAM cost.

- `ledger.Currency` is a byte. `NANA` is zero, so every record written before
  this existed reads back as coins, and any transaction that does not mention
  a currency is in coins.
- `Posting.Currency` tags each leg. In the packed record the two tags live in
  padding the struct was already carrying: `PackedTransaction` is **56 bytes
  before and after**, and the 365-record ring is **20.0 KB before and after**.
  Measured, not estimated.
- `account:usd-issuance` mirrors `account:system-issuance`. Both are exempt
  from the overdraft check, because their negative balance *is* the amount in
  circulation. Without the exemption no dollar could ever enter the household.
- `Book.USDHeld()` is the dollar equivalent of `Circulation()`.

**Balancing is per currency.** `Sum` takes a currency, `Validate` checks each
one separately, and `CheckInvariants` keeps two grand totals. A single total
would let 100 coins leaving cancel 100 cents arriving and call the book
balanced - `TestCrossCurrencyCannotCancel` is that exact scenario.

The journal encodes the currency per posting. That is not optional: the wire
format is separate from the in-RAM record, and a currency it dropped would
replay as NanaCoin and silently turn dollars into coins at the next reboot.
Nothing would error and the ledger would still balance.
`TestCurrencySurvivesTheJournal` pins it.

## Why not one transaction with four postings

The obvious model for an exchange is a single atomic record:

```
Alice NANA  -100    Nana  NANA  +100
Nana  USD   -500    Alice USD   +500
```

`MaxInlinePostings` is 2. Raising it to 4 adds 20 bytes to every one of the
365 ring slots - **7.1 KB**, measured - on a board with about 22 KB free and
an empty ledger. Paying that on every chore payment so that rare forex trades
fit is the wrong trade.

So an exchange is **two linked transactions**, one per currency leg, joined by
the existing free-form `Reference` field:

```
txn-41  Alice NANA -100  →  Nana NANA +100
txn-42  Nana  USD  -500  →  Alice USD +500    reference: txn-41
```

### The honest cost

Two records can half-land: one commit succeeds, the other fails, and coins
have moved while dollars have not.

Mitigated, not eliminated. Both legs are validated before either is committed,
and an unmatched leg is visible and reversible through the same settlement
window offers use. `CheckInvariants` can be extended to "every forex leg has
its pair", which makes a half-landed trade a boot-time failure rather than a
quiet discrepancy.

A four-posting record would remove this entirely. It costs 7.1 KB. That is the
trade, stated plainly rather than hidden.

## Quotes, and why they are not listings

Anyone may quote a rate. The Angular client will eventually watch the book and
accept good rates automatically, which decides the shape: a quote has to be
**machine-readable and pollable**, not a price buried in a listing title.

So a `Quote` is its own record - side, rate as an integer (NanaCoin per cent),
amount, expiry - and `GET /quotes` returns the book sorted by rate. Roughly
40 bytes x 16 slots, about **640 bytes**.

Bid/ask is then the best quote on each side, and the spread is the gap between
them. There is **no matching engine**: a trade is someone accepting a specific
quote, through the same idempotency-keyed accept path offers already use. A
household is a quote-driven market, not an exchange.

## Still to build

2. **Quote records and `/quotes`** - the pollable book.
3. **Accept-a-quote** - the two-leg transaction and its cross-reference. This
   is where the half-landing risk lives and deserves the most test attention.
4. **The Angular forex page** - the book, balances in both currencies, and a
   form for quoting.

## Settlement is offline, deliberately

The ledger records that dollars moved. Whether the physical five-dollar bill
has actually changed hands is trust, exactly as it is for "an hour of Switch
time" - and reversible the same way if it has not. The board is not going to
learn what is in anyone's wallet, and pretending otherwise would make the
record less honest rather than more.
