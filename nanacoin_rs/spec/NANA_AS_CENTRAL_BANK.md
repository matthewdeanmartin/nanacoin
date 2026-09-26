# Nana as central bank: the report

Status: implemented client side, September 26, 2026. Angular:
`nanacoin_ui/src/app/economy/central-bank.ts` (pure calculations, tested in
`central-bank.spec.ts`) and `pages/central-bank.ts`, route `/central-bank`,
listed as **Nana as Central Bank** in the **Accounting** menu. No server
changes. Every signed-in member can read it: the household can audit its
money, like the paper notebook.

Related: NANA_AS_MARKET_MAKER.md (the desk that posts Nana's offers).

## Inputs

- `GET /transactions?limit=365`: the retained ledger and `circulation`.
- `GET /quotes`, `GET /loans`, `GET /lottos`: Nana's open promises. Members
  see only their own loans, so loan receivables are shown to Nana only.
- Session household: Nana's coin balance and `usd_cents`.

The board keeps at most 365 transactions. Flows describe that retained window,
and the page says so when older history has gone. Period choices: last 30
days, last 365 days, or everything retained.

## Headline numbers

| Number | Meaning |
|---|---|
| Money supply | All coins that exist (`circulation` = minus the issuance account). |
| Held by the household | Money supply minus Nana's own coins; includes lotto pools. |
| Dollar reserve | Nana's recorded real dollars. |
| Reserve ratio | Dollar reserve / dollars owed if every open Nana buy offer is taken. 100% or more means every cash-out promise is covered. "No buy offers" when she promises none. |
| Backing per coin | Dollar reserve / coins held by the household. What each coin would get if everyone cashed out pro rata. Nana's ladder pays early sellers more than this and late ones less. |
| Inflation | Average repeat-sale price change in the period (same measure as Economy). |
| Money supply growth | Net issuance in the period / supply at the start of it. |
| Exchange rate | Price of the most recent completed dollar trade. |

## Flows

Every retained transaction in the period, messages excluded, is sorted once:

1. **Dollar issuance** (a posting to `account:usd-issuance`): real dollars
   recorded into the household.
2. **Dollar legs of Nana's trades** (`-usd` postings with a `quote-`
   reference): dollars Nana paid out or took in.
3. **Money supply** (a posting to `account:system-issuance`):
   - issued to Nana, issued to members, issued for lotto interest (a `lotto-`
     reference; the part Nana could not pay from her balance),
   - retired,
   - corrections: reversals of issuing or retiring, shown separately.
   - Net change = the sum of all of these.
4. **Nana's coin flows** (her posting, no supply posting):
   - exchange coin legs (`quote-`): coins bought back or sold,
   - `INTEREST`: interest received or paid,
   - `LOAN_PRINCIPAL`: lent or repaid (not income),
   - `LABOR` / `GOOD`: Nana buying or selling work and goods,
   - anything else: paid out (allowances, gifts) or received.

A reversal carries its original's economic kind with mirrored postings. It is
booked against the original's category with a minus sign, so a reversed
purchase cancels the purchase instead of looking like a sale.

**Seigniorage** is the net new coins valued at the latest trade price: what
Nana "earned" by creating money. **Demurrage** is listed as "None charged"
(see the market-maker spec for why).

## Holds and promises

A simple balance sheet, not double-entry accounting:

- Holds: dollar reserve, her own coins, loans owed to her (principal plus
  accrued interest, and how much is overdue; Nana only).
- Promised: dollars for her open buy offers, coins for her open sell offers,
  lotto interest still to pay on unsettled delayed and savings lottos, ticket
  money held in her lottos, loan offers not yet taken (Nana only).
- Best buy and sell price and the spread.

Nana's own coins are not really an asset of a money issuer, which can always
make more. They are listed because they are what she spends and lends without
issuing.

## Answers to owner questions

- **How are coins retired?** `/admin/retire` moves coins from an account back
  to the issuance account. It is a normal `RETIRE` ledger entry, visible to
  everyone, and it shrinks the money supply. It is not a silent edit of Nana's
  balance, and should not become one: the books must add up.
- **Does Nana go negative?** No. The server refuses overdrafts for everyone.
  When she is short, she issues coins to herself first, which appears here as
  "New coins issued to Nana" and raises money-supply growth.
- **Demurrage?** A fee on holding money. Not charged. With a dollar exit
  available, it would push people toward dollars. See the market-maker spec.

## Not yet covered

- Full history: once the board overwrites old transactions, older flows are
  gone. A durable monthly summary (server side, or exported) would keep them.
- Escrow detail beyond Nana's own lottos.
- Corporations (roadmap) will need their own lines: money held by companies
  and lending to them.
