mod common;
use nanacoin::{auth::PasswordVerifier, domain::*, journal::*, lotto::*};
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
fn create<J: Journal>(s: &mut Service<J>, kind: LottoKind, rate: u32) -> u64 {
    exec(
        s,
        1,
        Command::CreateLotto {
            terms: LottoTerms {
                kind,
                title: Title::try_from("Family draw").unwrap(),
                ticket_price: 100,
                closes_at: START + 100,
                rate_bps: rate,
            },
        },
    )
    .unwrap()
    .sequence
}
fn buy<J: Journal>(s: &mut Service<J>, id: u64, who: u8, count: u32) {
    exec(s, who, Command::BuyTickets { lotto: id, count }).unwrap();
}
fn finish<J: Journal>(s: &mut Service<J>, id: u64) {
    for _ in 0..30 {
        if s.state().lotto(id).unwrap().step == 19 {
            break;
        }
        s.tick().unwrap();
        s.state().check_invariants().unwrap();
    }
    assert_eq!(s.state().lotto(id).unwrap().step, 19);
    assert_eq!(s.tick().unwrap(), 0);
}
#[test]
fn three_kinds_hold_principal_and_settle_at_exact_deadline() {
    for kind in [LottoKind::Simple, LottoKind::Delayed, LottoKind::Savings] {
        let (mut s, m) = house();
        let id = create(
            &mut s,
            kind,
            if kind == LottoKind::Simple { 0 } else { 1000 },
        );
        buy(&mut s, id, 2, 10);
        buy(&mut s, id, 3, 20);
        assert_eq!(s.state().lotto_escrow, 3000);
        assert_eq!(balance(&s, 2), 9000);
        let due = s.state().lotto(id).unwrap().due_at();
        time(due - 1);
        assert_eq!(s.tick().unwrap(), 0);
        time(due);
        assert_eq!(s.tick().unwrap(), 1);
        let winner = s.state().lotto(id).unwrap().winner.unwrap().0;
        // Restart at every step: the selected winner and paid legs must survive.
        for _ in 0..18 {
            drop(s);
            s = Service::open_with_clock(m.clone(), now).unwrap();
            assert_eq!(s.state().lotto(id).unwrap().winner.unwrap().0, winner);
            s.tick().unwrap();
            s.state().check_invariants().unwrap();
        }
        finish(&mut s, id);
        let interest = if kind == LottoKind::Simple { 0 } else { 300 };
        for who in [2, 3] {
            let base = if kind == LottoKind::Savings {
                10000
            } else if who == 2 {
                9000
            } else {
                8000
            };
            let prize = if who == winner {
                interest + if kind == LottoKind::Savings { 0 } else { 3000 }
            } else {
                0
            };
            assert_eq!(balance(&s, who), base + prize);
        }
        assert_eq!(s.state().lotto_escrow, 0);
        let tx = s
            .state()
            .history
            .iter()
            .find(|t| t.lotto == Some(id))
            .unwrap()
            .id;
        assert_eq!(
            exec(
                &mut s,
                1,
                Command::Reverse {
                    transaction: tx,
                    memo: Memo::new()
                }
            ),
            Err(Error::Forbidden)
        );
    }
}
#[test]
fn house_pays_first_and_only_shortfall_is_issued() {
    for cash in [0, 50, 1000] {
        let (mut s, _) = house();
        let house_balance = balance(&s, 1);
        if house_balance > cash {
            exec(
                &mut s,
                1,
                Command::Transfer {
                    to: MemberId(2),
                    amount: house_balance - cash,
                    memo: Memo::new(),
                },
            )
            .unwrap();
        } else if house_balance < cash {
            exec(
                &mut s,
                1,
                Command::Issue {
                    to: MemberId(1),
                    amount: cash - house_balance,
                    memo: Memo::new(),
                },
            )
            .unwrap();
        }
        let id = create(&mut s, LottoKind::Savings, 1000);
        buy(&mut s, id, 2, 10);
        let supply = s.state().issuance_balance;
        time(START + 100 + MONTH);
        finish(&mut s, id);
        assert_eq!(balance(&s, 1), cash - 100.min(cash));
        assert_eq!(s.state().issuance_balance, supply - (100 - cash).max(0));
        let paid: i64 = s
            .state()
            .history
            .iter()
            .filter(|t| t.lotto == Some(id) && t.economic.kind == EconomicKind::Interest)
            .map(|t| t.amount)
            .sum();
        assert_eq!(paid, 100);
    }
}
#[test]
fn purchases_are_keyed_and_deadlines_permissions_and_random_ranges_are_enforced() {
    let (mut s, m) = house();
    let id = create(&mut s, LottoKind::Simple, 0);
    let command = Command::BuyTickets {
        lotto: id,
        count: 3,
    };
    s.execute_keyed(MemberId(2), "g0:m0:ticket", command.clone())
        .unwrap();
    s.execute_keyed(MemberId(2), "g0:m0:ticket", command.clone())
        .unwrap();
    assert_eq!(s.state().lotto(id).unwrap().total_tickets(), 3);
    drop(s);
    let mut s = Service::open_with_clock(m, now).unwrap();
    s.execute_keyed(MemberId(2), "g0:m0:ticket", command)
        .unwrap();
    buy(&mut s, id, 3, 2);
    let l = s.state().lotto(id).unwrap();
    assert_eq!(
        (0..5)
            .map(|t| l.winner_for(t).unwrap().0)
            .collect::<Vec<_>>(),
        vec![2, 2, 2, 3, 3]
    );
    assert_eq!(l.winner_for(5), None);
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::BuyTickets {
                lotto: id,
                count: 1
            }
        ),
        Err(Error::Forbidden)
    );
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::BuyTickets {
                lotto: id,
                count: 0
            }
        ),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::BuyTickets {
                lotto: id,
                count: 1000
            }
        ),
        Err(Error::InsufficientFunds)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::RunLotto {
                lotto: id,
                step: 0,
                ticket: 0
            }
        ),
        Err(Error::Forbidden)
    );
    time(START + 100);
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::BuyTickets {
                lotto: id,
                count: 1
            }
        ),
        Err(Error::Conflict)
    );
}
#[test]
fn ambiguous_draw_and_payment_never_redraw_or_double_pay() {
    for step in [0, 2, 17, 18] {
        for failure in [1, 2] {
            let (mut s, m) = house();
            let id = create(&mut s, LottoKind::Savings, 1000);
            buy(&mut s, id, 2, 10);
            time(START + 100 + MONTH);
            while s.state().lotto(id).unwrap().step < step {
                s.tick().unwrap();
            }
            let old_balance = balance(&s, 2);
            let old_step = s.state().lotto(id).unwrap().step;
            m.fail.set(failure);
            assert_eq!(s.tick(), Err(Error::Storage));
            assert_eq!(balance(&s, 2), old_balance);
            assert_eq!(s.state().lotto(id).unwrap().step, old_step);
            drop(s);
            m.fail.set(0);
            let mut s = Service::open_with_clock(m, now).unwrap();
            finish(&mut s, id);
            assert_eq!(balance(&s, 2), 10100);
        }
    }
}
#[test]
fn empty_draws_finish_and_late_ticks_do_not_increase_interest() {
    let (mut s, _) = house();
    let empty = create(&mut s, LottoKind::Savings, 100);
    let id = create(&mut s, LottoKind::Delayed, 100);
    buy(&mut s, id, 2, 1);
    time(START + 100 + MONTH * 12);
    finish(&mut s, id);
    assert_eq!(s.state().lotto(empty).unwrap().step, 19);
    assert_eq!(balance(&s, 2), 10001);
}
#[test]
fn checkpoint_keeps_escrow_draw_and_partial_settlement_and_reform() {
    use nanacoin::journal::file::FileJournal;
    time(START);
    let path = std::env::temp_dir().join(format!(
        "nanacoin-lotto-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    setup(&mut s);
    let id = create(&mut s, LottoKind::Savings, 1000);
    buy(&mut s, id, 2, 10);
    buy(&mut s, id, 3, 20);
    let sequence = s.state().sequence;
    exec(
        &mut s,
        1,
        Command::ReformCurrency {
            decimals: 5,
            power: 0,
            expected_epoch: 0,
            expected_sequence: sequence,
        },
    )
    .unwrap();
    time(START + 100 + MONTH);
    for _ in 0..3 {
        s.tick().unwrap();
    }
    let winner = s.state().lotto(id).unwrap().winner;
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    assert_eq!(s.state().lotto(id).unwrap().winner, winner);
    finish(&mut s, id);
    for who in [2, 3] {
        assert_eq!(
            balance(&s, who),
            100000
                + if winner == Some(MemberId(who)) {
                    3000
                } else {
                    0
                }
        );
    }
}

#[test]
fn http_contract_uses_accounts_statuses_and_durable_keys() {
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
    let (mut s, _) = house();
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "Alice", "1234");
    let terms = json!({"kind":"SAVINGS","title":"Family draw","ticket_price":100,"closes_at":START+100,"rate_bps":1000});
    assert_eq!(
        call(
            &mut s,
            "POST",
            "/api/v1/lottos",
            &alice,
            "g0:m0:forbidden",
            terms.clone()
        )
        .0,
        403
    );
    let (status, draw) = call(
        &mut s,
        "POST",
        "/api/v1/lottos",
        &nana,
        "g0:m0:create",
        terms.clone(),
    );
    assert_eq!(status, 200, "{draw}");
    assert_eq!(draw["house"], "account-1");
    assert_eq!(draw["status"], "OPEN");
    assert_eq!(draw["due_at"], START + 100 + MONTH);
    assert_eq!(
        call(
            &mut s,
            "POST",
            "/api/v1/lottos",
            &nana,
            "g0:m0:create",
            terms
        )
        .1["id"],
        draw["id"]
    );
    let path = format!("/api/v1/lottos/{}/tickets", draw["id"]);
    for _ in 0..2 {
        let (status, bought) = call(
            &mut s,
            "POST",
            &path,
            &alice,
            "g0:m0:buy",
            json!({"count":3}),
        );
        assert_eq!(status, 200);
        assert_eq!(bought["my_tickets"], 3);
    }
    assert_eq!(balance(&s, 2), 9700);
    let (_, book) = call(&mut s, "GET", "/api/v1/lottos", &alice, "", json!({}));
    assert_eq!(book["lottos"][0]["interest"], 30);
    time(START + 100);
    assert_eq!(
        call(&mut s, "GET", "/api/v1/lottos", &alice, "", json!({})).1["lottos"][0]["status"],
        "WAITING"
    );
    time(START + 100 + MONTH);
    finish(&mut s, draw["id"].as_u64().unwrap());
    let (_, book) = call(&mut s, "GET", "/api/v1/lottos", &alice, "", json!({}));
    assert_eq!(book["lottos"][0]["winner"], "account-2");
    assert_eq!(book["lottos"][0]["status"], "SETTLED");
}
