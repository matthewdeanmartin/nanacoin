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
    outbox: Rc<RefCell<nanacoin::screen::Outbox>>,
    fail_screen: Rc<Cell<bool>>,
}
impl Journal for Memory {
    fn supports_screen(&self) -> bool {
        true
    }
    fn screen_outbox(&self) -> Result<nanacoin::screen::Outbox, Error> {
        Ok(self.outbox.borrow().clone())
    }
    fn set_screen_outbox(&mut self, outbox: &nanacoin::screen::Outbox) -> Result<(), Error> {
        if self.fail_screen.get() {
            return Err(Error::Storage);
        }
        *self.outbox.borrow_mut() = outbox.clone();
        Ok(())
    }

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

#[test]
fn committed_lotto_notification_survives_restart_and_retry() {
    let (mut s, m) = house();
    let id = create(&mut s, LottoKind::Simple, 0);
    assert_eq!(s.screen_pending().len(), 1);
    assert_eq!(s.screen_pending()[0].text, "New Lotto created: Family draw");
    assert_eq!(s.screen_pending()[0].expires_at, START + 86400);
    let request = s.state().member(MemberId(1)).unwrap().last_request;
    let command = Command::CreateLotto {
        terms: s.state().lotto(id).unwrap().terms.clone(),
    };
    assert!(s.execute(MemberId(1), request, command).unwrap().replayed);
    drop(s);
    let s = Service::open_with_clock(m, now).unwrap();
    assert_eq!(s.screen_pending().len(), 1);
    assert_eq!(s.screen_pending()[0].expires_at, START + 86400);
}
#[test]
fn notification_storage_failure_cannot_undo_or_repeat_lotto() {
    let (mut s, m) = house();
    m.fail_screen.set(true);
    let id = create(&mut s, LottoKind::Simple, 0);
    assert!(s.state().lotto(id).is_ok());
    assert!(!s.storage_failed());
    assert!(s.screen_pending().is_empty());
    let command = Command::CreateLotto {
        terms: s.state().lotto(id).unwrap().terms.clone(),
    };
    let request = s.state().member(MemberId(1)).unwrap().last_request;
    assert!(s.execute(MemberId(1), request, command).unwrap().replayed);
}
#[test]
fn message_copy_is_sender_only_read_is_recipient_only_and_supersedes_pending_copy() {
    let (mut s, _) = house();
    let tx = exec(
        &mut s,
        2,
        Command::Transfer {
            to: MemberId(3),
            amount: 0,
            memo: Memo::try_from("Come to dinner").unwrap(),
        },
    )
    .unwrap()
    .sequence;
    assert!(s.screen_pending().is_empty());
    assert_eq!(
        s.screen_message(MemberId(3), tx, false),
        Err(Error::Forbidden)
    );
    s.screen_message(MemberId(2), tx, false).unwrap();
    s.screen_message(MemberId(2), tx, false).unwrap();
    assert_eq!(s.screen_pending().len(), 1);
    assert_eq!(s.screen_pending()[0].recipient, "Bob");
    assert_eq!(s.screen_pending()[0].text, "Alice: Come to dinner");
    assert_eq!(
        s.screen_message(MemberId(2), tx, true),
        Err(Error::Forbidden)
    );
    s.screen_message(MemberId(3), tx, true).unwrap();
    assert_eq!(s.screen_pending().len(), 1);
    assert!(s.screen_pending()[0].read);
    time(START + 86401);
    assert_eq!(
        s.screen_message(MemberId(2), tx, false),
        Err(Error::InvalidInput)
    );
}
#[test]
fn screen_api_requires_login_and_does_not_append_another_transfer() {
    let (mut s, m) = house();
    let tx = exec(
        &mut s,
        2,
        Command::Transfer {
            to: MemberId(3),
            amount: 0,
            memo: Memo::try_from("Hello").unwrap(),
        },
    )
    .unwrap()
    .sequence;
    let path = format!("/api/v1/transactions/tx-{tx}/screen");
    let frames = m.frames.borrow().len();
    assert_eq!(
        common::call(&mut s, &path, "", serde_json::json!({})).0,
        401
    );
    let alice = common::login_as(&mut s, "Alice", "1234");
    assert_eq!(
        common::call(&mut s, &path, &alice, serde_json::json!({})).0,
        200
    );
    assert_eq!(m.frames.borrow().len(), frames);
}
#[test]
fn bounded_outbox_encoding_detects_corruption() {
    let (mut s, _) = house();
    create(&mut s, LottoKind::Simple, 0);
    let outbox = nanacoin::screen::Outbox {
        pending: s.screen_pending().iter().cloned().collect(),
    };
    let mut bytes = [0; nanacoin::screen::STORAGE_BYTES];
    let encoded = outbox.encode(&mut bytes).unwrap();
    assert_eq!(
        nanacoin::screen::Outbox::decode(encoded).unwrap().pending,
        outbox.pending
    );
    bytes[5] ^= 1;
    assert!(nanacoin::screen::Outbox::decode(&bytes).is_err());
}

#[test]
fn stats_distinguish_missing_prices_and_loans_from_measured_employment() {
    let (s, _) = house();
    let cards = nanacoin::screen::stats(s.state(), START);
    assert!(cards[0].contains("Inflation: No data"));
    assert!(cards[0].contains("Employment: 0% (0/2)"));
    assert!(cards[1].contains("Interest: No active loans"));
    assert!(cards[1].contains("Exchange: No trades"));
}
#[test]
fn listing_and_negotiation_events_queue_only_after_commits_and_dedupe_retries() {
    let (mut s, _) = house();
    let command = Command::List {
        title: Title::try_from("Playground trip").unwrap(),
        description: Memo::new(),
        price: 35,
        side: Side::Buy,
        details: None,
    };
    let receipt = exec(&mut s, 2, command.clone()).unwrap();
    assert!(s.screen_pending()[0]
        .text
        .contains("New listing: Playground trip"));
    let request = s.state().member(MemberId(2)).unwrap().last_request;
    assert!(s.execute(MemberId(2), request, command).unwrap().replayed);
    assert_eq!(s.screen_pending().len(), 1);
    exec(
        &mut s,
        3,
        Command::MakeOffer {
            listing: receipt.sequence,
            amount: 35,
            message: Default::default(),
        },
    )
    .unwrap();
    assert!(s.screen_pending()[1]
        .text
        .contains("awaiting Alice's acceptance"));
    let offer = s.state().offers.last().unwrap().id;
    exec(&mut s, 2, Command::AcceptOffer { offer }).unwrap();
    assert!(s.screen_pending()[2].text.contains("Offer accepted"));
}

#[test]
fn stats_measure_repeat_sales_labor_and_cash_funded_interest() {
    let (mut s, _) = house();
    for price in [100, 200] {
        let listing = exec(
            &mut s,
            2,
            Command::ClassifiedList {
                title: Title::try_from("Cookies").unwrap(),
                description: Memo::new(),
                price,
                side: Side::Sell,
                economic: EconomicDetails {
                    kind: EconomicKind::Good,
                    quantity_milli: 1000,
                    unit: Unit::Each,
                    ..Default::default()
                },
                standard: true,
            },
        )
        .unwrap()
        .sequence;
        exec(&mut s, 3, Command::Buy { listing }).unwrap();
    }
    let labor = exec(
        &mut s,
        3,
        Command::ClassifiedTransfer {
            to: MemberId(2),
            amount: 100,
            memo: Memo::try_from("Work").unwrap(),
            economic: EconomicDetails {
                kind: EconomicKind::Labor,
                quantity_milli: 1000,
                unit: Unit::Task,
                ..Default::default()
            },
        },
    )
    .unwrap()
    .sequence;
    let loan = exec(
        &mut s,
        2,
        Command::OfferLoan {
            terms: nanacoin::loans::LoanTerms {
                borrower: MemberId(3),
                amount: 1000,
                rate_bps: 500,
                rate_days: 365,
                payment_days: 7,
                installment: 100,
                credit: false,
                memo: Memo::new(),
            },
        },
    )
    .unwrap()
    .sequence;
    exec(&mut s, 3, Command::AcceptLoan { loan }).unwrap();
    let cards = nanacoin::screen::stats(s.state(), START);
    assert!(cards[0].contains("Inflation: 100.0%"), "{}", cards[0]);
    assert!(cards[0].contains("Employment: 50% (1/2)"), "{}", cards[0]);
    assert!(cards[1].contains("Interest: 5.00%/yr"), "{}", cards[1]);
    exec(
        &mut s,
        2,
        Command::Reverse {
            transaction: labor,
            memo: Memo::new(),
        },
    )
    .unwrap();
    assert!(nanacoin::screen::stats(s.state(), START)[0].contains("Employment: 0%"));
    s.state().check_invariants().unwrap();
}

#[test]
fn exchange_stats_use_retained_legs_after_quote_book_recycling() {
    let (mut s, _) = house();
    exec(
        &mut s,
        1,
        Command::IssueUsd {
            to: MemberId(3),
            cents: 10000,
            memo: Memo::new(),
        },
    )
    .unwrap();
    let quote = exec(
        &mut s,
        2,
        Command::PostQuote {
            side: nanacoin::forex::QuoteSide::ASK,
            cents_per_coin: 250,
            coins: 10000,
            expires_at: 0,
        },
    )
    .unwrap()
    .sequence;
    exec(&mut s, 3, Command::TakeQuote { quote }).unwrap();
    assert!(nanacoin::screen::stats(s.state(), START)[1].contains("Exchange: $2.50/NC"));
    // Force the bounded quote book to recycle the filled row, while keeping
    // its immutable transaction legs in history.
    for _ in 0..16 {
        let q = exec(
            &mut s,
            2,
            Command::PostQuote {
                side: nanacoin::forex::QuoteSide::ASK,
                cents_per_coin: 100,
                coins: 100,
                expires_at: 0,
            },
        )
        .unwrap()
        .sequence;
        exec(&mut s, 2, Command::CancelQuote { quote: q }).unwrap();
    }
    assert!(!s.state().quotes.iter().any(|q| q.id == quote));
    assert!(nanacoin::screen::stats(s.state(), START)[1].contains("Exchange: $2.50/NC"));
}
