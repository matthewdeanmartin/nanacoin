# Household lending

Implemented in Rust `src/loans.rs`, `src/client/loans.rs`, `Service::tick`, and
Angular's Loans & credit screen. Demo lending follows the same cash rules in
tab-local memory. Money units and reforms are documented in FRACTIONAL_MONEY.md.

## Contracts

Any active member, including Nana, can offer a loan to another active member.
Only its borrower can accept it. Nana lends existing wallet cash: offering,
funding and repayment never issue currency. Ordinary loans fund on acceptance;
funds are checked then, not reserved when the offer is created.

Terms include principal, a nonnegative interest rate in basis points per
1/7/30/365 days, principal installment, and a 1/7/30-day payment interval. Interest
is simple on outstanding principal, with exact sub-unit remainders carried
forward. Rates over 100% are allowed; u32 basis points and checked intermediate
arithmetic bound them. Negative rates are rejected. The installment is principal
PLUS accrued interest, not an amortized total-payment quote. Final payoff waives
the remaining fraction smaller than one spendable unit, as disclosed on acceptance.

Credit is currently a one-time draw: borrower acceptance arms the offer, and
the event loop funds the stated amount only when the borrower is exactly at
zero and the lender has enough funds. This is not revolving credit or an
overdraft. Spending beyond available cash is still refused. Either party may
cancel an offered or armed contract. Loan postings do not trigger other credit;
outside cash activity must occur before credit can fund that account again.

## Scheduled events and recovery

Desktop and ESP32 run the same bounded event processor once per second. It
scans at most 32 contracts and commits at most four due events per pass, ordered
by deadline and loan ID. It needs a trusted wall clock. Overdue periods are
combined in constant time; actual payments are recorded at collection time,
not backdated. Interest accrues through collection time without compounding.

Automatic collection takes available cash, interest first, and retains unpaid
dues. No negative wallet balance, penalty, or automatic debt forgiveness is
created. New borrower funds permit another arrears attempt; idle scans do not
write repeated failed attempts. Disabled accounts or arithmetic limits pause
their contract, without blocking other contracts.

Internal events are forbidden through public commands. Service serialization,
validation, durable append, then state application make loan updates and their
ledger legs atomic. One event may produce separate principal and interest
postings. Restart replay, checkpoint restoration and keyed HTTP retries protect
against duplicate funding or collection. Storage failures latch the service
until recovery, as for other financial writes. Loan postings cannot be reversed
independently of their contract.

## API and UI

- GET `/api/v1/loans`: contracts visible to the parties or Nana, aggregate
  outstanding principal/overdue amounts, and principal-weighted annual simple
  interest rate. Zero-rate active principal counts; undrawn credit does not.
- POST `/api/v1/loans`: offer terms; borrower is an account ID.
- POST `/api/v1/loans/{id}/accept`, `/close`, `/repay` (amount in minor units).

Angular provides offers, borrower consent, credit offers, manual repayments,
status, due dates and paused-funding explanations. Economy headline definitions
describe their measurement windows and rate normalization. Interest payments
are tracked separately: pure interest transfers and loan principal are excluded
from household GDP. Explicit goods/labor production counts; gifts and
unclassified transfers do not. Banking-service fees/FISIM are not implemented.

## Board limits

32 fixed contract slots; oldest terminal slots recycle, active contracts do not.
No unbounded event queue, one task per loan, dynamic decimal library, or database
is added. Journal frames remain 1024 bytes; checkpoints add one bounded row per
contract. The ESP32 event worker uses a dedicated 24 KiB stack and the existing
service mutex. Firmware image size must be checked by the build script; a
successful build is not evidence of measured live heap/stack headroom.
