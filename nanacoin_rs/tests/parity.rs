mod common;
use nanacoin::{api, auth::PasswordVerifier, domain::*, forex::*, journal::*};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

thread_local! { static NOW: Cell<u64> = const { Cell::new(1_800_000_000) }; }
fn now() -> u64 {
    NOW.with(Cell::get)
}
#[derive(Clone, Default)]
struct Memory {
    frames: Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>,
    failure: Rc<Cell<u8>>,
}
impl Journal for Memory {
    fn read(&mut self, i: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(data) = self.frames.borrow().get(i) {
            *frame = *data;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, _: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        let failure = self.failure.get();
        if failure != 1 {
            self.frames.borrow_mut().push(*frame);
        }
        if failure != 0 {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}
fn exec(s: &mut Service<Memory>, actor: u8, cmd: Command) -> Result<Receipt, Error> {
    s.execute(
        MemberId(actor),
        s.state().member(MemberId(actor)).unwrap().last_request + 1,
        cmd,
    )
}
fn house() -> (Service<Memory>, Memory) {
    NOW.with(|n| n.set(1_800_000_000));
    let m = Memory::default();
    let mut s = Service::open_with_clock(m.clone(), now).unwrap();
    common::provision(&mut s);
    for name in ["alice", "bob"] {
        exec(
            &mut s,
            1,
            Command::CreateMember {
                username: Name::try_from(name).unwrap(),
                display_name: Name::try_from(name).unwrap(),
                password: PasswordVerifier::hash("1234").unwrap(),
                role: Role::User,
                grant: 100,
            },
        )
        .unwrap();
    }
    for to in [MemberId(2), MemberId(3)] {
        exec(
            &mut s,
            1,
            Command::IssueUsd {
                to,
                cents: 1000,
                memo: Memo::new(),
            },
        )
        .unwrap();
    }
    (s, m)
}
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
fn quote(s: &mut Service<Memory>, side: QuoteSide, expires_at: u64) -> u64 {
    exec(
        s,
        2,
        Command::PostQuote {
            side,
            cents_per_coin: 25,
            coins: 10,
            expires_at,
        },
    )
    .unwrap()
    .sequence
}

#[test]
fn balance_and_ledger_privacy_matches_go_including_rust_state_endpoint() {
    let (mut s, _) = house();
    let alice = common::login_as(&mut s, "alice", "1234");
    let nana = common::login(&mut s);
    let users = call(&mut s, "GET", "/api/v1/users", &alice, "", json!({})).1;
    assert!(users["users"][0].get("balance").is_none());
    assert!(users["users"][2].get("usd_cents").is_none());
    assert_eq!(users["users"][1]["balance"], 100);
    for path in [
        "/api/v1/state",
        "/api/v1/transactions",
        "/api/v1/accounts/account-3",
        "/api/v1/accounts/account-3/transactions",
        "/api/v1/accounts/account-3-usd/transactions",
        "/api/v1/transactions/tx-5",
    ] {
        assert_eq!(
            call(&mut s, "GET", path, &alice, "", json!({})).0,
            403,
            "{path}"
        );
        assert_eq!(
            call(&mut s, "GET", path, &nana, "", json!({})).0,
            200,
            "{path}"
        );
    }
    assert_eq!(
        call(
            &mut s,
            "GET",
            "/api/v1/accounts/account-2-usd",
            &alice,
            "",
            json!({})
        )
        .1["balance"],
        1000
    );
    assert_eq!(
        call(
            &mut s,
            "GET",
            "/api/v1/transactions/tx-2",
            &alice,
            "",
            json!({})
        )
        .0,
        200
    );
}

#[test]
fn listing_metadata_timestamps_and_item_reads_survive_replay() {
    let (mut s, m) = house();
    let nana = common::login(&mut s);
    let created = call(
        &mut s,
        "POST",
        "/api/v1/listings",
        &nana,
        "",
        json!({"title":"Five dollars", "description":"Cash", "price":20, "side":"SELL", "kind":"currency", "currency":"USD", "minor_units":500}),
    );
    assert_eq!(created.0, 201, "{created:?}");
    assert_eq!(created.1["created_at"], now());
    let path = format!("/api/v1/listings/{}", created.1["id"].as_str().unwrap());
    NOW.with(|n| n.set(now() + 1));
    let updated = call(
        &mut s,
        "PATCH",
        &path,
        &nana,
        "",
        json!({"title":"Dollar notes"}),
    );
    assert_eq!(updated.1["updated_at"], now());
    let mut s = Service::open_with_clock(m, now).unwrap();
    let nana = common::login(&mut s);
    let read = call(&mut s, "GET", &path, &nana, "", json!({}));
    assert_eq!(read, updated);
    assert_eq!(read.1["kind"], "currency");
    assert_eq!(read.1["minor_units"], 500);
    assert_eq!(s.state().members[0].created_at, 1_800_000_000);
    assert_eq!(
        call(
            &mut s,
            "POST",
            "/api/v1/listings",
            &nana,
            "",
            json!({"title":"Bad", "description":"", "price":20,"kind":"currency"})
        )
        .0,
        400
    );
}

#[test]
fn forex_bid_ask_contract_balances_and_durable_retries() {
    for side in [QuoteSide::ASK, QuoteSide::BID] {
        let (mut s, m) = house();
        let alice = common::login_as(&mut s, "alice", "1234");
        let bob = common::login_as(&mut s, "bob", "1234");
        let posted = call(
            &mut s,
            "POST",
            "/api/v1/quotes",
            &alice,
            "",
            json!({"side":side,"cents_per_coin":25,"coins":10}),
        );
        assert_eq!(posted.0, 201, "{posted:?}");
        let path = format!("/api/v1/quotes/{}/take", posted.1["id"].as_str().unwrap());
        let taken = call(&mut s, "POST", &path, &bob, "take-one", json!({}));
        assert_eq!(taken.0, 201, "{taken:?}");
        assert_eq!(taken.1["quote"]["status"], "FILLED");
        assert_eq!(
            taken.1["cash_transaction"]["postings"][0]["account"],
            if side == QuoteSide::ASK {
                "account-3-usd"
            } else {
                "account-2-usd"
            }
        );
        assert_eq!(taken.1["coin_transaction"]["reference"], posted.1["id"]);
        let a = &s.state().members[1];
        assert_eq!(
            (a.balance, a.usd_cents),
            if side == QuoteSide::ASK {
                (90, 1250)
            } else {
                (110, 750)
            }
        );
        s.state().check_invariants().unwrap();
        let mut s = Service::open_with_clock(m, now).unwrap();
        let bob = common::login_as(&mut s, "bob", "1234");
        assert_eq!(
            call(&mut s, "POST", &path, &bob, "take-one", json!({})),
            taken
        );
        assert_eq!(
            call(&mut s, "POST", &path, &bob, "take-two", json!({})).0,
            409
        );
        let coin_id = s.state().quotes[0].coin_tx.unwrap();
        let cash_id = s.state().quotes[0].cash_tx.unwrap();
        exec(
            &mut s,
            1,
            Command::Reverse {
                transaction: coin_id,
                memo: Memo::new(),
            },
        )
        .unwrap();
        exec(
            &mut s,
            1,
            Command::Reverse {
                transaction: cash_id,
                memo: Memo::new(),
            },
        )
        .unwrap();
        assert_eq!(
            (s.state().members[1].balance, s.state().members[1].usd_cents),
            (100, 1000)
        );
        s.state().check_invariants().unwrap();
    }
}

#[test]
fn forex_failed_and_ambiguous_append_cannot_commit_only_one_leg() {
    for failure in [1, 2] {
        let (mut s, m) = house();
        let id = quote(&mut s, QuoteSide::ASK, 0);
        m.failure.set(failure);
        assert_eq!(
            s.execute_keyed(MemberId(3), "trade", Command::TakeQuote { quote: id }),
            Err(Error::Storage)
        );
        assert_eq!(
            (s.state().members[1].balance, s.state().members[1].usd_cents),
            (100, 1000)
        );
        assert!(s.storage_failed());
        m.failure.set(0);
        let mut s = Service::open_with_clock(m, now).unwrap();
        assert_eq!(
            (s.state().members[1].balance, s.state().members[1].usd_cents),
            if failure == 1 {
                (100, 1000)
            } else {
                (90, 1250)
            }
        );
        let receipt = s
            .execute_keyed(MemberId(3), "trade", Command::TakeQuote { quote: id })
            .unwrap();
        assert_eq!(receipt.replayed, failure == 2);
        assert_eq!(
            (s.state().members[1].balance, s.state().members[1].usd_cents),
            (90, 1250)
        );
        s.state().check_invariants().unwrap();
    }
}

#[test]
fn quote_permissions_limits_funds_deadlines_and_recycling() {
    let (mut s, _) = house();
    let id = quote(&mut s, QuoteSide::ASK, now() + 10);
    assert_eq!(
        exec(&mut s, 2, Command::TakeQuote { quote: id }),
        Err(Error::SelfDeal)
    );
    assert_eq!(
        exec(&mut s, 3, Command::CancelQuote { quote: id }),
        Err(Error::Forbidden)
    );
    let unaffordable = exec(
        &mut s,
        2,
        Command::PostQuote {
            side: QuoteSide::ASK,
            cents_per_coin: 1000,
            coins: 10,
            expires_at: 0,
        },
    )
    .unwrap()
    .sequence;
    assert_eq!(
        exec(
            &mut s,
            3,
            Command::TakeQuote {
                quote: unaffordable
            }
        ),
        Err(Error::InsufficientFunds)
    );
    for _ in 2..QUOTES {
        quote(&mut s, QuoteSide::ASK, 0);
    }
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::PostQuote {
                side: QuoteSide::ASK,
                cents_per_coin: 1,
                coins: 1,
                expires_at: 0
            }
        ),
        Err(Error::Capacity)
    );
    NOW.with(|n| n.set(now() + 10));
    assert_eq!(
        exec(&mut s, 3, Command::TakeQuote { quote: id }),
        Err(Error::Conflict)
    );
    quote(&mut s, QuoteSide::BID, 0);
    assert!(!s.state().quotes.iter().any(|q| q.id == id));
    exec(
        &mut s,
        1,
        Command::CancelQuote {
            quote: unaffordable,
        },
    )
    .unwrap();
    quote(&mut s, QuoteSide::BID, 0);
    NOW.with(|n| n.set(0));
    let id = s.state().quotes[0].id;
    assert_eq!(
        exec(&mut s, 3, Command::TakeQuote { quote: id }),
        Err(Error::Unavailable)
    );
}
