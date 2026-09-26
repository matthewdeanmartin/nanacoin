# Nana as market maker

Status: first version implemented client side, September 26, 2026. No server
changes. Angular: `nanacoin_ui/src/app/nana/market-maker-plan.ts` (pure
planning and checks) and `pages/market-maker.ts` (the page, route
`/nana/market`, linked as **Market desk** from Nana's household page).

## Goal

Nana is the household's government, central bank and market maker. She should
always be standing by to:

- buy and sell NanaCoin for dollars,
- lend (and, later, borrow),
- run lottos, taking the house side.

That gives children liquidity: they can always cash out a little, borrow a
little, or save. But the prices should make a run on the bank punishingly
expensive, so Nana's real dollars cannot be drained.

## How the desk works

Nana fills in up to three sections, checks a preview, and presses one button.
The page turns the settings into a list of ordinary API calls and runs them
one at a time, in order:

1. withdraw Nana's old exchange offers and unaccepted loan offers (optional),
2. top up Nana's dollars or coins if needed (optional, and shown),
3. post exchange quotes,
4. post loan offers,
5. create lottos.

Each step has its own idempotency key. If one fails, the batch stops and marks
the failed step. **Try the rest again** reuses the same keys, so no step runs
twice. The frozen batch is resumed as it was, not re-planned. Re-planning
after partial success would, for example, withdraw the quotes the batch had
just posted.

The server validates every step as it does for any other caller. The client
plan exists to explain problems in plain words before anything is sent.

### Dollar exchange ladder

Each rung is one quote: a share of the coins in circulation at a price per
coin. Shares scale with the money supply, so the ladder stays in proportion as
the economy grows. Default rungs, which reproduce the owner's example at 1,000
coins in circulation:

| Nana buys (BID) | at 1,000 NC | price | dollars |
|---|---|---|---|
| 1% | 10 NC | $1.00 | $10.00 |
| 5% | 50 NC | $0.75 | $37.50 |
| 10% | 100 NC | $0.25 | $25.00 |
| 100% | 1,000 NC | $0.01 | $10.00 |
| | | **total** | **$82.50** |

Nana sells (ASK) at 1% for $1.50, 5% for $2.00, 10% for $3.00.

- The first child to cash out a little gets a good price. If everyone tried to
  cash out, the later rungs pay almost nothing. Nana's worst-case dollar
  outflow for a whole ladder is fixed and shown before posting ($82.50 per
  1,000 coins above), however many coins exist.
- Quotes fill all or nothing (`forex.rs`). A rung is one child's trade; once
  taken, it is gone until Nana posts again. That bounds a run on the bank by
  construction.
- Rung sizes are whole coins, at least one coin. The server requires coins x
  cents to divide evenly by the coin scale, and whole coins guarantee it.
- The plan refuses a buy price at or above any sell price. Otherwise anyone
  could buy from Nana and sell straight back for free money.
- The spread between buy and sell prices is Nana's dollar income.
- Buy rungs are backed by Nana's recorded dollars (`usd_cents`), which the
  server checks when a child takes the quote. If the full ladder needs more
  than she has, the plan says how much. Optionally, it first records the real
  dollars she is holding (`/admin/issue-usd`). That checkbox must mean real
  cash: recording dollars that don't exist makes the promise worthless.
- Sell rungs are backed by Nana's coins. If she is short, the plan can issue
  the shortfall to her first (visible issuance).
- Offers can expire after N days (default 30), so a stale ladder does not
  outlive a change in policy.

### Standing loan offers

Nana picks who can borrow and up to four loan sizes. Each size is a share of
circulation and an interest rate per 30 days. Defaults: Small 1% at 1%, Medium
5% at 5%, Large 15% at 15%. Small loans are cheap and big loans are expensive,
which is the owner's "market maker for loans".

- Payments every day, week (default) or 30 days. The installment is about a
  quarter of the principal in whole coins, so a loan is repaid in about four
  payments plus interest.
- Nana lends coins she already has. Offers do not reserve funds; the server
  checks when a child accepts. The preview shows the total if every offer
  were accepted at once, and warns if Nana could not cover it.
- The loan contract names a single borrower (`LoanOfferInput.borrower`). So
  the desk posts one offer per person per size. With 5 children and 3 sizes,
  that is 15 of the board's 32 loan slots.

### Lotto series

Up to 12 lottos, one a month. They close on the same day each month (clamped
to month end, so January 31 is followed by February 28), all with the same
kind, ticket price and interest. Titles are "Monthly lotto · February 2027".
Nana is the house of every lotto she creates.

### Board limits the plan enforces

| Resource | Board limit | Plan leaves free |
|---|---|---|
| Open exchange quotes | 16 (`forex.rs QUOTES`) | warns below 4 |
| Loans not finished | 32 (`loans.rs LOANS`) | warns below 8 |
| Lottos not settled | 16 (`lotto.rs LOTTOS`) | warns below 2 |

These limits, not the arithmetic, shape the defaults: at most four rungs a
side, four loan sizes, twelve lottos.

## What needs the server (not done)

These are the gaps the client could not close. Each is small, but each is
server code.

1. **Nana borrowing ("borrowing offers").** Only a lender can create a loan
   (`OfferLoan`). Nana cannot post "I will borrow 50 NC at 2% a month" for a
   child to accept. Needed: a borrower-initiated `RequestLoan`, or an open
   offer any member can fund. Until then, the **savings lotto** is Nana's
   borrowing product: children lend her their ticket money for 30 days, get
   it all back, and one of them gets the interest.
2. **Open offers.** An offer to "anyone" instead of one per borrower would cut
   the loan ladder from people x sizes slots to just sizes. The same applies
   to a quote that can be **partly filled**: one rung could then serve many
   small cash-outs instead of one child taking all of it.
3. **Lotto start dates.** `LottoTerms` has only `closes_at`. Every lotto in a
   series opens for tickets immediately, so a child can buy a ticket for next
   December today. The page warns about this. Needed: an `opens_at` field,
   with ticket purchases refused before it.
4. **Recurring batches.** Re-posting the ladder each month is manual. A
   server-side schedule is the only way to do it while Nana's browser is
   closed.

## Solvency

**Coins.** Nana issues the currency, so she can never run out of coins in the
sense a person can. The server does not let her balance go negative:
`validate_posting` refuses overdrafts for every member, Nana included. Only
the issuance account (the money supply itself) is negative. When Nana is
short, she issues coins to herself. That is the right answer to "I guess Nana
issues herself money if she goes in the negative": yes, but explicitly and
before spending, not by overdrawing. Each issuance is a ledger line anyone
can audit. The desk does this only when Nana ticks the box, and lotto
interest shortfalls are already issued automatically.

The cost of issuing is not insolvency but **inflation**: more coins chasing
the same chores and treats. So the real questions are:

- Where do new coins come from? Issuance to Nana, allowances, lotto interest
  shortfalls.
- Where do coins go back? Interest paid to Nana on loans, coins Nana buys back
  on the dollar ladder, taxes or fees if she ever charges them. Coins Nana
  receives can be **retired** (`/admin/retire`), which shrinks the supply.

**Dollars.** This is the one real solvency constraint, because dollars are
real. The rules that keep Nana solvent:

1. Buy offers are always backed. The server refuses a child's cash-out if
   Nana's recorded dollars cannot pay it, and the desk refuses to post a
   ladder she cannot back unless she records more real dollars.
2. The ladder's cost is bounded and proportional: its dollar total is fixed
   per coin in circulation, and the cheap rungs are where most coins would
   have to go.
3. The spread earns dollars. Selling at $1.50 and buying at $1.00 means every
   round trip adds to the reserve.
4. A useful health number for a future dashboard: **reserve ratio** = Nana's
   dollars / cost to fill her whole buy ladder. At or above 1, every standing
   promise is covered.

**Loans.** Children may not repay. The server never creates negative balances
and keeps unpaid amounts owed, so a default costs Nana only coins, which she
can issue. It is a policy loss, not an insolvency.

## Other things Nana could do

Revised after owner review. Principle: **Nana acts only where a government or
central bank must**: issuing and retiring money, holding the reserve, standing
ready to trade and lend, and keeping the system running (for example making sure
the board stays plugged in). She should not compete with the household for
ordinary work.

Already exist, not new:

- **Allowances.** Recurring payments are on the Send page. They are stored in
  the payer's browser and paid when that person opens the page and presses
  **Check allowances** (catching up on missed dates). They are recurring but not
  automatic: nothing pays while that browser is closed. A server-side schedule
  would make them automatic; that is the only gap.
- **Chore bounties.** A want-ad (a Side Buy listing, "Offer to do this") is
  already an offer to buy work. Nana does not need a separate bounty product.

Candidates that fit the central-bank role:

1. **Savings account (Nana borrows).** A standing deposit rate. Needs a
   borrower-initiated loan (above). The savings lotto covers part of this now.
2. **Open-market operations.** Retire coins Nana buys back on the dollar ladder
   when inflation runs high; issue when the household is short of cash.
   Retiring already exists (`/admin/retire`): it moves coins from an account to
   the issuance account as a normal, visible ledger entry. It is not a silent
   edit of Nana's balance, and it should never become one, because the books
   must still add up.
3. **Lender of last resort.** A tiny zero-interest emergency loan tier for a
   child stuck at zero.
4. **Matching savings.** Nana adds a share of what a child saves, paid by
   issuance.
5. **Dividends.** Share Nana's dollar spread profit or interest income with
   everyone.
6. **Transparency.** The **Nana as Central Bank** report (Accounting menu), see
   NANA_AS_CENTRAL_BANK.md.

Rejected for now: **demurrage** (a fee on holding money). It pushes people to
spend, but with a dollar exit available it also pushes them to spend coins on
dollars, which is a run on NanaCoin. The dollar ladder bounds the damage (the
cheap rungs pay almost nothing), but the incentive points the wrong way.
Revisit only if hoarding becomes a real problem and the exit is priced to match.

Corporations are on the roadmap and will change who holds money and who
borrows; revisit the ladders and this list then.
