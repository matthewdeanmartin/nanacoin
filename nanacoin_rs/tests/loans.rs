mod common;
use nanacoin::{auth::PasswordVerifier, domain::*, journal::*, loans::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const START: u64 = 1_700_000_000;
thread_local! { static NOW: Cell<u64> = const { Cell::new(START) }; }
fn now() -> u64 {
    NOW.with(Cell::get)
}
fn time(t: u64) {
    NOW.with(|n| n.set(t));
}
#[derive(Clone, Default)]
struct Memory {
    frames: Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>,
    fail: Rc<Cell<u8>>,
}
impl Journal for Memory {
    fn read(&mut self, i: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(f) = self.frames.borrow().get(i) {
            *frame = *f;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, i: usize, f: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        if self.fail.get() == 1 {
            return Err(Error::Storage);
        }
        assert_eq!(i, self.frames.borrow().len());
        self.frames.borrow_mut().push(*f);
        if self.fail.get() == 2 {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}
fn exec<J: Journal>(s: &mut Service<J>, who: u8, c: Command) -> Result<Receipt, Error> {
    let request = s.state().member(MemberId(who))?.last_request + 1;
    s.execute(MemberId(who), request, c)
}
fn setup<J: Journal>(s: &mut Service<J>) {
    common::provision(s);
    for name in ["Alice", "Bob"] {
        exec(
            s,
            1,
            Command::CreateMember {
                username: Name::try_from(name).unwrap(),
                display_name: Name::try_from(name).unwrap(),
                password: PasswordVerifier::hash("1234").unwrap(),
                role: Role::User,
                grant: 10000,
                mastodon_id: MastodonId::new(),
            },
        )
        .unwrap();
    }
}
fn house() -> (Service<Memory>, Memory) {
    time(START);
    let m = Memory::default();
    let mut s = Service::open_with_clock(m.clone(), now).unwrap();
    setup(&mut s);
    (s, m)
}
fn balance<J: Journal>(s: &Service<J>, who: u8) -> i64 {
    s.state().member(MemberId(who)).unwrap().balance
}
fn offer<J: Journal>(s: &mut Service<J>, lender: u8, borrower: u8, credit: bool, rate: u32) -> u64 {
    exec(
        s,
        lender,
        Command::OfferLoan {
            terms: LoanTerms {
                borrower: MemberId(borrower),
                amount: 10000,
                installment: 2000,
                rate_bps: rate,
                rate_days: 1,
                payment_days: 1,
                credit,
                memo: Memo::new(),
            },
        },
    )
    .unwrap()
    .sequence
}
fn transfer<J: Journal>(s: &mut Service<J>, from: u8, to: u8, amount: i64) {
    exec(
        s,
        from,
        Command::Transfer {
            to: MemberId(to),
            amount,
            memo: Memo::new(),
        },
    )
    .unwrap();
}

#[test]
fn cash_funding_consent_and_permissions() {
    let (mut s, _) = house();
    let id = offer(&mut s, 1, 2, false, 100);
    assert_eq!(
        exec(&mut s, 3, Command::AcceptLoan { loan: id }),
        Err(Error::Forbidden)
    );
    assert_eq!(
        exec(&mut s, 2, Command::AcceptLoan { loan: id }),
        Err(Error::InsufficientFunds)
    );
    let issuance = s.state().issuance_balance;
    transfer(&mut s, 3, 1, 10000);
    exec(&mut s, 2, Command::AcceptLoan { loan: id }).unwrap();
    assert_eq!(balance(&s, 1), 0);
    assert_eq!(balance(&s, 2), 20000);
    assert_eq!(s.state().issuance_balance, issuance);
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::RepayLoan {
                loan: id,
                amount: 100
            }
        ),
        Err(Error::Forbidden)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::RunLoan {
                loan: id,
                expected_updated_at: START
            }
        ),
        Err(Error::Forbidden)
    );
    s.state().check_invariants().unwrap();
}

#[test]
fn scheduled_interest_is_simple_partial_and_catches_up_once() {
    let (mut s, m) = house();
    let id = offer(&mut s, 2, 3, false, 100); // 1% daily; deliberately not capped at 100% annual.
    exec(&mut s, 3, Command::AcceptLoan { loan: id }).unwrap();
    transfer(&mut s, 3, 2, 19500); // Leave 500, less than the first installment.
    time(START + 3 * DAY);
    assert_eq!(s.tick().unwrap(), 1);
    let l = s.state().loan(id).unwrap();
    assert_eq!(
        (l.principal, l.interest, l.principal_due, l.interest_due),
        (9800, 0, 5800, 0)
    );
    assert_eq!(balance(&s, 3), 0);
    let sequence = s.state().sequence;
    assert_eq!(s.tick().unwrap(), 0);
    drop(s);
    let mut s = Service::open_with_clock(m, now).unwrap();
    assert_eq!(s.tick().unwrap(), 0);
    assert_eq!(s.state().sequence, sequence);
    transfer(&mut s, 2, 3, 1000);
    assert_eq!(s.tick().unwrap(), 1); // Collect arrears after outside income.
    assert_eq!(s.state().loan(id).unwrap().principal, 8800);
    assert_eq!(s.state().loan(id).unwrap().principal_due, 4800);
    assert!(s.state().history.iter().any(|t| t.loan == Some(id)
        && t.economic.kind == EconomicKind::Interest
        && t.amount == 300));
    s.state().check_invariants().unwrap();
}

#[test]
fn credit_draws_only_at_zero_and_never_finances_another_loan_payment() {
    let (mut s, _) = house();
    let id = offer(&mut s, 2, 3, true, 0);
    exec(&mut s, 3, Command::AcceptLoan { loan: id }).unwrap();
    assert_eq!(s.tick().unwrap(), 0);
    transfer(&mut s, 3, 1, 10000);
    assert_eq!(s.tick().unwrap(), 1);
    assert_eq!(balance(&s, 3), 10000);
    let other = offer(&mut s, 1, 3, true, 0);
    exec(&mut s, 3, Command::AcceptLoan { loan: other }).unwrap();
    exec(
        &mut s,
        3,
        Command::RepayLoan {
            loan: id,
            amount: 10000,
        },
    )
    .unwrap();
    assert_eq!(balance(&s, 3), 0);
    assert_eq!(s.tick().unwrap(), 0);
    assert_eq!(s.state().loan(other).unwrap().status, LoanStatus::Armed);
    transfer(&mut s, 2, 3, 1);
    transfer(&mut s, 3, 2, 1);
    assert_eq!(s.tick().unwrap(), 1);
    assert_eq!(s.state().loan(other).unwrap().status, LoanStatus::Active);
    s.state().check_invariants().unwrap();
}

#[test]
fn failed_or_ambiguous_automatic_payment_recovers_exactly_once() {
    for failure in [1, 2] {
        let (mut s, m) = house();
        let id = offer(&mut s, 2, 3, false, 100);
        exec(&mut s, 3, Command::AcceptLoan { loan: id }).unwrap();
        time(START + DAY);
        let before = balance(&s, 3);
        m.fail.set(failure);
        assert_eq!(s.tick(), Err(Error::Storage));
        assert_eq!(balance(&s, 3), before);
        drop(s);
        m.fail.set(0);
        let mut s = Service::open_with_clock(m, now).unwrap();
        assert_eq!(s.tick().unwrap(), if failure == 1 { 1 } else { 0 });
        assert_eq!(balance(&s, 3), before - 2100);
        assert_eq!(s.tick().unwrap(), 0);
        s.state().check_invariants().unwrap();
    }
}

#[test]
fn exact_decimal_reform_preserves_contracts_and_rejects_rounding_and_stale_previews() {
    let (mut s, m) = house();
    let id = offer(&mut s, 2, 3, false, 100);
    exec(&mut s, 3, Command::AcceptLoan { loan: id }).unwrap();
    let sequence = s.state().sequence;
    let reform = Command::ReformCurrency {
        decimals: 6,
        power: 0,
        expected_epoch: 0,
        expected_sequence: sequence,
    };
    assert_eq!(exec(&mut s, 2, reform.clone()), Err(Error::Forbidden));
    exec(&mut s, 1, reform.clone()).unwrap();
    assert_eq!(s.state().decimals, 6);
    assert_eq!(s.state().money_epoch, 1);
    assert_eq!(balance(&s, 3), 2_000_000);
    assert_eq!(s.state().loan(id).unwrap().terms.installment, 200_000);
    assert_eq!(exec(&mut s, 1, reform), Err(Error::Conflict));
    transfer(&mut s, 3, 2, 1);
    let sequence = s.state().sequence;
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::ReformCurrency {
                decimals: 4,
                power: 0,
                expected_epoch: 1,
                expected_sequence: sequence
            }
        ),
        Err(Error::Conflict)
    );
    assert_eq!(s.state().sequence, sequence);
    drop(s);
    let s = Service::open_with_clock(m, now).unwrap();
    assert_eq!(s.state().money_epoch, 1);
    assert_eq!(balance(&s, 3), 1_999_999);
    s.state().check_invariants().unwrap();
}

#[test]
fn checkpoint_restores_loan_schedule_and_currency_scale() {
    use nanacoin::journal::file::FileJournal;
    time(START);
    let path = std::env::temp_dir().join(format!(
        "nanacoin-loan-checkpoint-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    setup(&mut s);
    let id = offer(&mut s, 2, 3, false, 100);
    exec(&mut s, 3, Command::AcceptLoan { loan: id }).unwrap();
    let sequence = s.state().sequence;
    exec(
        &mut s,
        1,
        Command::ReformCurrency {
            decimals: 5,
            power: 1,
            expected_epoch: 0,
            expected_sequence: sequence,
        },
    )
    .unwrap();
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    time(START + DAY);
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    assert_eq!(s.state().decimals, 5);
    assert_eq!(s.state().money_epoch, 1);
    assert_eq!(s.tick().unwrap(), 1);
    assert_eq!(s.state().loan(id).unwrap().principal, 8000);
    s.state().check_invariants().unwrap();
}

#[test]
fn http_contract_privacy_nonnegative_rates_and_keyed_retries() {
    use nanacoin::api;
    use serde_json::{json, Value};
    fn call(
        s: &mut Service<Memory>,
        method: &str,
        path: &str,
        auth: &str,
        key: &str,
        body: Value,
    ) -> (u16, Value) {
        let mut output = vec![0; api::RESPONSE_LIMIT];
        let (status, n) = api::handle_keyed(
            s,
            method,
            path,
            auth,
            key,
            &serde_json::to_vec(&body).unwrap(),
            &mut output,
        );
        (status, serde_json::from_slice(&output[..n]).unwrap())
    }
    let (mut s, m) = house();
    let alice = common::login_as(&mut s, "Alice", "1234");
    let bob = common::login_as(&mut s, "Bob", "1234");
    let nana = common::login(&mut s);
    let terms = json!({"borrower":"account-3","amount":10000,"rate_bps":12000,"rate_days":365,"payment_days":7,"installment":2000,"credit":false,"memo":"Private loan terms"});
    let mut bad = terms.clone();
    bad["rate_bps"] = json!(-1);
    assert_eq!(
        call(&mut s, "POST", "/api/v1/loans", &alice, "g0:m0:bad", bad).0,
        400
    );
    let (status, loan) = call(
        &mut s,
        "POST",
        "/api/v1/loans",
        &alice,
        "g0:m0:offer",
        terms.clone(),
    );
    assert_eq!(status, 200, "{loan}");
    let path = format!("/api/v1/loans/{}/accept", loan["id"]);
    let (status, accepted) = call(&mut s, "POST", &path, &bob, "g0:m0:accept", json!({}));
    assert_eq!(status, 200, "{accepted}");
    assert_eq!(accepted["principal"], 10000);
    assert_eq!(
        call(&mut s, "POST", &path, &bob, "g0:m0:accept", json!({})).0,
        200
    );
    assert_eq!(balance(&s, 3), 20000);
    let (_, book) = call(&mut s, "GET", "/api/v1/loans", &nana, "", json!({}));
    assert_eq!(book["summary"]["weighted_annual_percent"], 120.0);
    assert_eq!(book["summary"]["outstanding"], "10000");
    drop(s);
    let mut s = Service::open_with_clock(m, now).unwrap();
    let alice = common::login_as(&mut s, "Alice", "1234");
    let (_, retry) = call(
        &mut s,
        "POST",
        "/api/v1/loans",
        &alice,
        "g0:m0:offer",
        terms,
    );
    assert_eq!(retry["id"], loan["id"]);
    assert_eq!(s.state().loans.len(), 1);
}
