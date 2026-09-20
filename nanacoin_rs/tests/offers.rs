mod common;
use nanacoin::{api, auth::PasswordVerifier, domain::*, journal::*, offers::*};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const START: u64 = 1_700_000_000;
thread_local! { static NOW: Cell<u64> = const { Cell::new(START) }; }
fn now() -> u64 {
    NOW.with(Cell::get)
}
fn set_time(time: u64) {
    NOW.with(|n| n.set(time));
}

#[derive(Clone, Default)]
struct Memory {
    frames: Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>,
    fail: Rc<Cell<u8>>,
}
impl Journal for Memory {
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        match self.frames.borrow().get(index) {
            Some(f) => {
                *frame = *f;
                Ok(true)
            }
            None => Ok(false),
        }
    }
    fn append(&mut self, index: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        if self.fail.get() == 1 {
            return Err(Error::Storage);
        }
        assert_eq!(index, self.frames.borrow().len());
        self.frames.borrow_mut().push(*frame);
        if self.fail.get() == 2 {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}
type House = Service<Memory>;
fn exec(s: &mut House, actor: u8, command: Command) -> Result<Receipt, Error> {
    s.execute(
        MemberId(actor),
        s.state().member(MemberId(actor))?.last_request + 1,
        command,
    )
}
fn house() -> (House, Memory) {
    set_time(START);
    let memory = Memory::default();
    let mut s = Service::open_with_clock(memory.clone(), now).unwrap();
    common::provision(&mut s);
    for name in ["alice", "bob", "carol"] {
        exec(
            &mut s,
            1,
            Command::CreateMember {
                username: Name::try_from(name).unwrap(),
                display_name: Name::try_from(name).unwrap(),
                password: PasswordVerifier::hash("1234").unwrap(),
                role: Role::User,
                grant: 100,
                mastodon_id: MastodonId::new(),
            },
        )
        .unwrap();
    }
    (s, memory)
}
fn listing(s: &mut House, side: Side) -> u64 {
    exec(
        s,
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
    .sequence
}
fn offer(s: &mut House, listing: u64, amount: i64) -> OfferId {
    OfferId(
        exec(
            s,
            3,
            Command::MakeOffer {
                listing,
                amount,
                message: OfferMessage::try_from("Saturday 🍪").unwrap(),
            },
        )
        .unwrap()
        .sequence,
    )
}
fn undo(offer: OfferId) -> Command {
    Command::UnacceptOffer {
        offer,
        reason: TransactionMemo::try_from("Not delivered").unwrap(),
    }
}
fn balance(s: &House, member: u8) -> i64 {
    s.state().member(MemberId(member)).unwrap().balance
}
fn api(
    s: &mut House,
    method: &str,
    path: &str,
    auth: &str,
    key: &str,
    body: Value,
) -> (u16, Value) {
    let mut output = vec![0; api::RESPONSE_LIMIT];
    let (status, length) = api::handle_keyed(
        s,
        method,
        path,
        auth,
        key,
        &serde_json::to_vec(&body).unwrap(),
        &mut output,
    );
    (status, serde_json::from_slice(&output[..length]).unwrap())
}

#[test]
fn negotiated_price_and_both_payment_directions_match_tinygo() {
    for (side, alice, bob) in [(Side::Sell, 115, 85), (Side::Buy, 85, 115)] {
        let (mut s, memory) = house();
        let listing = listing(&mut s, side);
        let id = offer(&mut s, listing, 15);
        assert_eq!((balance(&s, 2), balance(&s, 3)), (100, 100));
        assert_eq!(s.state().listings[0].status, ListingStatus::Active);
        let result = exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
        assert_eq!((balance(&s, 2), balance(&s, 3)), (alice, bob));
        assert_eq!(s.state().listings[0].status, ListingStatus::Sold);
        let o = s.state().offer(id).unwrap();
        assert_eq!(o.settlement().unwrap().transaction, result.sequence);
        assert_eq!(
            o.settlement().unwrap().settles_at,
            START + DEFAULT_SETTLEMENT
        );
        let replay = Service::open_with_clock(memory, now).unwrap();
        assert_eq!((balance(&replay, 2), balance(&replay, 3)), (alice, bob));
        assert_eq!(replay.state().offer(id).unwrap().phase, o.phase);
        replay.state().check_invariants().unwrap();
    }
}

#[test]
fn funds_checked_only_on_acceptance_and_disabled_parties_cannot_trade() {
    let (mut s, memory) = house();
    let l = listing(&mut s, Side::Sell);
    let id = offer(&mut s, l, 500);
    let writes = memory.frames.borrow().len();
    assert_eq!(
        exec(&mut s, 2, Command::AcceptOffer { offer: id }),
        Err(Error::InsufficientFunds)
    );
    assert_eq!(memory.frames.borrow().len(), writes);
    assert_eq!(s.state().offer(id).unwrap().phase, OfferPhase::Open);
    exec(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(3),
            amount: 500,
            memo: Memo::new(),
        },
    )
    .unwrap();
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
        exec(&mut s, 2, Command::AcceptOffer { offer: id }),
        Err(Error::Disabled)
    );
    assert_eq!(
        exec(&mut s, 3, Command::WithdrawOffer { offer: id }),
        Err(Error::Disabled)
    );
}

#[test]
fn acceptance_is_owners_decision_and_rejects_self_deals_closed_listings_and_offers() {
    let (mut s, _) = house();
    let l = listing(&mut s, Side::Sell);
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::MakeOffer {
                listing: l,
                amount: 10,
                message: OfferMessage::new()
            }
        ),
        Err(Error::SelfDeal)
    );
    let id = offer(&mut s, l, 15);
    for actor in [1, 3, 4] {
        assert_eq!(
            exec(&mut s, actor, Command::AcceptOffer { offer: id }),
            Err(Error::Forbidden)
        );
    }
    exec(&mut s, 3, Command::WithdrawOffer { offer: id }).unwrap();
    assert_eq!(
        exec(&mut s, 2, Command::AcceptOffer { offer: id }),
        Err(Error::OfferClosed)
    );
    let id = offer(&mut s, l, 15);
    exec(&mut s, 2, Command::Cancel { listing: l }).unwrap();
    assert_eq!(
        exec(&mut s, 2, Command::AcceptOffer { offer: id }),
        Err(Error::ListingClosed)
    );
    assert_eq!(
        exec(
            &mut s,
            3,
            Command::MakeOffer {
                listing: l,
                amount: 10,
                message: OfferMessage::new()
            }
        ),
        Err(Error::ListingClosed)
    );
}

#[test]
fn decline_withdraw_permissions_and_nana_override() {
    let (mut s, _) = house();
    let l = listing(&mut s, Side::Sell);
    let first = offer(&mut s, l, 10);
    assert_eq!(
        exec(&mut s, 3, Command::DeclineOffer { offer: first }),
        Err(Error::Forbidden)
    );
    assert_eq!(
        exec(&mut s, 2, Command::WithdrawOffer { offer: first }),
        Err(Error::Forbidden)
    );
    exec(&mut s, 2, Command::DeclineOffer { offer: first }).unwrap();
    let second = offer(&mut s, l, 10);
    exec(&mut s, 3, Command::WithdrawOffer { offer: second }).unwrap();
    for decline in [true, false] {
        let id = offer(&mut s, l, 10);
        exec(
            &mut s,
            1,
            if decline {
                Command::DeclineOffer { offer: id }
            } else {
                Command::WithdrawOffer { offer: id }
            },
        )
        .unwrap();
    }
    assert_eq!(s.state().listings[0].status, ListingStatus::Active);
}

#[test]
fn either_party_or_nana_can_undo_once_and_reopen_in_one_event() {
    for actor in [1, 2, 3] {
        for side in [Side::Sell, Side::Buy] {
            let (mut s, memory) = house();
            let l = listing(&mut s, side);
            let id = offer(&mut s, l, 20);
            let accepted = exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
            assert_eq!(exec(&mut s, 4, undo(id)), Err(Error::Forbidden));
            let writes = memory.frames.borrow().len();
            set_time(START + DEFAULT_SETTLEMENT - 1);
            exec(&mut s, actor, undo(id)).unwrap();
            assert_eq!(memory.frames.borrow().len(), writes + 1);
            assert_eq!((balance(&s, 2), balance(&s, 3)), (100, 100));
            assert_eq!(s.state().listings[0].status, ListingStatus::Active);
            assert_eq!(
                s.state().history.back().unwrap().reverses,
                Some(accepted.sequence)
            );
            assert_eq!(exec(&mut s, actor, undo(id)), Err(Error::OfferClosed));
            let replay = Service::open_with_clock(memory, now).unwrap();
            assert_eq!(replay.state().listings[0].status, ListingStatus::Active);
            assert_eq!(replay.state().offer(id).unwrap().status(now()), "REVERSED");
        }
    }
}

#[test]
fn undo_allows_spent_funds_and_survives_history_eviction() {
    let (mut s, memory) = house();
    let l = listing(&mut s, Side::Sell);
    let id = offer(&mut s, l, 20);
    let accepted = exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
    exec(
        &mut s,
        2,
        Command::Transfer {
            to: MemberId(4),
            amount: 120,
            memo: Memo::new(),
        },
    )
    .unwrap();
    for _ in 0..HISTORY {
        exec(
            &mut s,
            1,
            Command::Issue {
                to: MemberId(4),
                amount: 1,
                memo: Memo::new(),
            },
        )
        .unwrap();
    }
    assert!(!s.state().history.iter().any(|t| t.id == accepted.sequence));
    exec(&mut s, 3, undo(id)).unwrap();
    assert_eq!(balance(&s, 2), -20);
    assert_eq!(balance(&s, 3), 100);
    assert_eq!(
        exec(
            &mut s,
            2,
            Command::Transfer {
                to: MemberId(4),
                amount: 1,
                memo: Memo::new()
            }
        ),
        Err(Error::InsufficientFunds)
    );
    let replay = Service::open_with_clock(memory, now).unwrap();
    assert_eq!(balance(&replay, 2), -20);
    replay.state().check_invariants().unwrap();
}

#[test]
fn deadline_is_exact_configurable_durable_and_not_changed_retroactively() {
    let (mut s, memory) = house();
    let nana = common::login(&mut s);
    let config = json!({"offer_settles_after":3600});
    assert_eq!(
        api(&mut s, "PATCH", "/api/v1/admin/config", &nana, "", config).0,
        200
    );
    let l = listing(&mut s, Side::Sell);
    let id = offer(&mut s, l, 20);
    exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
    assert_eq!(
        api(
            &mut s,
            "PATCH",
            "/api/v1/admin/config",
            &nana,
            "",
            json!({"offer_settles_after":0})
        )
        .0,
        200
    );
    assert_eq!(s.state().offer_settles_after, DEFAULT_SETTLEMENT);
    set_time(START + 3600);
    let mut replay = Service::open_with_clock(memory, now).unwrap();
    for actor in [1, 2, 3] {
        assert_eq!(exec(&mut replay, actor, undo(id)), Err(Error::OfferSettled));
    }
    assert_eq!(replay.state().offer(id).unwrap().status(now()), "SETTLED");
    assert!(!replay.state().offer(id).unwrap().reversible(now()));
}

#[test]
fn unsynchronized_or_rolled_back_clock_cannot_extend_a_deal() {
    let (mut s, memory) = house();
    let l = listing(&mut s, Side::Sell);
    let id = offer(&mut s, l, 20);
    exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
    for time in [0, START - 1] {
        set_time(time);
        let mut replay = Service::open_with_clock(memory.clone(), now).unwrap();
        assert_eq!(exec(&mut replay, 3, undo(id)), Err(Error::Unavailable));
        assert!(!replay.state().offer(id).unwrap().reversible(replay.now()));
    }
    set_time(START + DEFAULT_SETTLEMENT);
    assert_eq!(exec(&mut s, 3, undo(id)), Err(Error::OfferSettled));
}

#[test]
fn visibility_is_private_newest_first_and_state_endpoint_does_not_leak() {
    let (mut s, _) = house();
    let l = listing(&mut s, Side::Sell);
    let first = offer(&mut s, l, 20);
    let second = offer(&mut s, l, 15);
    for (name, count) in [("nana", 2), ("alice", 2), ("bob", 2), ("carol", 0)] {
        let auth = common::login_as(&mut s, name, "1234");
        let (status, page) = api(&mut s, "GET", "/api/v1/offers", &auth, "", json!(null));
        assert_eq!(status, 200);
        assert_eq!(page["offers"].as_array().unwrap().len(), count);
        if count == 2 {
            assert_eq!(page["offers"][0]["id"], format!("offer-{}", second.0));
        }
        let single = api(
            &mut s,
            "GET",
            &format!("/api/v1/offers/offer-{}", first.0),
            &auth,
            "",
            json!(null),
        );
        assert_eq!(single.0, if count == 0 { 404 } else { 200 });
        let (_, state) = api(&mut s, "GET", "/api/v1/state", &auth, "", json!(null));
        assert!(state["state"].get("offers").is_none());
    }
    assert_eq!(
        api(&mut s, "GET", "/api/v1/offers", "", "", json!(null)).0,
        401
    );
}

#[test]
fn bounded_offer_slots_recycle_closed_and_settled_but_never_live_deals() {
    let (mut s, memory) = house();
    let l = listing(&mut s, Side::Sell);
    let ids: Vec<_> = (0..OFFERS).map(|_| offer(&mut s, l, 1)).collect();
    let command = Command::MakeOffer {
        listing: l,
        amount: 2,
        message: OfferMessage::new(),
    };
    assert_eq!(exec(&mut s, 3, command.clone()), Err(Error::Capacity));
    exec(&mut s, 3, Command::WithdrawOffer { offer: ids[0] }).unwrap();
    exec(&mut s, 3, command).unwrap();
    assert!(s.state().offer(ids[0]).is_err());
    exec(&mut s, 2, Command::AcceptOffer { offer: ids[1] }).unwrap();
    let other_listing = listing(&mut s, Side::Sell);
    let command = Command::MakeOffer {
        listing: other_listing,
        amount: 1,
        message: OfferMessage::new(),
    };
    assert_eq!(exec(&mut s, 3, command.clone()), Err(Error::Capacity));
    set_time(START + DEFAULT_SETTLEMENT);
    exec(&mut s, 3, command).unwrap();
    assert!(s.state().offer(ids[1]).is_err());
    let replay = Service::open_with_clock(memory, now).unwrap();
    assert_eq!(replay.state().offers.len(), OFFERS);
    assert!(replay.state().offer(ids[1]).is_err());
}

#[test]
fn reversible_listing_is_pinned_until_undo_deadline() {
    let (mut s, _) = house();
    let l = listing(&mut s, Side::Sell);
    let id = offer(&mut s, l, 20);
    exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
    for _ in 1..LISTINGS {
        listing(&mut s, Side::Sell);
    }
    let command = Command::List {
        title: Title::try_from("Extra").unwrap(),
        description: Memo::new(),
        price: 10,
        side: Side::Sell,
        details: None,
    };
    assert_eq!(exec(&mut s, 2, command.clone()), Err(Error::Capacity));
    set_time(START + DEFAULT_SETTLEMENT);
    exec(&mut s, 2, command).unwrap();
    assert!(!s.state().listings.iter().any(|item| item.id == l));
    assert_eq!(exec(&mut s, 3, undo(id)), Err(Error::OfferSettled));
}

#[test]
fn failed_and_ambiguous_accept_or_undo_never_publish_half_a_deal() {
    for undoing in [false, true] {
        for failure in [1, 2] {
            let (mut s, memory) = house();
            let l = listing(&mut s, Side::Sell);
            let id = offer(&mut s, l, 20);
            if undoing {
                exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
            }
            let before = (
                balance(&s, 2),
                s.state().listings[0].status,
                s.state().offer(id).unwrap().phase,
            );
            let command = if undoing {
                undo(id)
            } else {
                Command::AcceptOffer { offer: id }
            };
            memory.fail.set(failure);
            assert_eq!(
                s.execute_keyed(MemberId(2), "deal", command.clone()),
                Err(Error::Storage)
            );
            assert_eq!(
                (
                    balance(&s, 2),
                    s.state().listings[0].status,
                    s.state().offer(id).unwrap().phase
                ),
                before
            );
            assert!(s.storage_failed());
            memory.fail.set(0);
            let mut replay = Service::open_with_clock(memory.clone(), now).unwrap();
            let receipt = replay.execute_keyed(MemberId(2), "deal", command).unwrap();
            assert_eq!(receipt.replayed, failure == 2);
            assert_eq!(balance(&replay, 2), if undoing { 100 } else { 120 });
            assert_eq!(
                replay.state().listings[0].status,
                if undoing {
                    ListingStatus::Active
                } else {
                    ListingStatus::Sold
                }
            );
            replay.state().check_invariants().unwrap();
        }
    }
}

#[test]
fn http_offer_contract_and_keyed_retries_survive_restart_and_undo() {
    let (mut s, memory) = house();
    let alice = common::login_as(&mut s, "alice", "1234");
    let bob = common::login_as(&mut s, "bob", "1234");
    let l = listing(&mut s, Side::Sell);
    let (status, proposal) = api(
        &mut s,
        "POST",
        &format!("/api/v1/listings/listing-{l}/offers"),
        &bob,
        "",
        json!({"amount":15,"message":"Saturday 🍪"}),
    );
    assert_eq!(status, 201, "{proposal}");
    assert_eq!(proposal["offerer"], "account-3");
    assert_eq!(proposal["listing_title"], "Cookies");
    assert_eq!(proposal["created_at"], START);
    let path = format!("/api/v1/offers/{}", proposal["id"].as_str().unwrap());
    assert_eq!(
        api(
            &mut s,
            "POST",
            &format!("{path}/accept"),
            &alice,
            "",
            json!({})
        )
        .0,
        400
    );
    let accepted = api(
        &mut s,
        "POST",
        &format!("{path}/accept"),
        &alice,
        "accept",
        json!({}),
    );
    assert_eq!(accepted.0, 201);
    assert_eq!(accepted.1["offer"]["status"], "ACCEPTED");
    assert_eq!(accepted.1["offer"]["reversible"], true);
    assert_eq!(
        api(
            &mut s,
            "POST",
            &format!("{path}/accept"),
            &alice,
            "accept",
            json!({})
        ),
        accepted
    );
    let mut s = Service::open_with_clock(memory, now).unwrap();
    let alice = common::login_as(&mut s, "alice", "1234");
    let bob = common::login_as(&mut s, "bob", "1234");
    let undone = api(
        &mut s,
        "POST",
        &format!("{path}/unaccept"),
        &bob,
        "undo",
        json!({"reason":"r".repeat(140)}),
    );
    assert_eq!(undone.0, 200);
    assert_eq!(undone.1["offer"]["status"], "REVERSED");
    assert_eq!(undone.1["transaction"]["description"], "r".repeat(140));
    assert_eq!(
        api(
            &mut s,
            "POST",
            &format!("{path}/unaccept"),
            &bob,
            "undo",
            json!({"reason":"r".repeat(140)})
        ),
        undone
    );
    assert_eq!(
        api(
            &mut s,
            "POST",
            &format!("{path}/unaccept"),
            &bob,
            "undo",
            json!({"reason":"Different"})
        )
        .0,
        409
    );
    assert_eq!(
        api(
            &mut s,
            "POST",
            &format!("{path}/accept"),
            &alice,
            "accept",
            json!({})
        ),
        accepted
    );
    assert_eq!((balance(&s, 2), balance(&s, 3)), (100, 100));
}

#[test]
fn nana_manual_reversal_prevents_a_second_offer_refund() {
    let (mut s, _) = house();
    let l = listing(&mut s, Side::Sell);
    let id = offer(&mut s, l, 20);
    let tx = exec(&mut s, 2, Command::AcceptOffer { offer: id }).unwrap();
    exec(
        &mut s,
        2,
        Command::Transfer {
            to: MemberId(4),
            amount: 120,
            memo: Memo::new(),
        },
    )
    .unwrap();
    exec(
        &mut s,
        1,
        Command::Reverse {
            transaction: tx.sequence,
            memo: Memo::new(),
        },
    )
    .unwrap();
    assert_eq!(exec(&mut s, 3, undo(id)), Err(Error::OfferClosed));
    assert_eq!((balance(&s, 2), balance(&s, 3)), (-20, 100));
    s.state().check_invariants().unwrap();
}

#[test]
fn bounded_wire_input_and_full_table_responses() {
    let (mut s, memory) = house();
    let bob = common::login_as(&mut s, "bob", "1234");
    let l = listing(&mut s, Side::Sell);
    let path = format!("/api/v1/listings/listing-{l}/offers");
    let count = memory.frames.borrow().len();
    for body in [
        json!({"amount":0}),
        json!({"amount":-1}),
        json!({"amount":1.5}),
        json!({"amount":MAX_AMOUNT + 1}),
        json!({"amount":1,"message":"x".repeat(141)}),
        json!({"amount":1,"message":"🍪".repeat(36)}),
        json!({"amount":1,"message":"hidden\u{0000}control"}),
        json!({"amount":1,"timestamp":START - 1000}),
    ] {
        assert_eq!(api(&mut s, "POST", &path, &bob, "", body).0, 400);
    }
    assert_eq!(memory.frames.borrow().len(), count);
    for _ in 0..OFFERS {
        assert_eq!(
            api(
                &mut s,
                "POST",
                &path,
                &bob,
                "",
                json!({"amount":1,"message":"\"\\".repeat(70)})
            )
            .0,
            201
        );
    }
    let (status, response) = api(&mut s, "GET", "/api/v1/offers", &bob, "", json!(null));
    assert_eq!(status, 200);
    assert_eq!(response["offers"].as_array().unwrap().len(), OFFERS);
    assert_eq!(response["offers"][0]["message"], "\"\\".repeat(70));
}
