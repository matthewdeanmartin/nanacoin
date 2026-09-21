# Fractional money and decimal reforms

Implemented in `src/money.rs`, the domain, and Angular `api/money.ts`.
This is development schema version 2. Existing development data is disposable;
see the repository AGENTS.md. There is no old-journal migration path.

## Units and limits

All NC amount, balance, price, grant, principal and installment fields are exact
integer **minor units**. Default precision is four decimal places: 123456 means
12.3456 NC. Household precision is configurable from zero through eight places.
Rust stores i64 values, with checked i128/u128 intermediates. JSON amounts stay
numbers within JavaScript's safe integer range. The transaction cap is
1,000,000,000,000,000 minor units; balances and loan debt cannot exceed
9,007,199,254,740,991. Aggregate lending totals are decimal integer strings.
Precision therefore trades spendable granularity against the maximum whole-NC
amount. This implementation does not offer unlimited integer magnitudes.

Angular parses decimal text using BigInt arithmetic and locale decimal symbols
and digits, rejecting excess places instead of rounding. Formatting divides
integer units exactly. Chart coordinates and statistical ratios use approximate
floating-point presentation only. Full translation of the app remains separate.
USD wallets still use USD cents. Forex rates are USD cents per whole NC, with
the NC scale stored on quotes. A quote must settle to an exact positive USD cent.

## Nana's reforms

The Household screen offers a preview followed by an explicit reform action.
`POST /api/v1/admin/reform` accepts `decimals`, `power`, `expected_epoch`,
`expected_sequence`, and `preview`. One new NC equals 10^power old NC; power is
between -12 and 12. The conversion exponent for stored minor units is
`new_decimals - old_decimals - power`.

Examples: changing precision from 4 to 6 with power 0 multiplies stored amounts
by 100 while preserving the displayed NC value. Precision 4 to 7 with power 3
keeps stored integers unchanged and makes 1 new NC equal 1,000 old NC. Repeated
reforms provide a way to respond to inflation or deflation within finite bounds.

Validation scans all retained NC balances, transactions, listings, offers,
quotes, loan terms, accrued interest and fractional interest before writing one
durable event. Division must be exact: a reform that would discard even a minor
unit or fractional interest is refused. USD balances stay unchanged; forex
rates change to preserve their value. Retained NC history is normalized too,
so the reform does not create an artificial chart price jump. There is no
rounding pool, debt forgiveness, or selective balance conversion.

Every reform increments the money epoch. Preview sequence and epoch checks
reject intervening changes. Mutation keys include `g<generation>:m<epoch>:...`;
stale forms are rejected after a reform. Angular stops money entry and requires
a reload when it detects a changed epoch. Browser-local allowances must be
recreated after a reform. Replay/checkpoints preserve the scale and epoch.

Limits are explicit, not an assumption of stable household inflation. An
inexact proposed reform can require increasing precision or choosing a different
factor. No schema migration is needed for a supported decimal reform.
