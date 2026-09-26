mod common;
use nanacoin::{auth::PasswordVerifier, commerce::*, domain::*, journal::*};
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

fn action(s: &mut Service<Memory>, who: u8, action: Action) -> Result<Receipt, Error> {
    exec(s, who, Command::Commerce { action })
}
fn mint(s: &mut Service<Memory>) -> u64 {
    action(
        s,
        2,
        Action::MintArt {
            title: Title::try_from("Sunrise").unwrap(),
            license: Memo::try_from("Profile display").unwrap(),
            sha256: Digest::try_from(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )
            .unwrap(),
            locator: Locator::try_from("https://example.org/sunrise.png").unwrap(),
        },
    )
    .unwrap()
    .sequence
}
#[test]
fn art_sale_is_atomic_authorized_stale_safe_and_replayable() {
    let (mut s, m) = house();
    let art = mint(&mut s);
    assert_eq!(
        action(
            &mut s,
            3,
            Action::ListArt {
                art,
                price: Some(100)
            }
        ),
        Err(Error::Forbidden)
    );
    let revision = action(
        &mut s,
        2,
        Action::ListArt {
            art,
            price: Some(100),
        },
    )
    .unwrap()
    .sequence;
    let buy = Action::BuyArt {
        art,
        expected_owner: MemberId(2),
        expected_revision: revision,
        expected_price: 100,
    };
    assert_eq!(
        action(
            &mut s,
            3,
            Action::BuyArt {
                art,
                expected_owner: MemberId(2),
                expected_revision: revision - 1,
                expected_price: 100
            }
        ),
        Err(Error::Conflict)
    );
    let receipt = s
        .execute_keyed(
            MemberId(3),
            "art-sale",
            Command::Commerce {
                action: buy.clone(),
            },
        )
        .unwrap();
    assert!(
        s.execute_keyed(
            MemberId(3),
            "art-sale",
            Command::Commerce {
                action: buy.clone()
            }
        )
        .unwrap()
        .replayed
    );
    assert_eq!(action(&mut s, 3, buy), Err(Error::Conflict));
    assert_eq!((balance(&s, 2), balance(&s, 3)), (10100, 9900));
    assert_eq!(s.state().artwork(art).unwrap().owner, MemberId(3));
    assert_eq!(s.state().history.back().unwrap().meta.art, Some(art));
    assert_eq!(s.state().history.back().unwrap().id, receipt.sequence);
    action(
        &mut s,
        3,
        Action::EquipArt {
            art,
            equipped: true,
        },
    )
    .unwrap();
    action(
        &mut s,
        3,
        Action::GiftArt {
            art,
            to: MemberId(2),
        },
    )
    .unwrap();
    assert!(!s.state().artwork(art).unwrap().equipped);
    let reopened = Service::open_with_clock(m, now).unwrap();
    assert_eq!(reopened.state().artwork(art).unwrap().owner, MemberId(2));
    assert_eq!(balance(&reopened, 3), 9900);
    reopened.state().check_invariants().unwrap();
}
#[test]
fn gift_request_overfunding_deadline_closure_and_posting() {
    let (mut s, m) = house();
    let request = action(
        &mut s,
        2,
        Action::CreateRequest {
            title: Title::try_from("Supplies").unwrap(),
            description: Memo::new(),
            target: Some(50),
            deadline: Some(START + 100),
        },
    )
    .unwrap()
    .sequence;
    assert_eq!(
        action(&mut s, 3, Action::CloseRequest { request }),
        Err(Error::Forbidden)
    );
    assert_eq!(
        action(
            &mut s,
            2,
            Action::Contribute {
                request,
                amount: 100,
                memo: Memo::new()
            }
        ),
        Err(Error::InvalidInput)
    );
    action(
        &mut s,
        3,
        Action::Contribute {
            request,
            amount: 100,
            memo: Memo::new(),
        },
    )
    .unwrap();
    assert_eq!(s.state().gift_request(request).unwrap().received, 100);
    let tx = s.state().history.back().unwrap();
    assert_eq!(tx.meta.gift_request, Some(request));
    assert_eq!(tx.economic.kind, EconomicKind::Gift);
    time(START + 100);
    assert_eq!(
        action(
            &mut s,
            3,
            Action::Contribute {
                request,
                amount: 100,
                memo: Memo::new()
            }
        ),
        Err(Error::Conflict)
    );
    action(&mut s, 2, Action::CloseRequest { request }).unwrap();
    let reopened = Service::open_with_clock(m, now).unwrap();
    assert_eq!(
        reopened.state().gift_request(request).unwrap().received,
        100
    );
    assert!(reopened.state().gift_request(request).unwrap().closed);
    reopened.state().check_invariants().unwrap();
}
#[test]
fn failed_append_never_transfers_art_or_cash() {
    let (mut s, m) = house();
    let art = mint(&mut s);
    let revision = action(
        &mut s,
        2,
        Action::ListArt {
            art,
            price: Some(100),
        },
    )
    .unwrap()
    .sequence;
    m.fail.set(1);
    assert_eq!(
        action(
            &mut s,
            3,
            Action::BuyArt {
                art,
                expected_owner: MemberId(2),
                expected_revision: revision,
                expected_price: 100
            }
        ),
        Err(Error::Storage)
    );
    assert_eq!(s.state().artwork(art).unwrap().owner, MemberId(2));
    assert_eq!(balance(&s, 3), 10000);
    m.fail.set(0);
    let reopened = Service::open_with_clock(m, now).unwrap();
    assert_eq!(reopened.state().artwork(art).unwrap().owner, MemberId(2));
}
#[test]
fn api_requires_authentication_and_key_and_rejects_oversize_metadata() {
    let (mut s, _) = house();
    let mut out = vec![0; nanacoin::api::RESPONSE_LIMIT];
    let body = br#"{"create_request":{"title":"Supplies","description":"","target":null,"deadline":null}}"#;
    let (status, _) = nanacoin::api::handle_keyed(
        &mut s,
        "POST",
        "/api/v1/commerce/commands",
        "",
        "request-create",
        body,
        &mut out,
    );
    assert_eq!(status, 401);
    let auth = common::login_as(&mut s, "Alice", "1234");
    let (status, _) = nanacoin::api::handle_keyed(
        &mut s,
        "POST",
        "/api/v1/commerce/commands",
        &auth,
        "",
        body,
        &mut out,
    );
    assert_eq!(status, 400);
    let (status, _) = nanacoin::api::handle_keyed(
        &mut s,
        "POST",
        "/api/v1/commerce/commands",
        &auth,
        "request-create",
        body,
        &mut out,
    );
    assert_eq!(status, 200);
    let (status, len) = nanacoin::api::handle_keyed(
        &mut s,
        "POST",
        "/api/v1/commerce/commands",
        &auth,
        "request-create",
        body,
        &mut out,
    );
    assert_eq!(status, 200);
    let receipt: serde_json::Value = serde_json::from_slice(&out[..len]).unwrap();
    assert_eq!(receipt["replayed"], true);
    assert_eq!(s.state().commerce.requests.len(), 1);
    let oversized = serde_json::to_vec(&serde_json::json!({"create_request":{"title":"x".repeat(81),"description":"","target":null,"deadline":null}})).unwrap();
    let (status, _) = nanacoin::api::handle_keyed(
        &mut s,
        "POST",
        "/api/v1/commerce/commands",
        &auth,
        "oversized",
        &oversized,
        &mut out,
    );
    assert_eq!(status, 400);
}
#[test]
fn currency_reform_scales_live_commerce_and_retains_sale_and_gift_units() {
    let (mut s, _) = house();
    let request = action(
        &mut s,
        2,
        Action::CreateRequest {
            title: Title::try_from("Supplies").unwrap(),
            description: Memo::new(),
            target: Some(1000),
            deadline: None,
        },
    )
    .unwrap()
    .sequence;
    let gift = action(
        &mut s,
        3,
        Action::Contribute {
            request,
            amount: 100,
            memo: Memo::new(),
        },
    )
    .unwrap()
    .sequence;
    let art = mint(&mut s);
    action(
        &mut s,
        2,
        Action::ListArt {
            art,
            price: Some(250),
        },
    )
    .unwrap();
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
    assert_eq!(s.state().gift_request(request).unwrap().target, Some(10000));
    assert_eq!(s.state().gift_request(request).unwrap().received, 1000);
    assert_eq!(s.state().artwork(art).unwrap().price, Some(2500));
    assert_eq!(
        s.state()
            .history
            .iter()
            .find(|t| t.id == gift)
            .unwrap()
            .amount,
        100
    );
    s.state().check_invariants().unwrap();
}
#[test]
fn commerce_get_is_authenticated_and_exposes_current_ownership() {
    let (mut s, _) = house();
    let art = mint(&mut s);
    let mut out = vec![0; nanacoin::api::RESPONSE_LIMIT];
    let (status, _) = nanacoin::api::handle(&mut s, "GET", "/api/v1/commerce", "", &[], &mut out);
    assert_eq!(status, 401);
    let auth = common::login_as(&mut s, "Bob", "1234");
    let (status, len) =
        nanacoin::api::handle(&mut s, "GET", "/api/v1/commerce", &auth, &[], &mut out);
    assert_eq!(status, 200);
    let result: serde_json::Value = serde_json::from_slice(&out[..len]).unwrap();
    assert_eq!(result["artworks"][0]["id"], art);
    assert_eq!(result["artworks"][0]["owner"], 2);
}
#[test]
fn digital_art_sale_succeeds_with_full_physical_fulfillment_capacity() {
    let (mut s, _) = house();
    for _ in 0..nanacoin::fulfillment::CAPACITY {
        exec(
            &mut s,
            2,
            Command::ClassifiedTransfer {
                to: MemberId(3),
                amount: 1,
                memo: Memo::try_from("Work").unwrap(),
                economic: EconomicDetails {
                    kind: EconomicKind::Labor,
                    quantity_milli: 1000,
                    unit: Unit::Hour,
                    ..Default::default()
                },
            },
        )
        .unwrap();
    }
    let art = mint(&mut s);
    let revision = action(
        &mut s,
        2,
        Action::ListArt {
            art,
            price: Some(100),
        },
    )
    .unwrap()
    .sequence;
    action(
        &mut s,
        3,
        Action::BuyArt {
            art,
            expected_owner: MemberId(2),
            expected_revision: revision,
            expected_price: 100,
        },
    )
    .unwrap();
    assert_eq!(s.state().artwork(art).unwrap().owner, MemberId(3));
    assert_eq!(
        s.state().fulfillments.len(),
        nanacoin::fulfillment::CAPACITY
    );
    s.state().check_invariants().unwrap();
}
