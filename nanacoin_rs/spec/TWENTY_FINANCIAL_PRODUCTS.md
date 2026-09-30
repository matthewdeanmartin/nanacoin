# Twenty financial products for a household bank

Status: ideas, none implemented. Written September 29, 2026.

NanaCoin already has the basics of a real economy: payments, a market with
negotiable offers, loans, savings lottos, a dollar exchange desk, gift
requests, art editions, and Nana as central bank. This is a menu of what people
could build on top, from the plain (escrow, deposits) to the exotic (options,
weather derivatives). Each is sized for a household, where the point is as
much learning how money works as moving it.

Related: LENDING.md, NANA_AS_CENTRAL_BANK.md, NANA_AS_MARKET_MAKER.md,
COMMERCE_API.md.

## Ground rules for every product

These come from how the board already works, and any new product should keep
them.

- **Everything is on the ledger.** Money held for a product sits in a visible
  system account, the way the lotto escrow already does, never in a hidden
  field. The books must still add up.
- **No negative balances.** A product that might owe money later must hold it
  now (escrow, margin, a reserve), or accept that it can fail to pay and say
  so up front.
- **Nana issues money only as the central bank.** She can back a product, but
  new coins appear only through visible issuance.
- **Bounded and scheduled.** The board has fixed-size tables and one event loop
  (once a second, a few due events per pass). Products with deadlines use that
  loop, as loans do.
- **Somebody decides disputes.** Most household products end in "did it
  happen?". Nana, or an agreed adjuster, rules. The fulfillment dispute flow is
  the model.
- **Careful with chance.** Anything that pays on luck is gambling-shaped. Keep
  stakes small, prefer products where skill, effort or saving matters, and let
  Nana turn them off.

Each entry gives the idea, a household example, how the money moves, which
existing parts it reuses, what is new, and what it teaches.

---

## Protecting against loss

### 1. Escrow for jobs

**Idea.** The buyer's coins are held, not paid, until the work is done.

**Example.** Dad hires Ivy to wash the car for 15 NC. The 15 NC moves to
escrow at hire time. When Dad marks the job complete, Ivy is paid. If he
disputes, Nana decides.

**Money.** Buyer to escrow on acceptance; escrow to seller on completion, or
back to buyer on a successful dispute. An expiry date releases to the seller if
the buyer never responds.

**Reuses.** Offers and purchases, the fulfillment TODO list with Complete and
Dispute, the escrow-account pattern from lottos.

**New.** Holding the payment instead of paying at once, and a release
deadline in the event loop.

**Teaches.** Why marketplaces hold money: trust between strangers (or
siblings).

### 2. Warranty bond on market sales

**Idea.** A seller posts a bond with a listing. If the thing breaks within the
warranty period, the buyer is refunded from the bond.

**Example.** Sam sells his old bike light for 20 NC with a 14-day warranty and
a 10 NC bond. It dies on day 5; the buyer claims, Nana agrees, and 10 NC comes
back from the bond. On day 15 the bond returns to Sam.

**Money.** Seller to bond account at listing; bond to buyer on an upheld claim,
or back to seller at expiry.

**Reuses.** Listings, refunds, disputes, the scheduled loop.

**New.** A bond amount and warranty period on a listing; a claim action.

**Teaches.** Warranties are a promise with money behind it. Sellers who back
their goods can charge more.

### 3. Breakage insurance

**Idea.** A shared pool. Members pay a small weekly premium; the pool pays
claims when insured things break.

**Example.** Tablets are covered for 1 NC a week each. When Ivy cracks hers,
the pool pays 30 NC toward the repair, less a 5 NC deductible she pays herself.

**Money.** Premiums to the pool account on a schedule; claims from the pool,
approved by Nana as adjuster. If the pool is short, claims are paid in part,
stated up front.

**Reuses.** Recurring payments, an escrow-style account, approval like a gift
request.

**New.** Policies (who, what is covered, premium, cover limit, deductible), a
claim queue, and a pool that may be thin.

**Teaches.** Pooling risk, deductibles, why premiums rise after claims, and
moral hazard ("I'll be careless, it's insured").

### 4. Chore-cover insurance

**Idea.** Insurance against being unable to do your chores. If you are sick,
the pool pays someone else to cover them.

**Example.** Everyone pays 0.5 NC a week. Ivy has flu on dishes night; the pool
pays Sam 3 NC to do them.

**Money.** Premiums to the pool; the pool pays the substitute directly, on the
insured member's claim and Nana's approval.

**Reuses.** Pool account (as 3), want-ads for finding a substitute.

**New.** A claim that creates a paid job instead of a cash payout.

**Teaches.** Income protection and sick pay, in a form a child can see.

---

## Saving and investing

### 5. Term deposits (certificates of deposit)

**Idea.** Lock coins with Nana for a fixed time and get a guaranteed rate.
Early withdrawal forfeits the interest.

**Example.** Ivy locks 50 NC for 30 days at 2%. On day 30 she gets 51 NC back.

**Money.** Saver to a deposit account; on maturity, principal plus interest
back (interest from Nana's reserve, or issued, as Nana chooses and discloses).

**Reuses.** The savings lotto already holds and returns principal with
interest; loans have the rate arithmetic.

**New.** A deterministic, no-draw version: the "savings account" candidate in
NANA_AS_MARKET_MAKER.md made concrete.

**Teaches.** Being paid to wait. The trade between access and return.

### 6. Rotating savings club

**Idea.** A tanda, sou-sou or ROSCA: every member pays the same amount each
week, and each week one member takes the whole pot, in a fixed rotation.

**Example.** Four kids, 5 NC a week. Each week one of them gets 20 NC. The
first to receive gets an early lump sum; the last one has, in effect, saved.

**Money.** Members to the club account on schedule; club account to that
week's recipient. Nobody gains or loses coins overall; the only difference is
timing.

**Reuses.** The lotto's escrow and step-by-step payout. This is a lotto with
the draw replaced by a rota.

**New.** The rota, and a rule for a member who cannot pay (skip them, or pause
the club).

**Teaches.** One of the oldest savings tools in the world, and the value of
timing when there is no interest.

### 7. Layaway

**Idea.** Pay for something in instalments before you get it. The seller keeps
it until it is paid off.

**Example.** The big Lego set is 60 NC. Sam pays 10 NC a week; in week 6 it is
his. If he gives up, he gets his money back less a small fee.

**Money.** Buyer to escrow in instalments; escrow to seller on the last
payment; escrow to buyer, less the fee, on cancellation.

**Reuses.** Listings, escrow, fulfillment for handing over the goods.

**New.** A reserved listing with a payment plan.

**Teaches.** "Buy now, pay later" in reverse: saving first, with no interest
and no debt.

### 8. Shares in a household business

**Idea.** Buy part of a child's business and share its profits.

**Example.** Ivy's lemonade stand issues 10 shares at 5 NC to buy lemons and
cups. At the end of the summer she pays 2 NC per share from her profit.

**Money.** Investors to the founder at issue. Dividends from the founder,
split by shares held. Shares can be resold in the market.

**Reuses.** Art editions already give a numbered thing an owner and let it be
sold; a share can work the same way.

**New.** A dividend action that pays every holder in proportion, and a cap on
the number of holders to fit the board.

**Teaches.** Equity versus debt: shareholders share the upside and the risk,
lenders just want their money back.

### 9. Household index fund

**Idea.** One purchase that spreads money across many investments: shares in
several household businesses, and loans.

**Example.** Grandma puts 100 NC into the fund. Its manager (Nana, or a
teenager earning a fee) buys shares and makes small loans. Grandma's units go
up or down with the lot.

**Money.** Investors buy units at the fund's current value; the fund's account
holds the investments; redemptions pay out at current value.

**Reuses.** Shares (8), loans, the central-bank report's valuation.

**New.** Unit pricing (value of holdings divided by units), and an honest,
shown valuation of loans that might not be repaid.

**Teaches.** Diversification, fees, and why "the market" is an average.

### 10. Project bonds

**Idea.** Borrow from the household for a project, and repay with interest
once it pays off.

**Example.** The treehouse needs 80 NC of wood. Sam sells eight 10 NC bonds
paying 11 NC in three months, and repays them from charging siblings 1 NC per
visit.

**Money.** Bondholders to issuer now; issuer to holders at maturity. On
default, holders are owed but, as with loans, no negative balance is created.

**Reuses.** Gift requests already have a target and deadline for raising
money; loans have repayment and arrears.

**New.** Many lenders to one borrower, with the bond as a tradeable claim.

**Teaches.** How schools, roads and companies borrow, and what a default
means for the people who lent.

### 11. Annuity

**Idea.** Hand over a lump sum now in exchange for a guaranteed weekly payment
for a fixed time.

**Example.** Ivy wins 50 NC in the savings lotto and buys a 12-week annuity
from Nana paying 4.5 NC a week (54 NC in total).

**Money.** Buyer to Nana now; Nana to buyer on a schedule. Nana holds a reserve
so the payments are certain.

**Reuses.** Loans run in reverse, with the scheduled loop for payments.

**New.** A product where the member lends to Nana and is repaid in instalments.

**Teaches.** The time value of money: why a lump sum is worth less than the
same total paid later, or more, depending on the rate.

---

## Credit

### 12. Pawn loans (collateralised)

**Idea.** Borrow against something you own. If you do not repay, the lender
keeps it.

**Example.** Sam borrows 20 NC from Nana against his art edition "Moonlit
garden". He repays 22 NC in two weeks and the art is his again. If not, it
passes to Nana.

**Money.** A loan, as today, with ownership of an item held until repayment.

**Reuses.** Loans, art edition ownership (the only asset the board tracks
today).

**New.** Locking an edition as collateral, and transferring it on default.

**Teaches.** Secured versus unsecured lending, and why secured loans are
cheaper.

### 13. Co-signed loans

**Idea.** Someone with a good record promises to repay if the borrower does
not.

**Example.** Ivy (11) wants a 40 NC loan. Nana will lend only if Dad
co-signs. If Ivy misses payments, they are collected from Dad.

**Money.** A loan with a second account for arrears collection.

**Reuses.** Loans and their arrears collection.

**New.** A guarantor who must accept, and collection that falls through to
them.

**Teaches.** Vouching for someone costs something; co-signing is a real
promise.

### 14. Credit report and tiered rates

**Idea.** A record of how each member has handled loans, which lenders can use
to set rates.

**Example.** Sam has repaid three loans on time, so Nana's desk offers him 1%
instead of 3%. The report shows him why.

**Money.** None directly. It changes the terms other products offer.

**Reuses.** Loan history is already on the ledger; the central-bank report
already reads it.

**New.** A computed, member-visible report, and a rule for a lender to price
from it. Keep it explainable: no hidden score.

**Teaches.** Reputation has a price, and the report is something you can check
and fix.

### 15. Prepaid store credit

**Idea.** Pay a seller in advance for future goods, at a discount.

**Example.** Ivy's bakery sells a card for 10 cookies at 18 NC, instead of 2 NC
each. She gets the coins now and owes cookies later.

**Money.** Buyer to seller at purchase. The seller's promise is a liability,
tracked as remaining uses.

**Reuses.** Nana-nickles are bearer promises; fulfillment tracks delivery.

**New.** A counted, seller-specific voucher, and what happens if the seller
stops trading.

**Teaches.** Gift cards and subscriptions: the seller is borrowing from you,
interest-free.

---

## Derivatives

### 16. Forward contracts

**Idea.** Agree today on the price of something delivered later. Both sides
post a deposit (margin) so neither can walk away.

**Example.** Grandma agrees now to buy 12 cookies from Ivy next Saturday for
20 NC. Each puts 5 NC in margin. On Saturday Ivy delivers, Grandma pays, and
both margins come back.

**Money.** Margin from both into escrow at agreement; the price paid on
delivery; margin to the other party if one side defaults.

**Reuses.** Offers for agreeing terms, catalog standard items for "what",
fulfillment for delivery, escrow.

**New.** A deferred settlement date and two-sided margin.

**Futures** are the next step: forwards on standardised items (the catalog's
standard items) with Nana as clearing house and daily settlement. Probably too
much for a household; forwards cover the lesson.

**Teaches.** Locking in a price removes uncertainty for both sides.

### 17. Options on scarce privileges

**Idea.** Pay a small premium for the right, but not the duty, to buy
something later at a fixed price.

**Example.** Movie night picks are scarce. Sam pays Ivy 1 NC for the option to
buy her pick next Friday for 5 NC. On Friday, if the film he wants is showing,
he exercises; if not, he lets it lapse and Ivy keeps the 1 NC.

**Money.** Premium from holder to writer at once. At expiry, either exercise
(holder pays the strike, the writer delivers) or lapse.

**Reuses.** Offers, fulfillment, the scheduled loop for expiry.

**New.** An option record with strike, expiry and exercise. The writer's
obligation to deliver is non-monetary, so enforcement is a dispute, as with
chores.

**Teaches.** Paying for flexibility, and why sellers of options want the
premium up front.

### 18. Weather derivatives

**Idea.** A contract that pays on the weather, to protect a plan that depends
on it.

**Example.** The lemonade stand loses money on rainy Saturdays. Ivy pays Nana
2 NC; if it rains on Saturday, Nana pays her 10 NC.

**Money.** Premium to the writer; payout from escrow if the event happens. The
writer must hold the full payout in escrow so it can always be paid.

**Reuses.** Escrow, Nana as writer or adjuster.

**New.** A settlement source everyone agrees on in advance ("the forecast site
at noon", or Nana's ruling). No outside data feed is needed.

**Teaches.** Hedging: this is insurance on a plan rather than a thing, and it
is how farmers and ski resorts manage bad seasons.

### 19. Dollar rate lock

**Idea.** Fix today the coins-per-dollar rate for a dollar purchase later.

**Example.** Sam is saving coins to buy a $20 game in two months. He locks
Nana's current rate for a 3 NC fee. If coins weaken, he still gets his $20.

**Money.** Fee to Nana. At the agreed date, Sam's coins to Nana and dollars to
Sam at the locked rate, through the existing dollar desk.

**Reuses.** Forex quotes and Nana's dollar ladder.

**New.** A forward quote honoured at a future date. Nana must limit total
locks to her dollar reserve (see Solvency in NANA_AS_MARKET_MAKER.md).

**Teaches.** Currency risk, and why companies that buy abroad hedge.

### 20. Event contracts (prediction market)

**Idea.** Buy "yes" or "no" shares on a household question. Each winning share
pays 1 NC.

**Example.** "Will the tomato plant have ripe fruit by July 1?" Yes trades at
0.3 NC, meaning the household thinks there is a 30% chance.

**Money.** Each pair of yes and no shares is backed by 1 NC in escrow. At
resolution, the escrow pays the winning side.

**Reuses.** Offers and forex-style quotes for trading, escrow, Nana to resolve.

**New.** Share pairs, a market per question, and resolution.

**Watch out.** This is the most gambling-shaped entry. Keep it to questions
people can influence or learn from, with small caps, and let Nana switch it
off. Never allow questions about a person's private life.

**Teaches.** Prices as forecasts, and that a confident price can still be
wrong.

---

## Summary

| # | Product | Family | Mostly reuses | Size |
| --- | --- | --- | --- | --- |
| 1 | Escrow for jobs | Protection | Fulfillment, escrow | Small |
| 2 | Warranty bond | Protection | Listings, disputes | Small |
| 3 | Breakage insurance | Protection | Pool account, approval | Medium |
| 4 | Chore-cover insurance | Protection | Pool, want-ads | Medium |
| 5 | Term deposits | Saving | Savings lotto, loans | Small |
| 6 | Rotating savings club | Saving | Lotto mechanics | Small |
| 7 | Layaway | Saving | Listings, escrow | Small |
| 8 | Business shares | Investing | Art editions | Medium |
| 9 | Index fund | Investing | Shares, loans, reports | Large |
| 10 | Project bonds | Investing | Gift requests, loans | Medium |
| 11 | Annuity | Investing | Loans in reverse | Small |
| 12 | Pawn loans | Credit | Loans, art ownership | Medium |
| 13 | Co-signed loans | Credit | Loan arrears | Small |
| 14 | Credit report | Credit | Loan history | Small, client side |
| 15 | Prepaid store credit | Credit | Nickles, fulfillment | Medium |
| 16 | Forwards | Derivative | Offers, catalog, escrow | Medium |
| 17 | Options | Derivative | Offers, fulfillment | Medium |
| 18 | Weather derivatives | Derivative | Escrow, adjuster | Small |
| 19 | Dollar rate lock | Derivative | Forex desk | Medium |
| 20 | Event contracts | Derivative | Quotes, escrow | Large |

## Where to start

Three shared pieces would unlock most of the list:

1. **A general escrow account** with hold, release and refund, plus a deadline
   in the event loop. Unlocks 1, 2, 7, 16, 17, 18 and 20.
2. **A pool with scheduled contributions** and adjuster-approved payouts.
   Unlocks 3, 4 and 6 (and term deposits are close).
3. **Member-to-Nana lending** (the member is the lender). Unlocks 5 and 11,
   and is already listed as needed in NANA_AS_MARKET_MAKER.md.

A good first pair: escrow for jobs (1), which fixes a real gap in today's
market, and the credit report (14), which needs no server change.

Every new product adds durable state, so follow
FORWARD_COMPATIBLE_DATA_CHANGES.md: new commands at the end of `Command`, new
state as trailing checkpoint rows, and a fixture test from the previous
firmware.
