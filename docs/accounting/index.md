# How accounting works in NanaCoin

This page explains how a NanaCoin bank keeps its books: which accounts exist,
what every kind of payment does to them, what the board checks before and
after each write, and which numbers the reports are built from. It covers what
is implemented first and what is planned last.

NanaCoin's accounting doesn't follow GAAP, IFRS or any other country's
bookkeeping tradition. It is the smallest set of rules that keeps a
household's money honest and still fits on a microcontroller. Where a familiar
accounting word fits, this page uses it; where it doesn't, it says what
NanaCoin does instead.

The code referred to below lives in `nanacoin_rs/src`: the domain in
`domain.rs`, flow totals and corrections in `ledger.rs`, and one module per
feature (`forex.rs`, `loans.rs`, `lotto.rs`, `offers.rs`, `commerce.rs`,
`fulfillment.rs`, `money.rs`).

## The one rule: the books add up

A bank has a fixed, small set of **accounts**. Each account has a balance in
exactly one **asset**. Today there are two assets:

- the household's own coin (named at setup, for example "NanaCoin"), and
- recorded US dollars, kept in cents.

Every movement of money is a **transaction**: an amount of one asset leaves
one account and arrives in another. Nothing else changes a balance. So for
each asset, the sum of all balances is always exactly zero:

```
coin:  issuance + lotto escrow + every member's coin balance = 0
USD:   USD issuance + every member's USD balance            = 0
```

`State::check_invariants` checks both sums after replay and checkpoint load,
and the board refuses to run on a journal where they don't hold (it reports
`CorruptJournal`). This is the double-entry idea without the paperwork: each
transaction is one debit and one matching credit, so the total can never
drift.

The trick that makes the sum zero is the **issuance account**: it is where
money comes from and where retired money goes back, and it is the only
account allowed to be negative in normal use.

## Units

- Every amount is an integer count of **minor units** (`i64`). There is no
  floating point anywhere in the ledger.
- The coin has a configurable number of decimal places, 0 to 8, default 4:
  `123456` means 12.3456 coins. USD always has 2 (cents).
- A single transaction is at most 10^15 minor units (`MAX_AMOUNT`). A balance
  may not exceed 2^53 − 1 (`MAX_SEQUENCE`), so every number stays exact in a
  browser's JavaScript too.
- Intermediate arithmetic (interest, exchange prices, reforms) is checked
  `i128`/`u128`. Anything that would overflow, or that would need rounding
  away a minor unit, is refused, not rounded.

Details and the reform procedure: `nanacoin_rs/spec/FRACTIONAL_MONEY.md`.

## Accounts

### Accounts that hold money

| Account | Where | Asset | Can be negative? | What it means |
|---|---|---|---|---|
| **Member wallet** | `Member.balance` | coin | only through a correction | A member's spendable coins. |
| **Member USD wallet** | `Member.usd_cents` | USD | only through a correction | Real dollars recorded as belonging to the member. |
| **Nana's wallet** | a member wallet whose member has role `Nana` | coin, USD | only through a correction | Nana is a member like any other: she spends, lends and trades from her wallet. She is not the issuance account. |
| **Issuance** | `State.issuance_balance`, `MemberId(0)`, shown as `account:system-issuance` | coin | yes, always | The source of new coins and destination of retired ones. Its balance is minus the money supply. |
| **USD issuance** | `State.usd_issuance_balance`, `MemberId(0)` on USD legs, shown as `account:usd-issuance` | USD | yes, always | The door between the real world and the ledger: minus all real dollars recorded into the household. |
| **Lotto escrow** | `State.lotto_escrow`, `MemberId(255)` | coin | no | Ticket money held until a lotto pays out. Counted in the money supply, but belongs to nobody until the draw. |

There are at most 16 members (`MEMBERS`), and account IDs are one byte:
`0` is issuance, `1..=16` are members, `255` is lotto escrow.

**Money supply** (called `circulation` in the API) is `−issuance_balance`. It
equals every member's coins, including Nana's, plus lotto escrow.

### Records that are not accounts

Several features create **promises**, not balances. They are bounded records
beside the ledger. Money moves only when a promise is kept, and it moves as an
ordinary transaction.

| Record | Holds money? | What it records |
|---|---|---|
| Loan (`loans.rs`) | no | Principal still owed, interest accrued (with a sub-unit remainder), what is due and when. The borrower's debt is a number in the loan, not a negative wallet. |
| Offer (`offers.rs`) | no | A negotiated deal. After acceptance it keeps its settlement, including the undo deadline. |
| Quote (`forex.rs`) | no | A standing promise to buy or sell coins for dollars at a price. Nothing is reserved when it is posted. |
| Listing | no | Something for sale or wanted, at a price. |
| Gift request (`commerce.rs`) | no | A request for gifts, with the running net total received. |
| Artwork (`commerce.rs`) | no | Ownership of a unique digital edition. A sale is a coin payment plus a change of owner in one event. |
| Fulfilment (`fulfillment.rs`) | no | Whether paid-for work or goods were delivered (to do, done, disputed, reversed). Delivery and payment are tracked separately. |
| Lotto (`lotto.rs`) | no (its escrow does) | Tickets per member, pool, interest promised, the winner, and which payout step is next. |

Because a promise is not an account, posting one reserves nothing. A quote or
loan offer can fail when it is taken if the maker's wallet has run low. That
is deliberate: it keeps the books to a handful of balances, and the check
happens at the moment money actually moves.

## Transactions

A transaction (`Transaction` in `domain.rs`) records:

- `from`, `to`, `amount`, and whether it is a USD leg (`usd`);
- who asked for it (`actor`) and the server time (`created_at`);
- a memo;
- links to what caused it: `listing`, `quote`, `loan`, `lotto`, and in
  `meta`, `gift_request` and `art`;
- `reverses`: the transaction it corrects, if any;
- an economic classification (`economic`: kind, catalogue thing, quantity,
  unit);
- `meta.epoch` and `meta.decimals`: which currency epoch and precision the
  amount is written in;
- `meta.ordinal` (position in the ledger) and `meta.group` (the journal event
  that produced it).

### Events, legs and atomicity

The journal stores **events** (commands), not transactions. Replaying an
event produces zero, one or several transactions, which are called legs. All
legs of one event are applied together or not at all, because the event is
one durable journal record. Examples:

- taking a dollar quote writes a coin leg and a USD leg;
- a loan payment writes a principal leg and an interest leg;
- creating a member with a starting grant writes one issuance leg.

Transaction IDs come from the event sequence. A second leg gets its own ID
range (for example, the USD leg of a quote gets the event sequence plus
`MAX_RECORDS`), so every leg has a stable, unique ID.

### Checks before money moves

`validate_posting` and `validate_currency_posting` apply to every leg:

1. `from` and `to` are different, and the amount is between 1 and
   `MAX_AMOUNT`.
2. No balance may exceed the exact-integer limit on either side.
3. **No overdraft:** a normal payment may not take a wallet below zero. Only
   issuance can go negative.
4. **Disabled members** cannot pay or be paid, except through corrections.
5. A **correction** made by Nana may overdraw a wallet. This is the one way a
   member balance can go negative: if the money has already been spent, the
   reversal still has to happen, and it leaves a visible debt.

Validation runs before the event is written, and replay applies the same
rules. An event that passed validation always applies cleanly.

## Every kind of money movement

Coin legs unless noted. "Issuance" means `MemberId(0)`.

| What happens | From | To | Notes |
|---|---|---|---|
| Nana issues coins | issuance | member | Money supply goes up. |
| Starting grant for a new member | issuance | new member | `CreateMember` with `grant`; memo "Initial household allocation". |
| Nana retires coins | member | issuance | Money supply goes down. A visible ledger entry, never a silent edit. |
| Transfer, gift, allowance | member | member | `Transfer`, or `ClassifiedTransfer` with an economic kind. A classified labour or goods transfer also opens a fulfilment. |
| Marketplace purchase (sell listing) | buyer | seller | `Buy`. The listing becomes sold and a fulfilment opens. |
| Marketplace purchase (buy listing) | the member who posted the want | the member who fills it | Same command, parties reversed. |
| Accepted offer | payer | payee | Paid in full immediately. It can be undone until `settles_at` (default 48 hours, set by the household). |
| Good deed reward | **issuance** | member | Nana's standing "good deed" BUY listing pays in new coins, not from her wallet, and the listing stays open. Undoing it retires the reward. |
| Gift to a gift request | giver | requester | Tagged with `gift_request`; the request's net total goes up. |
| Buying digital art | buyer | current owner | Ownership changes in the same event. Art sales cannot be refunded (see below). |
| Lotto tickets | member | lotto escrow | |
| Lotto principal payout | lotto escrow | winner (simple and delayed) or each ticket holder (savings) | One leg per settlement step, so a restart mid-payout resumes at the next step. |
| Lotto interest | house (the lotto's creator) | winner | As much as the house can pay from its wallet. |
| Lotto interest shortfall | **issuance** | winner | Whatever the house could not pay is issued. The house's promise is kept by making money. |
| Loan drawn | lender | borrower | Funded only from the lender's existing wallet. No lending from issuance, including for Nana. |
| Credit line drawn | lender | borrower | An armed credit line draws automatically only when the borrower's balance is exactly zero, and not straight after a loan payment. It is not overdraft protection. |
| Loan repayment | borrower | lender | Interest first, then principal, in two legs, and never more than the borrower has. What can't be paid stays due in the loan record. |
| Recording real dollars | **USD issuance** | member USD | `IssueUsd`, a USD leg. Real cash came into the household. |
| Coins-for-dollars trade | seller | buyer (coin leg) plus buyer → seller (USD leg) | Taking a quote. Both legs are one event, so the swap is atomic. |
| Refund | original payee | original payer | Partial or full (see corrections). |
| Reversal | original payee | original payer | Full correction by Nana, or by the recipient for money they received. |
| Currency reform | none | none | Every balance is rescaled in one event. No money moves (see below). |

### Classification: what counts as production

Each transaction has an economic kind (`EconomicKind`): `LABOR`, `GOOD`,
`GIFT`, `LOAN_PRINCIPAL`, `INTEREST` or `OTHER`. Kinds don't change balances.
They tell the reports what the money was for:

- **Labour and goods** are household production (the "GDP" on the Economy
  page).
- **Gifts, loan principal, interest and exchanges** are transfers of money,
  not production. A loan isn't income for the borrower, and principal repaid
  isn't income for the lender.
- A listing can name a catalogue **thing** with a unit and quantity (for
  example 2 hours of lawn mowing), so prices can be compared over time for
  inflation.

## Corrections: nothing is ever edited

A mistake is never fixed by changing an old transaction. It is fixed by adding
a new transaction that moves the money back, with `reverses` pointing at the
original.

- **Refund** (`Refund`): part or all of a payment goes back to the payer. The
  recipient can refund money they received; Nana can refund anything eligible.
  The ledger's `Correction` record keeps the running total refunded, so no
  payment can be refunded more than once in total.
- **Reversal** (`Reverse`): a full correction in one step. Nana may reverse
  any ordinary entry, including issuance and USD recording. Members may
  reverse only money they received, and never issuance, USD or an exchange
  leg.
- **Undoing an accepted offer** (`UnacceptOffer`) before its deadline is a
  reversal too, and the listing becomes available again.
- **Cannot be corrected with a refund or reversal:** loan and lotto legs (the
  contract has its own state, and a reversal would contradict it), a
  transaction that is itself a correction, and art sales (ownership would
  have to move back as well). Refunds also exclude issuance, retirement, USD
  and exchange legs.

A reversal keeps the original's economic kind, and the reports book it against
the original's category with a minus sign. A reversed purchase cancels the
purchase; it doesn't look like a sale.

If a correction reaches back across a currency reform, the refund is
converted to the current epoch's units (`current_amount`), and `refund_units`
remembers the amount in the original's units.

## Currency reform and epochs

Nana can change the currency's precision, redenominate it (for example
1 new coin = 1,000 old coins), or both (`ReformCurrency`). A reform is one
event that rescales every coin amount the bank holds: balances, issuance,
lotto escrow and pools, listing and offer prices, quotes, loan terms and
accrued interest, gift targets and art prices.

- The conversion must be exact. If rescaling would drop even one minor unit
  or a fraction of accrued interest, the reform is refused.
- USD amounts are untouched. Exchange quotes change their price so that their
  value stays the same.
- Each reform starts a new **money epoch**. Every transaction records the
  epoch and precision it was written in, so history stays readable.
- At most 32 epochs (`EPOCHS`).

## Totals that survive forgetting

A board cannot keep every transaction forever. The S3 keeps 3,000 recent
transactions in RAM and the S2 keeps 300. Older ones go to a bounded flash
archive, and eventually they are gone. The reports need some lifetime
numbers anyway, so the ledger keeps **flow totals per epoch**
(`Epoch.flows`, 20 counters, updated by `ledger_post`):

| Group | Counters |
|---|---|
| Money supply | issued to Nana, issued to members, issued for lotto interest, retired, corrections to issuance, net issuance |
| Dollars | USD recorded; USD paid out and taken in by Nana's exchange trades |
| Nana's exchange desk | coins bought back, coins sold |
| Nana's lending | lent, repaid, interest received, interest paid |
| Nana's trading | bought from members, sold to members |
| Nana's other flows | paid out (allowances, gifts), received otherwise |
| | transaction count |

These totals are exact `i128` numbers and are part of the checkpoint, so
they survive compaction. What is not kept once history is gone is the detail:
who paid whom, and when.

Other durable pieces:

- **Balances** are never evicted. Only history is.
- **Corrections** (`ledger.corrections`, bounded) keep refund totals for
  payments that were partly or fully corrected, so refund limits still hold
  after the original has left RAM.
- **Fulfilments** keep a copy of their payment, so a disputed purchase can
  still be reversed after the payment has left recent history.
- The **audit log** records every business command and identity change
  (never credentials).

## Nana's balance sheet

The Central Bank page (`spec/NANA_AS_CENTRAL_BANK.md`) presents Nana's
position without a formal balance sheet:

- **Holds:** her recorded dollars (the reserve), her own coins, and loans owed
  to her.
- **Promised:** dollars for her open BID quotes, coins for her open ASK
  quotes, lotto interest still to pay, ticket money in her lottos, and loan
  offers not yet taken.
- **Reserve ratio:** dollars held ÷ dollars she would owe if every BID were
  taken.
- **Backing per coin:** dollars held ÷ coins held by the household.
- **Seigniorage:** net new coins, valued at the latest exchange price.

Nana's own coins are listed as a holding because she spends and lends from
them, even though an issuer can always make more. She cannot overdraw: when
she is short, she issues coins to herself first, and that shows up as
"issued to Nana" in the money-supply flows.

## Where accounting happens in the code

| Step | Function | File |
|---|---|---|
| Validate a command | `State::validate_at` and the feature validators | `domain.rs`, feature modules |
| Validate a leg | `validate_posting`, `validate_currency_posting` | `domain.rs`, `forex.rs` |
| Append the event durably | journal append (see [storage](../rust/storage.md)) | `journal.rs` |
| Apply the legs | `State::replay` → `record_transaction` | `domain.rs` |
| Update flows and corrections | `ledger_post` | `ledger.rs` |
| Check the books | `check_invariants`, `check_ledger`, `check_lottos` | `domain.rs`, `ledger.rs`, `lotto.rs` |

The order matters: validate, write durably, then apply. Applying never fails,
and a crash before the write loses nothing that a member was told had
happened.

---

## Not implemented yet

Everything below is design direction, not current behaviour.

### Separate accounts from members

Today an account ID is a member ID, plus two reserved numbers (0 and 255).
The roadmap separates three things: the **actor** who logs in, the **party**
who owns an account (a member, a corporation, another bank), and the
**account** itself. Needed for corporations (a shared treasury operated by
authorized members), bearer-voucher reserves, and foreign banks' accounts.

### Asset IDs instead of a USD flag

`Transaction.usd: bool` becomes an asset ID: the local coin, USD, or a
foreign bank's coin. Each asset keeps its own sum-to-zero rule and its own
issuance-like **outside account**. The invariant becomes:

```
for each asset:  sum of all accounts holding it = 0
```

### Foreign coin and correspondent accounts

Owner decisions (September 2026): each bank issues its own currency; money
between banks is exchanged, not moved; **every member may hold foreign coin**;
exchange rates float; one bank may overdraw with another only if the other
bank allows it.

Suppose bank A (currency A-coin) trades with bank B (B-coin). A's books gain:

| Account | Asset | Meaning |
|---|---|---|
| Member B-coin wallets | B-coin | What each A member holds in B's currency. |
| **Nostro at B** (A's outside account for B-coin) | B-coin | Minus the total B-coin that A says it holds at B. It plays the part that USD issuance plays for dollars. |
| **B's vostro account** (B's account at A) | A-coin | A-coin held by bank B on behalf of B's members. An ordinary account, allowed below zero only down to the credit limit A grants B. |

A's sum-to-zero holds for B-coin because member B-coin wallets + nostro = 0.
The other half of the story lives on B's books: B keeps an account for A
whose balance must equal what A's nostro says. The two banks check this
against each other with signed **statements**. A mismatch freezes the link.

Foreign coin held by A's members is B's money in circulation abroad. It
counts in B's money supply and never in A's.

Trading starts with an **opening purchase**: each bank's Nana buys some of
the other bank's currency, paying in her own. That one agreed ratio is the
only exchange rate anyone sets; after it, rates float on each bank's order
books. Every member may then trade internationally.

If B lets A's account at B go below zero (an overdraft B allows), B charges
**interest in B-coin**, its own currency, accrued on B's clock with the same
arithmetic as household loans. Interbank credit creates money the same way a
loan does, so B's money-supply report counts it as "credit to foreign banks".

The bank only performs **atomic transactions**. "Convert my coins and buy
that foreign listing" is two transactions that the client runs one after
the other. If the purchase fails, the member keeps the foreign coin they
bought.

Each bank can federate with only a few peers, because every member wallet
needs one balance per foreign currency held. With 16 members and 4 peers,
that is 64 extra balances.

The protocol is in
[`nanacoin_rs/spec/INTERBANK_PROTOCOL.md`](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/spec/INTERBANK_PROTOCOL.md).

### Money in transit

When A sends money to B, A's leg is final before B has seen the message. For
that gap, A shows the amount in an **in-transit** state tied to the outgoing
message number. It clears when B confirms, or reverses when B returns the
payment. In-transit amounts are part of the reconciliation, so "sent but not
yet arrived" is never invisible.

### Winding down a link

If two banks stop trusting each other, the link is frozen and the accounts are
closed out. There are two options, and the owner chooses per case:

1. **Buy-back.** Each bank's Nana buys back all of its members' foreign coin
   at a final rate, paying in local coin. Then the two banks net their
   positions against each other, and the remainder is paid outside the system
   (usually in real dollars, recorded as USD legs).
2. **Everyone to dollars.** Members convert foreign coin to recorded USD, and
   the bank that owes the net amount pays it to the other bank in real
   dollars.

Either way, the final statement of both banks must show zero.

### Bearer vouchers ("Nana-nickles")

Coins packaged into a voucher move to a **bearer reserve** account; redeeming
a voucher moves them to the redeemer. The reserve's balance always equals the
face value of the vouchers still outstanding. See `spec/NANANICKLES_PROPOSAL.md`.

### Other planned records

| Feature | Accounting consequence |
|---|---|
| Corporations | Company accounts; payments record the human who acted. |
| Standing orders / allowances on the board | A mandate record; each occurrence is an ordinary transfer that points back to it. |
| Loan forgiveness and write-offs | Adjustment events that reduce what is owed without pretending a payment happened. |
| USD withdrawal | An explicit "cash left the household" USD leg back to USD issuance, distinct from correcting a recording mistake. |
| Partial refunds with disputes | Refunds linked to the dispute, with the remaining eligible amount. |
| Long-term reports and backup | Monthly summaries with their own coverage, and a complete private backup of balances, promises and corrections. |
| Demurrage (a fee for holding money) | Not planned while a dollar exit exists; it would push people into dollars. |
