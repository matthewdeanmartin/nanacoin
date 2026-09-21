use nanacoin::{
    domain::{Error, MAX_AMOUNT, MAX_SEQUENCE},
    events::due_occurrence,
    lending::*,
    offers::MIN_CLOCK,
};

#[test]
fn interest_is_exact_and_independent_of_check_frequency() {
    let yearly = accrue_interest(137, 725, YEAR_SECONDS, 0).unwrap();
    let mut coins = 0;
    let mut remainder = 0;
    for _ in 0..365 {
        let daily = accrue_interest(137, 725, 86_400, remainder).unwrap();
        coins += daily.coins;
        remainder = daily.remainder;
    }
    assert_eq!(coins, 9);
    assert_eq!(coins, yearly.coins);
    assert_eq!(remainder, yearly.remainder);
    assert_eq!(remainder, INTEREST_DENOMINATOR * 9325 / 10_000);
}

#[test]
fn fractional_interest_survives_a_principal_payment() {
    let first = accrue_interest(100, 100, YEAR_SECONDS / 2, 0).unwrap();
    assert_eq!(first.coins, 0);
    let second = accrue_interest(50, 100, YEAR_SECONDS, first.remainder).unwrap();
    assert_eq!(
        second,
        InterestAccrual {
            coins: 1,
            remainder: 0
        }
    );
}

#[test]
fn interest_does_not_compound_and_zero_rates_work() {
    let first = accrue_interest(100, 1000, YEAR_SECONDS, 0).unwrap();
    let second = accrue_interest(100, 1000, YEAR_SECONDS, first.remainder).unwrap();
    assert_eq!(first.coins + second.coins, 20);
    assert_eq!(
        accrue_interest(100, 0, YEAR_SECONDS, 17).unwrap(),
        InterestAccrual {
            coins: 0,
            remainder: 17
        }
    );
    assert_eq!(accrue_interest(0, 1000, YEAR_SECONDS, 17).unwrap().coins, 0);
}

#[test]
fn invalid_interest_inputs_and_unsafe_totals_are_rejected() {
    for (principal, rate, seconds, remainder) in [
        (-1, 100, 1, 0),
        (MAX_AMOUNT + 1, 100, 1, 0),
        (1, 100, MAX_SEQUENCE + 1, 0),
        (1, 100, 1, INTEREST_DENOMINATOR),
    ] {
        assert_eq!(
            accrue_interest(principal, rate, seconds, remainder),
            Err(Error::InvalidInput)
        );
    }
    assert_eq!(
        accrue_interest(MAX_AMOUNT, 10_000, MAX_SEQUENCE, 0),
        Err(Error::Overflow)
    );
    assert_eq!(
        accrue_interest(MAX_AMOUNT, 10_000, YEAR_SECONDS, 0)
            .unwrap()
            .coins,
        MAX_AMOUNT
    );
}

#[test]
fn payments_are_interest_first_and_limited_by_actual_funds() {
    let paid = split_payment(100, 3, 20, 8).unwrap();
    assert_eq!(
        paid,
        Payment {
            principal: 5,
            interest: 3,
            unpaid: 12
        }
    );
    assert_eq!(paid.total(), 8);
    let interest_only = split_payment(100, 3, 20, 2).unwrap();
    assert_eq!(
        interest_only,
        Payment {
            principal: 0,
            interest: 2,
            unpaid: 18
        }
    );
    for balance in [0, -7] {
        assert_eq!(
            split_payment(100, 3, 20, balance).unwrap(),
            Payment {
                principal: 0,
                interest: 0,
                unpaid: 20
            }
        );
    }
}

#[test]
fn final_payment_never_exceeds_debt_and_does_not_invent_arrears() {
    assert_eq!(
        split_payment(2, 1, 20, 20).unwrap(),
        Payment {
            principal: 2,
            interest: 1,
            unpaid: 0
        }
    );
    assert_eq!(split_payment(0, 0, 20, 20).unwrap().total(), 0);
    assert_eq!(
        split_payment(1, MAX_SEQUENCE as i64, 1, 1),
        Err(Error::Overflow)
    );
    assert_eq!(split_payment(1, -1, 1, 1), Err(Error::InvalidInput));
    assert_eq!(split_payment(1, 0, 0, 1), Err(Error::InvalidInput));
}

#[test]
fn funding_conserves_coins_and_never_uses_issuance() {
    let funded = funded_balances(100, 20, 30).unwrap();
    assert_eq!(
        funded,
        FundedBalances {
            lender: 70,
            borrower: 50
        }
    );
    assert_eq!(funded.lender + funded.borrower, 120);
    assert_eq!(funded_balances(29, 20, 30), Err(Error::InsufficientFunds));
    assert_eq!(funded_balances(-1, 20, 30), Err(Error::InsufficientFunds));
    assert_eq!(
        funded_balances(100, MAX_SEQUENCE as i64, 1),
        Err(Error::Overflow)
    );
    assert_eq!(funded_balances(100, 0, 0), Err(Error::InvalidInput));
}

#[test]
fn credit_triggers_at_exactly_zero_and_still_requires_funding() {
    assert_eq!(
        credit_at_zero(100, 0, 30).unwrap(),
        Some(FundedBalances {
            lender: 70,
            borrower: 30
        })
    );
    assert_eq!(credit_at_zero(100, 1, 30).unwrap(), None);
    assert_eq!(credit_at_zero(100, -1, 30).unwrap(), None);
    assert_eq!(credit_at_zero(29, 0, 30), Err(Error::InsufficientFunds));
}

#[test]
fn schedules_are_anchored_and_return_only_one_occurrence() {
    let first = MIN_CLOCK + 86_400;
    assert_eq!(due_occurrence(first, 86_400, first - 1).unwrap(), None);
    let due = due_occurrence(first, 86_400, first).unwrap().unwrap();
    assert_eq!(due.due_at, first);
    assert_eq!(due.next_due_at, first + 86_400);
    assert_eq!(
        due_occurrence(due.next_due_at, 86_400, first).unwrap(),
        None
    );
    // A year offline does not cause this helper to allocate or loop for a year.
    assert_eq!(
        due_occurrence(first, 86_400, first + YEAR_SECONDS).unwrap(),
        Some(due)
    );
    // No persisted advance means the same candidate can be retried after failure.
    assert_eq!(due_occurrence(first, 86_400, first).unwrap(), Some(due));
}

#[test]
fn schedules_reject_untrusted_clock_and_overflow() {
    assert_eq!(due_occurrence(MIN_CLOCK, 1, 0), Err(Error::Unavailable));
    assert_eq!(
        due_occurrence(MIN_CLOCK, 1, MAX_SEQUENCE + 1),
        Err(Error::Unavailable)
    );
    assert_eq!(due_occurrence(0, 1, MIN_CLOCK), Err(Error::InvalidInput));
    assert_eq!(
        due_occurrence(MIN_CLOCK, 0, MIN_CLOCK),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        due_occurrence(MAX_SEQUENCE, 1, MAX_SEQUENCE),
        Err(Error::Overflow)
    );
}

#[test]
fn arithmetic_results_fit_small_fixed_records() {
    assert!(std::mem::size_of::<InterestAccrual>() <= 16);
    assert!(std::mem::size_of::<Payment>() <= 24);
    assert!(std::mem::size_of::<nanacoin::events::DueOccurrence>() <= 16);
}
