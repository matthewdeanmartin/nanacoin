mod common;
use nanacoin::{auth::PasswordVerifier, domain::*, journal::*};
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

use nanacoin::fulfillment::{Action, Kind, Status, CAPACITY};
fn purchase<J: Journal>(s: &mut Service<J>, side: Side, kind: &str) -> u64 {
    let listing = exec(
        s,
        2,
        Command::List {
            title: Title::try_from("Wash car").unwrap(),
            description: Memo::new(),
            price: 1,
            side,
            details: Some(ListingDetails {
                kind: heapless::String::try_from(kind).unwrap(),
                currency: heapless::String::try_from("USD").unwrap(),
                minor_units: 100,
            }),
        },
    )
    .unwrap()
    .sequence;
    exec(s, 3, Command::Buy { listing }).unwrap().sequence
}
fn change(transaction: u64, action: Action) -> Command {
    Command::SetFulfillment {
        transaction,
        action,
        reason: Memo::try_from("Not delivered yet").unwrap(),
    }
}
#[test]
fn directions_permissions_disputes_and_retries_do_not_move_money() {
    for side in [Side::Sell, Side::Buy] {
        let (mut s, m) = house();
        let tx = purchase(&mut s, side, "service");
        let (provider, recipient) = if side == Side::Sell { (2, 3) } else { (3, 2) };
        let f = &s.state().fulfillments[0];
        assert_eq!(
            (f.provider, f.recipient, f.kind, f.status),
            (
                MemberId(provider),
                MemberId(recipient),
                Kind::Work,
                Status::Todo
            )
        );
        assert_eq!(f.description.as_str(), "Wash car");
        let before = (balance(&s, 2), balance(&s, 3), s.state().transactions);
        assert_eq!(
            exec(&mut s, 1, change(tx, Action::Complete)),
            Err(Error::Forbidden)
        );
        s.execute_keyed(MemberId(provider), "complete", change(tx, Action::Complete))
            .unwrap();
        assert_eq!(
            exec(&mut s, provider, change(tx, Action::Dispute)),
            Err(Error::Forbidden)
        );
        exec(&mut s, recipient, change(tx, Action::Dispute)).unwrap();
        assert_eq!(
            exec(&mut s, provider, change(tx, Action::Complete)),
            Err(Error::Conflict)
        );
        s.execute_keyed(MemberId(provider), "complete", change(tx, Action::Complete))
            .unwrap();
        assert_eq!(s.state().fulfillments[0].status, Status::Disputed);
        exec(&mut s, recipient, change(tx, Action::WithdrawDispute)).unwrap();
        assert_eq!(s.state().fulfillments[0].status, Status::Done);
        assert_eq!(
            before,
            (balance(&s, 2), balance(&s, 3), s.state().transactions)
        );
        let s = Service::open_with_clock(m, now).unwrap();
        assert_eq!(s.state().fulfillments[0].status, Status::Done);
        s.state().check_invariants().unwrap();
    }
}
#[test]
fn cash_dispute_before_completion_and_reversal() {
    let (mut s, _) = house();
    let tx = purchase(&mut s, Side::Sell, "currency");
    assert_eq!(s.state().fulfillments[0].kind, Kind::Cash);
    exec(&mut s, 3, change(tx, Action::Dispute)).unwrap();
    exec(&mut s, 3, change(tx, Action::WithdrawDispute)).unwrap();
    exec(
        &mut s,
        1,
        Command::Reverse {
            transaction: tx,
            memo: Memo::new(),
        },
    )
    .unwrap();
    assert_eq!(s.state().fulfillments[0].status, Status::Reversed);
    assert_eq!(
        exec(&mut s, 3, change(tx, Action::Dispute)),
        Err(Error::Conflict)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::Reverse {
                transaction: tx,
                memo: Memo::new()
            }
        ),
        Err(Error::Conflict)
    );
}
#[test]
fn ambiguous_and_failed_updates_recover_once() {
    for failure in [1, 2] {
        let (mut s, m) = house();
        let tx = purchase(&mut s, Side::Sell, "service");
        m.fail.set(failure);
        assert_eq!(
            s.execute_keyed(MemberId(2), "complete", change(tx, Action::Complete)),
            Err(Error::Storage)
        );
        assert_eq!(s.state().fulfillments[0].status, Status::Todo);
        drop(s);
        m.fail.set(0);
        let mut s = Service::open_with_clock(m, now).unwrap();
        s.execute_keyed(MemberId(2), "complete", change(tx, Action::Complete))
            .unwrap();
        assert_eq!(s.state().fulfillments[0].updates.len(), 2);
    }
}
#[test]
fn pending_work_and_refunds_survive_history_eviction_checkpoint_and_reform() {
    use nanacoin::journal::file::FileJournal;
    let path = std::env::temp_dir().join(format!(
        "nanacoin-fulfillment-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    setup(&mut s);
    let tx = purchase(&mut s, Side::Sell, "service");
    for _ in 0..HISTORY {
        exec(
            &mut s,
            2,
            Command::Transfer {
                to: MemberId(3),
                amount: 0,
                memo: Memo::try_from("hello").unwrap(),
            },
        )
        .unwrap();
    }
    assert!(!s.state().history.iter().any(|t| t.id == tx));
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
    exec(&mut s, 3, change(tx, Action::Dispute)).unwrap();
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    assert_eq!(s.state().fulfillments[0].status, Status::Disputed);
    let before = balance(&s, 3);
    exec(
        &mut s,
        1,
        Command::Reverse {
            transaction: tx,
            memo: Memo::new(),
        },
    )
    .unwrap();
    assert_eq!(balance(&s, 3), before + 10);
    assert_eq!(s.state().fulfillments[0].status, Status::Reversed);
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::Reverse {
                transaction: tx,
                memo: Memo::new()
            }
        ),
        Err(Error::Conflict)
    );
    s.reset_economy(MemberId(1)).unwrap();
    assert!(s.state().fulfillments.is_empty());
}
#[test]
fn capacity_never_evicts_unfinished_work_or_moves_payment() {
    let (mut s, _) = house();
    for _ in 0..CAPACITY {
        exec(
            &mut s,
            3,
            Command::ClassifiedTransfer {
                to: MemberId(2),
                amount: 1,
                memo: Memo::new(),
                economic: EconomicDetails {
                    kind: EconomicKind::Good,
                    quantity_milli: 1000,
                    ..EconomicDetails::default()
                },
            },
        )
        .unwrap();
    }
    let before = balance(&s, 3);
    assert_eq!(
        exec(
            &mut s,
            3,
            Command::ClassifiedTransfer {
                to: MemberId(2),
                amount: 1,
                memo: Memo::new(),
                economic: EconomicDetails {
                    kind: EconomicKind::Good,
                    quantity_milli: 1000,
                    ..EconomicDetails::default()
                }
            }
        ),
        Err(Error::Capacity)
    );
    assert_eq!(balance(&s, 3), before);
}

#[test]
fn http_status_account_scope_and_durable_keys() {
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
        let mut out = vec![0; api::RESPONSE_LIMIT];
        let (status, len) = api::handle_keyed(
            s,
            method,
            path,
            auth,
            key,
            &serde_json::to_vec(&body).unwrap(),
            &mut out,
        );
        (status, serde_json::from_slice(&out[..len]).unwrap())
    }
    let (mut s, m) = house();
    let tx = purchase(&mut s, Side::Sell, "service");
    let alice = common::login_as(&mut s, "Alice", "1234");
    let bob = common::login_as(&mut s, "Bob", "1234");
    let nana = common::login(&mut s);
    let endpoint = format!("/api/v1/transactions/tx-{tx}/fulfillment");
    assert_eq!(
        call(
            &mut s,
            "POST",
            &endpoint,
            &nana,
            "bad",
            json!({"action":"COMPLETE"})
        )
        .0,
        403
    );
    let (code, book) = call(&mut s, "GET", "/api/v1/fulfillments", &alice, "", json!({}));
    assert_eq!(code, 200);
    assert_eq!(book["fulfillments"][0]["status"], "TODO");
    let body = json!({"action":"COMPLETE","reason":""});
    assert_eq!(
        call(&mut s, "POST", &endpoint, &alice, "done", body.clone()).0,
        200
    );
    assert_eq!(
        call(
            &mut s,
            "POST",
            &endpoint,
            &bob,
            "dispute",
            json!({"action":"DISPUTE","reason":"Car dirty"})
        )
        .0,
        200
    );
    let (_, ledger) = call(&mut s, "GET", "/api/v1/transactions", &bob, "", json!({}));
    assert_eq!(
        ledger["transactions"][0]["fulfillment"]["status"],
        "DISPUTED"
    );
    drop(s);
    let mut s = Service::open_with_clock(m, now).unwrap();
    let alice = common::login_as(&mut s, "Alice", "1234");
    let (code, f) = call(&mut s, "POST", &endpoint, &alice, "done", body);
    assert_eq!(code, 200);
    assert_eq!(f["status"], "DISPUTED");
    assert_eq!(f["updates"].as_array().unwrap().len(), 3);
}

#[test]
fn recipient_can_record_delivery_without_moving_money() {
    let (mut s, m) = house();
    let tx = purchase(&mut s, Side::Sell, "service");
    let before = (balance(&s, 2), balance(&s, 3), s.state().transactions);
    exec(&mut s, 3, change(tx, Action::Complete)).unwrap();
    assert_eq!(s.state().fulfillments[0].status, Status::Done);
    assert_eq!(
        before,
        (balance(&s, 2), balance(&s, 3), s.state().transactions)
    );
    let s = Service::open_with_clock(m, now).unwrap();
    assert_eq!(
        s.state().fulfillments[0].updates.last().unwrap().actor,
        MemberId(3)
    );
}
