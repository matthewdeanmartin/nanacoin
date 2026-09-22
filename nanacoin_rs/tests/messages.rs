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
#[test]
fn zero_send_is_a_durable_message_not_a_payment() {
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
        let (code, len) = api::handle_keyed(
            s,
            method,
            path,
            auth,
            key,
            &serde_json::to_vec(&body).unwrap(),
            &mut output,
        );
        (code, serde_json::from_slice(&output[..len]).unwrap())
    }
    let (mut s, m) = house();
    let alice = common::login_as(&mut s, "Alice", "1234");
    let bob = common::login_as(&mut s, "Bob", "1234");
    let nana = common::login(&mut s);
    let before = [
        balance(&s, 1),
        balance(&s, 2),
        balance(&s, 3),
        s.state().issuance_balance,
    ];
    let count = s.state().transactions;
    let body = json!({"to":"account-3","amount":0,"memo":"Please return the ladder"});
    let (code, tx) = call(
        &mut s,
        "POST",
        "/api/v1/transfers",
        &alice,
        "g0:m0:letter",
        body.clone(),
    );
    assert_eq!(code, 201, "{tx}");
    assert_eq!(tx["kind"], "MESSAGE");
    assert_eq!(tx["postings"][0]["amount"], 0);
    assert_eq!(
        call(
            &mut s,
            "POST",
            "/api/v1/transfers",
            &alice,
            "g0:m0:letter",
            body.clone()
        )
        .1["id"],
        tx["id"]
    );
    assert_eq!(s.state().transactions, count + 1);
    assert_eq!(
        [
            balance(&s, 1),
            balance(&s, 2),
            balance(&s, 3),
            s.state().issuance_balance
        ],
        before
    );
    for (account, auth) in [("account-2", &alice), ("account-3", &bob)] {
        let (code, history) = call(
            &mut s,
            "GET",
            &format!("/api/v1/accounts/{account}/transactions"),
            auth,
            "",
            json!({}),
        );
        assert_eq!(code, 200);
        assert_eq!(history["transactions"][0]["kind"], "MESSAGE");
    }
    let (_, public) = call(&mut s, "GET", "/api/v1/public/ledger", "", "", json!({}));
    assert!(!public.to_string().contains("Please return"));
    assert_eq!(
        call(
            &mut s,
            "GET",
            &format!("/api/v1/transactions/{}", tx["id"].as_str().unwrap()),
            "",
            "",
            json!({})
        )
        .0,
        404
    );
    let (_, other) = call(
        &mut s,
        "GET",
        "/api/v1/accounts/account-2/transactions",
        &nana,
        "",
        json!({}),
    );
    assert!(!other.to_string().contains("Please return"));
    let sequence = s.state().sequence;
    assert_eq!(
        exec(
            &mut s,
            1,
            Command::Reverse {
                transaction: sequence,
                memo: Memo::try_from("undo").unwrap()
            }
        ),
        Err(Error::Forbidden)
    );
    drop(s);
    let mut s = Service::open_with_clock(m, now).unwrap();
    let alice = common::login_as(&mut s, "Alice", "1234");
    assert_eq!(
        call(
            &mut s,
            "POST",
            "/api/v1/transfers",
            &alice,
            "g0:m0:letter",
            body
        )
        .1["id"],
        tx["id"]
    );
    s.state().check_invariants().unwrap();
}
#[test]
fn empty_messages_negative_amounts_and_disabled_recipients_are_rejected() {
    let (mut s, _) = house();
    for (amount, memo) in [(0, " "), (-1, "hello")] {
        assert_eq!(
            exec(
                &mut s,
                2,
                Command::Transfer {
                    to: MemberId(3),
                    amount,
                    memo: Memo::try_from(memo).unwrap()
                }
            ),
            Err(Error::InvalidInput)
        );
    }
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::Transfer {
                to: MemberId(2),
                amount: 0,
                memo: Memo::try_from("hello").unwrap()
            }
        ),
        Err(Error::SelfDeal)
    );
    exec(
        &mut s,
        1,
        Command::UpdateMember {
            member: MemberId(3),
            display_name: None,
            password: None,
            role: None,
            disabled: Some(true),
            mastodon_id: None,
        },
    )
    .unwrap();
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::Transfer {
                to: MemberId(3),
                amount: 0,
                memo: Memo::try_from("hello").unwrap()
            }
        ),
        Err(Error::Disabled)
    );
}
#[test]
fn messages_survive_checkpoints_and_ambiguous_writes_once() {
    for failure in [1, 2] {
        let (mut s, m) = house();
        let command = Command::Transfer {
            to: MemberId(3),
            amount: 0,
            memo: Memo::try_from("A note").unwrap(),
        };
        let count = s.state().transactions;
        m.fail.set(failure);
        assert_eq!(
            s.execute_keyed(MemberId(2), "g0:m0:note", command.clone()),
            Err(Error::Storage)
        );
        assert_eq!(s.state().transactions, count);
        drop(s);
        m.fail.set(0);
        let mut s = Service::open_with_clock(m, now).unwrap();
        s.execute_keyed(MemberId(2), "g0:m0:note", command).unwrap();
        assert_eq!(s.state().transactions, count + 1);
    }
    use nanacoin::journal::file::FileJournal;
    let path = std::env::temp_dir().join(format!(
        "nanacoin-mail-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    setup(&mut s);
    exec(
        &mut s,
        2,
        Command::Transfer {
            to: MemberId(3),
            amount: 0,
            memo: Memo::try_from("A durable note").unwrap(),
        },
    )
    .unwrap();
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    let t = s.state().history.back().unwrap();
    assert_eq!(t.amount, 0);
    assert_eq!(t.memo.as_str(), "A durable note");
    s.state().check_invariants().unwrap();
}
#[test]
fn retained_offers_keep_direction_and_names_after_listing_recycling() {
    use nanacoin::{
        api,
        offers::{OfferId, OfferMessage},
    };
    use serde_json::Value;
    for side in [Side::Buy, Side::Sell] {
        let (mut s, m) = house();
        let listing = exec(
            &mut s,
            2,
            Command::List {
                title: Title::try_from("Cookies").unwrap(),
                description: Memo::new(),
                price: 25,
                side,
                details: None,
            },
        )
        .unwrap()
        .sequence;
        let offer = exec(
            &mut s,
            3,
            Command::MakeOffer {
                listing,
                amount: 20,
                message: OfferMessage::new(),
            },
        )
        .unwrap()
        .sequence;
        exec(&mut s, 2, Command::Cancel { listing }).unwrap();
        for _ in 0..LISTINGS {
            exec(
                &mut s,
                2,
                Command::List {
                    title: Title::try_from("New listing").unwrap(),
                    description: Memo::new(),
                    price: 25,
                    side: Side::Sell,
                    details: None,
                },
            )
            .unwrap();
        }
        drop(s);
        let mut s = Service::open_with_clock(m, now).unwrap();
        let saved = s.state().offer(OfferId(offer)).unwrap();
        assert_eq!(saved.listing_side, side);
        assert_eq!(saved.listing_title.as_str(), "Cookies");
        let auth = common::login_as(&mut s, "Alice", "1234");
        let mut output = vec![0; api::RESPONSE_LIMIT];
        let (code, len) = api::handle(&mut s, "GET", "/api/v1/offers", &auth, b"", &mut output);
        assert_eq!(code, 200);
        let response: Value = serde_json::from_slice(&output[..len]).unwrap();
        assert_eq!(response["offers"][0]["listing_owner_name"], "Alice");
        assert_eq!(response["offers"][0]["listing_title"], "Cookies");
        assert_eq!(response["offers"][0]["status"], "NOT_SELECTED");
        assert_eq!(
            response["offers"][0]["listing_side"],
            if side == Side::Buy { "BUY" } else { "SELL" }
        );
    }
}
