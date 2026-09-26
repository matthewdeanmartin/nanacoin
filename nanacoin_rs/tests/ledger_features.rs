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

fn transfer<J: Journal>(s: &mut Service<J>, amount: i64) -> u64 {
    exec(
        s,
        2,
        Command::Transfer {
            to: MemberId(3),
            amount,
            memo: Memo::new(),
        },
    )
    .unwrap()
    .sequence
}
fn refund(id: u64, amount: i64) -> Command {
    Command::Refund {
        transaction: id,
        amount,
        memo: Memo::try_from("Return").unwrap(),
    }
}
fn reverse(id: u64) -> Command {
    Command::Reverse {
        transaction: id,
        memo: Memo::try_from("Correction").unwrap(),
    }
}
#[test]
fn partial_refunds_enforce_party_cumulative_limit_and_retry_then_replay() {
    let (mut s, m) = house();
    let original = transfer(&mut s, 100);
    assert_eq!(exec(&mut s, 2, refund(original, 25)), Err(Error::Forbidden));
    s.execute_keyed(MemberId(3), "partial-refund", refund(original, 25))
        .unwrap();
    assert!(
        s.execute_keyed(MemberId(3), "partial-refund", refund(original, 25))
            .unwrap()
            .replayed
    );
    assert_eq!(exec(&mut s, 1, reverse(original)), Err(Error::Conflict));
    assert_eq!(exec(&mut s, 3, refund(original, 76)), Err(Error::Conflict));
    exec(&mut s, 3, refund(original, 75)).unwrap();
    assert_eq!(exec(&mut s, 3, refund(original, 1)), Err(Error::Conflict));
    assert_eq!((balance(&s, 2), balance(&s, 3)), (10000, 10000));
    assert_eq!(s.state().ledger.refunded(original), 100);
    let mut reopened = Service::open_with_clock(m, now).unwrap();
    assert_eq!(reopened.state().ledger.refunded(original), 100);
    assert_eq!(
        exec(&mut reopened, 3, refund(original, 1)),
        Err(Error::Conflict)
    );
    assert_eq!(
        reopened.state().original_payment(original).unwrap().amount,
        100
    );
    reopened.state().check_invariants().unwrap();
}
#[test]
fn currency_epochs_preserve_original_units_and_scale_refund_cash() {
    let (mut s, _) = house();
    let original = transfer(&mut s, 100);
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
    exec(&mut s, 3, refund(original, 25)).unwrap();
    let tx = s.state().history.back().unwrap();
    assert_eq!(tx.amount, 250);
    assert_eq!(tx.meta.epoch, 1);
    assert_eq!(tx.meta.refund_units, 25);
    assert_eq!(s.state().original_payment(original).unwrap().amount, 100);
    assert_eq!(s.state().original_payment(original).unwrap().meta.epoch, 0);
    assert_eq!(s.state().ledger.refunded(original), 25);
    assert_eq!((balance(&s, 2), balance(&s, 3)), (99250, 100750));
    let mut out = vec![0; nanacoin::api::RESPONSE_LIMIT];
    let (status, used) = nanacoin::api::handle(
        &mut s,
        "GET",
        &format!("/api/v1/transactions/tx-{original}"),
        "",
        b"",
        &mut out,
    );
    assert_eq!(status, 200);
    let view: serde_json::Value = serde_json::from_slice(&out[..used]).unwrap();
    assert_eq!(view["postings"][1]["amount"], 100);
    assert_eq!(view["current_postings"][1]["amount"], 1000);
    assert_eq!(view["metadata"]["epoch"], 0);
    assert_eq!(view["current_money_epoch"], 1);
}
#[test]
fn gift_refunds_reduce_closed_request_progress_in_current_epoch() {
    let (mut s, _) = house();
    let request = exec(
        &mut s,
        2,
        Command::Commerce {
            action: Action::CreateRequest {
                title: Title::try_from("Supplies").unwrap(),
                description: Memo::new(),
                target: None,
                deadline: None,
            },
        },
    )
    .unwrap()
    .sequence;
    let gift = exec(
        &mut s,
        3,
        Command::Commerce {
            action: Action::Contribute {
                request,
                amount: 100,
                memo: Memo::new(),
            },
        },
    )
    .unwrap()
    .sequence;
    exec(
        &mut s,
        2,
        Command::Commerce {
            action: Action::CloseRequest { request },
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
    exec(&mut s, 2, refund(gift, 25)).unwrap();
    assert_eq!(s.state().gift_request(request).unwrap().received, 750);
    exec(&mut s, 2, refund(gift, 75)).unwrap();
    assert_eq!(s.state().gift_request(request).unwrap().received, 0);
    assert!(s.state().gift_request(request).unwrap().closed);
}
#[test]
fn art_sales_cannot_be_cash_refunded_or_reversed_without_ownership_return() {
    let (mut s, _) = house();
    let art = exec(
        &mut s,
        2,
        Command::Commerce {
            action: Action::MintArt {
                title: Title::try_from("Sunrise").unwrap(),
                license: Memo::try_from("Display").unwrap(),
                sha256: Digest::try_from(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )
                .unwrap(),
                locator: Locator::try_from("https://example.org/sunrise.png").unwrap(),
            },
        },
    )
    .unwrap()
    .sequence;
    let revision = exec(
        &mut s,
        2,
        Command::Commerce {
            action: Action::ListArt {
                art,
                price: Some(100),
            },
        },
    )
    .unwrap()
    .sequence;
    let sale = exec(
        &mut s,
        3,
        Command::Commerce {
            action: Action::BuyArt {
                art,
                expected_owner: MemberId(2),
                expected_revision: revision,
                expected_price: 100,
            },
        },
    )
    .unwrap()
    .sequence;
    assert_eq!(exec(&mut s, 2, refund(sale, 100)), Err(Error::Forbidden));
    assert!(exec(&mut s, 1, reverse(sale)).is_err());
    assert_eq!(s.state().artwork(art).unwrap().owner, MemberId(3));
    assert!(!s.state().fulfillments.iter().any(|f| f.transaction == sale));
    assert_eq!(balance(&s, 2), 10100);
}
#[test]
fn offer_undo_after_history_eviction_preserves_classification_and_blocks_repeat_reverse() {
    use nanacoin::offers::{OfferId, OfferMessage};
    let (mut s, _) = house();
    let economic = EconomicDetails {
        kind: EconomicKind::Labor,
        quantity_milli: 1000,
        unit: Unit::Hour,
        ..Default::default()
    };
    let listing = exec(
        &mut s,
        2,
        Command::ClassifiedList {
            title: Title::try_from("Cleaning").unwrap(),
            description: Memo::new(),
            price: 100,
            side: Side::Sell,
            economic,
            standard: false,
        },
    )
    .unwrap()
    .sequence;
    let offer = OfferId(
        exec(
            &mut s,
            3,
            Command::MakeOffer {
                listing,
                amount: 100,
                message: OfferMessage::new(),
            },
        )
        .unwrap()
        .sequence,
    );
    let payment = exec(&mut s, 2, Command::AcceptOffer { offer })
        .unwrap()
        .sequence;
    let expected = s.state().original_payment(payment).unwrap().economic;
    for _ in 0..HISTORY {
        transfer(&mut s, 1);
    }
    assert!(!s.state().history.iter().any(|t| t.id == payment));
    exec(
        &mut s,
        2,
        Command::UnacceptOffer {
            offer,
            reason: TransactionMemo::try_from("Undelivered").unwrap(),
        },
    )
    .unwrap();
    assert_eq!(s.state().history.back().unwrap().economic, expected);
    assert_eq!(s.state().history.back().unwrap().reverses, Some(payment));
    assert!(exec(&mut s, 1, reverse(payment)).is_err());
    s.state().check_invariants().unwrap();
}
#[test]
fn forex_reverse_retains_quote_and_cancels_exchange_counter() {
    use nanacoin::forex::QuoteSide;
    let (mut s, _) = house();
    exec(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(1),
            amount: 10000,
            memo: Memo::new(),
        },
    )
    .unwrap();
    exec(
        &mut s,
        1,
        Command::IssueUsd {
            to: MemberId(2),
            cents: 1000,
            memo: Memo::new(),
        },
    )
    .unwrap();
    let quote = exec(
        &mut s,
        1,
        Command::PostQuote {
            side: QuoteSide::ASK,
            cents_per_coin: 100,
            coins: 10000,
            expires_at: 0,
        },
    )
    .unwrap()
    .sequence;
    let taken = exec(&mut s, 2, Command::TakeQuote { quote })
        .unwrap()
        .sequence;
    let coin = s
        .state()
        .history
        .iter()
        .find(|t| t.id == taken)
        .unwrap()
        .clone();
    assert_eq!(coin.quote, Some(quote));
    let before = s.state().ledger.epochs[0].flows[9];
    assert_eq!(before, 10000);
    exec(&mut s, 1, reverse(coin.id)).unwrap();
    assert_eq!(s.state().history.back().unwrap().quote, Some(quote));
    assert_eq!(s.state().ledger.epochs[0].flows[9], 0);
}
#[cfg(feature = "desktop")]
#[test]
fn partial_refund_checkpoint_retains_retry_watermark_and_cumulative_amount() {
    use nanacoin::journal::file::FileJournal;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "nanacoin-ledger-refund-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("economy.journal");
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    setup(&mut s);
    let original = transfer(&mut s, 100);
    s.execute_keyed(MemberId(3), "refund-persist", refund(original, 40))
        .unwrap();
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut s = Service::open_with_clock(FileJournal::open(&path).unwrap(), now).unwrap();
    assert_eq!(s.state().ledger.refunded(original), 40);
    assert!(
        s.execute_keyed(MemberId(3), "refund-persist", refund(original, 40))
            .unwrap()
            .replayed
    );
    assert_eq!(exec(&mut s, 3, refund(original, 61)), Err(Error::Conflict));
    exec(&mut s, 3, refund(original, 60)).unwrap();
    assert_eq!((balance(&s, 2), balance(&s, 3)), (10000, 10000));
    drop(s);
    // Remove only this uniquely created test directory and its generated files.
    for entry in std::fs::read_dir(&dir).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(&dir).unwrap();
}
#[test]
fn refund_http_requires_key_and_totals_expose_exact_epoch_strings() {
    let (mut s, _) = house();
    let original = transfer(&mut s, 100);
    let auth = common::login_as(&mut s, "Bob", "1234");
    let path = format!("/api/v1/transactions/tx-{original}/refund");
    let mut out = vec![0; nanacoin::api::RESPONSE_LIMIT];
    let body = br#"{"amount":30,"reason":"Returned"}"#;
    let (status, _) = nanacoin::api::handle_keyed(&mut s, "POST", &path, &auth, "", body, &mut out);
    assert_eq!(status, 400);
    let (status, _) =
        nanacoin::api::handle_keyed(&mut s, "POST", &path, &auth, "http-refund", body, &mut out);
    assert_eq!(status, 200);
    let (status, _) =
        nanacoin::api::handle_keyed(&mut s, "POST", &path, &auth, "http-refund", body, &mut out);
    assert_eq!(status, 200);
    assert_eq!(s.state().ledger.refunded(original), 30);
    let (status, len) =
        nanacoin::api::handle(&mut s, "GET", "/api/v1/totals", &auth, &[], &mut out);
    assert_eq!(status, 200);
    let result: serde_json::Value = serde_json::from_slice(&out[..len]).unwrap();
    assert_eq!(result["epochs"][0]["epoch"], 0);
    assert_eq!(result["epochs"][0]["decimals"], 4);
    for (i, value) in result["epochs"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(
            value.as_str().unwrap().parse::<i128>().unwrap(),
            s.state().ledger.epochs[0].flows[i]
        );
    }
}
#[test]
fn fulfillment_payment_reverse_survives_ram_eviction_and_prevents_double_payment() {
    let (mut s, m) = house();
    let original = exec(
        &mut s,
        2,
        Command::ClassifiedTransfer {
            to: MemberId(3),
            amount: 100,
            memo: Memo::try_from("Work").unwrap(),
            economic: EconomicDetails {
                kind: EconomicKind::Labor,
                quantity_milli: 1000,
                unit: Unit::Hour,
                ..Default::default()
            },
        },
    )
    .unwrap()
    .sequence;
    for _ in 0..HISTORY {
        transfer(&mut s, 1);
    }
    assert!(!s.state().history.iter().any(|t| t.id == original));
    exec(&mut s, 1, reverse(original)).unwrap();
    let balance_after = balance(&s, 2);
    let mut reopened = Service::open_with_clock(m, now).unwrap();
    assert_eq!(
        exec(&mut reopened, 1, reverse(original)),
        Err(Error::Conflict)
    );
    assert_eq!(balance(&reopened, 2), balance_after);
    assert_eq!(
        reopened
            .state()
            .fulfillments
            .iter()
            .find(|f| f.transaction == original)
            .unwrap()
            .status,
        nanacoin::fulfillment::Status::Reversed
    );
}
#[test]
fn partial_refund_keeps_obligation_open_until_cumulative_full_refund() {
    let (mut s, _) = house();
    let original = exec(
        &mut s,
        2,
        Command::ClassifiedTransfer {
            to: MemberId(3),
            amount: 100,
            memo: Memo::try_from("Work").unwrap(),
            economic: EconomicDetails {
                kind: EconomicKind::Labor,
                quantity_milli: 1000,
                unit: Unit::Hour,
                ..Default::default()
            },
        },
    )
    .unwrap()
    .sequence;
    exec(&mut s, 3, refund(original, 25)).unwrap();
    assert_eq!(
        s.state()
            .fulfillments
            .iter()
            .find(|f| f.transaction == original)
            .unwrap()
            .status,
        nanacoin::fulfillment::Status::Todo
    );
    exec(&mut s, 3, refund(original, 75)).unwrap();
    assert_eq!(
        s.state()
            .fulfillments
            .iter()
            .find(|f| f.transaction == original)
            .unwrap()
            .status,
        nanacoin::fulfillment::Status::Reversed
    );
}
