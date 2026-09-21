//! Allocation-free reference arithmetic for fixed annual loan rates.
//!
//! These are calculations, not permission checks or money movements. The service
//! must validate the parties, persist an event, and only then apply its results.
//! Integrated contracts with selectable rate periods live in loans.rs.
//! All amounts use household minor units; remainders are fractions of that unit.
use crate::domain::{Error, MAX_AMOUNT, MAX_SEQUENCE};

pub const YEAR_SECONDS: u64 = 365 * 24 * 60 * 60;
pub const BASIS_POINTS: u64 = 10_000;
/// Actual elapsed seconds / a fixed 365-day year; no calendar or float rounding.
pub const INTEREST_DENOMINATOR: u64 = YEAR_SECONDS * BASIS_POINTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterestAccrual {
    pub coins: i64,
    /// Fraction of one ledger money unit, with INTEREST_DENOMINATOR as denominator.
    pub remainder: u64,
}

/// Simple annual interest on outstanding principal only. Accrue before each
/// principal change and carry the remainder unchanged into the next interval.
/// Rates are nonnegative; one basis point = 0.01%. There is no 100% ceiling.
pub fn accrue_interest(
    principal: i64,
    annual_rate_bps: u32,
    elapsed_seconds: u64,
    remainder: u64,
) -> Result<InterestAccrual, Error> {
    if !(0..=MAX_AMOUNT).contains(&principal)
        || elapsed_seconds > MAX_SEQUENCE
        || remainder >= INTEREST_DENOMINATOR
    {
        return Err(Error::InvalidInput);
    }
    let numerator = (principal as u128)
        .checked_mul(annual_rate_bps as u128)
        .and_then(|n| n.checked_mul(elapsed_seconds as u128))
        .and_then(|n| n.checked_add(remainder as u128))
        .ok_or(Error::Overflow)?;
    let coins = numerator / INTEREST_DENOMINATOR as u128;
    if coins > MAX_SEQUENCE as u128 {
        return Err(Error::Overflow);
    }
    Ok(InterestAccrual {
        coins: coins as i64,
        remainder: (numerator % INTEREST_DENOMINATOR as u128) as u64,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Payment {
    pub principal: i64,
    pub interest: i64,
    /// Unpaid portion of this requested installment, capped by total debt.
    /// The agreement must persist arrears separately from its future schedule.
    pub unpaid: i64,
}

impl Payment {
    pub fn total(self) -> i64 {
        self.principal + self.interest
    }
}

/// Interest first, then principal. Never creates an overdraft or pays more than
/// is owed. An existing correction overdraft means zero available funds.
/// This does not compound, forgive, or capitalize unpaid interest.
pub fn split_payment(
    principal: i64,
    interest_due: i64,
    requested: i64,
    borrower_balance: i64,
) -> Result<Payment, Error> {
    if !(0..=MAX_AMOUNT).contains(&principal)
        || !(0..=MAX_SEQUENCE as i64).contains(&interest_due)
        || !(1..=MAX_AMOUNT).contains(&requested)
        || !(-(MAX_SEQUENCE as i64)..=MAX_SEQUENCE as i64).contains(&borrower_balance)
    {
        return Err(Error::InvalidInput);
    }
    let owed = principal
        .checked_add(interest_due)
        .filter(|v| *v <= MAX_SEQUENCE as i64)
        .ok_or(Error::Overflow)?;
    let due = requested.min(owed);
    let paid = due.min(borrower_balance.max(0));
    let interest = paid.min(interest_due);
    Ok(Payment {
        principal: paid - interest,
        interest,
        unpaid: due - paid,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FundedBalances {
    pub lender: i64,
    pub borrower: i64,
}

/// Fund entirely from the lender's existing wallet. Nana uses this same rule.
/// Callers must reject self-deals, issuance accounts, and disabled parties.
pub fn funded_balances(
    lender_balance: i64,
    borrower_balance: i64,
    amount: i64,
) -> Result<FundedBalances, Error> {
    if !(1..=MAX_AMOUNT).contains(&amount)
        || !(-(MAX_SEQUENCE as i64)..=MAX_SEQUENCE as i64).contains(&lender_balance)
        || !(-(MAX_SEQUENCE as i64)..=MAX_SEQUENCE as i64).contains(&borrower_balance)
    {
        return Err(Error::InvalidInput);
    }
    if lender_balance < amount {
        return Err(Error::InsufficientFunds);
    }
    let borrower = borrower_balance
        .checked_add(amount)
        .filter(|v| *v <= MAX_SEQUENCE as i64)
        .ok_or(Error::Overflow)?;
    Ok(FundedBalances {
        lender: lender_balance - amount,
        borrower,
    })
}

/// Candidate for an already accepted, armed line of credit. Exactly zero is
/// intentional: this is not overdraft protection for an unaffordable purchase.
/// Agreement eligibility and remaining credit must be checked by the caller.
pub fn credit_at_zero(
    lender_balance: i64,
    borrower_balance: i64,
    draw_amount: i64,
) -> Result<Option<FundedBalances>, Error> {
    if !(1..=MAX_AMOUNT).contains(&draw_amount)
        || !(-(MAX_SEQUENCE as i64)..=MAX_SEQUENCE as i64).contains(&borrower_balance)
        || !(-(MAX_SEQUENCE as i64)..=MAX_SEQUENCE as i64).contains(&lender_balance)
    {
        return Err(Error::InvalidInput);
    }
    if borrower_balance != 0 {
        return Ok(None);
    }
    funded_balances(lender_balance, borrower_balance, draw_amount).map(Some)
}
