mod common;
use nanacoin::{api, domain::*, journal::*};
use std::{cell::RefCell, rc::Rc};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "abcdef0123456789abcdef0123456789";

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>, Rc<RefCell<bool>>);
impl Journal for Memory {
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        match self.0.borrow().get(index) {
            Some(data) => {
                *frame = *data;
                Ok(true)
            }
            None => Ok(false),
        }
    }
    fn append(&mut self, index: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        if *self.1.borrow() {
            return Err(Error::Storage);
        }
        assert_eq!(index, self.0.borrow().len());
        self.0.borrow_mut().push(*frame);
        Ok(())
    }
}
fn household() -> (Service<Memory>, Memory) {
    let memory = Memory::default();
    let mut s = Service::open(memory.clone()).unwrap();
    common::provision(&mut s);
    s.execute(
        MemberId(1),
        2,
        Command::AddMember {
            name: Name::try_from("Alice").unwrap(),
            token_hash: token_hash(OTHER).unwrap(),
        },
    )
    .unwrap();
    (s, memory)
}
fn issue(amount: i64) -> Command {
    Command::Issue {
        to: MemberId(2),
        amount,
        memo: Memo::new(),
    }
}
fn execute(s: &mut Service<Memory>, actor: u8, command: Command) -> Result<Receipt, Error> {
    let nonce = s.state().member(MemberId(actor)).unwrap().last_request + 1;
    s.execute(MemberId(actor), nonce, command)
}

#[test]
fn financial_classifications_survive_replay_and_corrections_without_issuance() {
    let (mut s, memory) = household();
    execute(&mut s, 1, issue(100)).unwrap();
    let mut payments = Vec::new();
    for kind in [EconomicKind::LoanPrincipal, EconomicKind::Interest] {
        let receipt = execute(
            &mut s,
            2,
            Command::ClassifiedTransfer {
                to: MemberId(1),
                amount: 10,
                memo: Memo::new(),
                economic: EconomicDetails {
                    kind,
                    quantity_milli: 1000,
                    ..EconomicDetails::default()
                },
            },
        )
        .unwrap();
        payments.push((receipt.sequence, kind));
    }
    let mut replay = Service::open(memory).unwrap();
    for (id, kind) in payments {
        let original = replay.state().history.iter().find(|t| t.id == id).unwrap();
        assert_eq!(original.economic.kind, kind);
        execute(
            &mut replay,
            1,
            Command::Reverse {
                transaction: id,
                memo: Memo::new(),
            },
        )
        .unwrap();
        assert_eq!(replay.state().history.back().unwrap().economic.kind, kind);
    }
    assert_eq!(replay.state().issuance_balance, -100);
    assert_eq!(replay.state().member(MemberId(2)).unwrap().balance, 100);
    replay.state().check_invariants().unwrap();
}

#[test]
fn ledger_survives_replay_and_keeps_double_entry_balances() {
    let (mut s, memory) = household();
    execute(&mut s, 1, issue(100)).unwrap();
    execute(
        &mut s,
        2,
        Command::Transfer {
            to: MemberId(1),
            amount: 25,
            memo: Memo::try_from("Thanks, Nana").unwrap(),
        },
    )
    .unwrap();
    let reopened = Service::open(memory).unwrap();
    assert_eq!(reopened.state().member(MemberId(2)).unwrap().balance, 75);
    assert_eq!(reopened.state().issuance_balance, -100);
    reopened.state().check_invariants().unwrap();
    assert_eq!(reopened.state().authenticate_legacy(OTHER), Ok(MemberId(2)));
}

#[test]
fn recipient_can_refund_but_payer_cannot_reverse_their_own_payment() {
    let (mut s, _) = household();
    execute(
        &mut s,
        1,
        Command::AddMember {
            name: Name::try_from("Bob").unwrap(),
            token_hash: [3; 32],
        },
    )
    .unwrap();
    execute(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(2),
            amount: 30,
            memo: Memo::new(),
        },
    )
    .unwrap();
    let payment = execute(
        &mut s,
        2,
        Command::ClassifiedTransfer {
            to: MemberId(3),
            amount: 20,
            memo: Memo::try_from("Mow lawn").unwrap(),
            economic: EconomicDetails {
                kind: EconomicKind::Labor,
                quantity_milli: 1000,
                unit: Unit::Task,
                ..EconomicDetails::default()
            },
        },
    )
    .unwrap()
    .sequence;
    assert_eq!(
        execute(
            &mut s,
            2,
            Command::Reverse {
                transaction: payment,
                memo: Memo::new()
            },
        ),
        Err(Error::Forbidden)
    );
    execute(
        &mut s,
        3,
        Command::Reverse {
            transaction: payment,
            memo: Memo::try_from("Refund").unwrap(),
        },
    )
    .unwrap();
    let refund = s.state().history.back().unwrap();
    assert_eq!(refund.reverses, Some(payment));
    assert_eq!(refund.economic.kind, EconomicKind::Labor);
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 30);
    assert_eq!(s.state().member(MemberId(3)).unwrap().balance, 0);
}

#[test]
fn authorization_and_input_fail_without_writes() {
    let (mut s, memory) = household();
    for (actor, command, error) in [
        (2, issue(1), Error::Forbidden),
        (1, issue(0), Error::InvalidInput),
        (1, issue(-1), Error::InvalidInput),
        (1, issue(i64::MAX), Error::InvalidInput),
        (
            2,
            Command::Transfer {
                to: MemberId(1),
                amount: 1,
                memo: Memo::new(),
            },
            Error::InsufficientFunds,
        ),
        (
            2,
            Command::Transfer {
                to: MemberId(0),
                amount: 1,
                memo: Memo::new(),
            },
            Error::NotFound,
        ),
    ] {
        assert_eq!(execute(&mut s, actor, command), Err(error));
    }
    assert_eq!(memory.0.borrow().len(), 2);
}

#[test]
fn classified_exchange_reuses_things_and_keeps_nana_out_of_labor() {
    let (mut s, memory) = household();
    execute(&mut s, 1, issue(100)).unwrap();
    execute(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(1),
            amount: 100,
            memo: Memo::new(),
        },
    )
    .unwrap();
    let labor = EconomicDetails {
        kind: EconomicKind::Labor,
        thing: 0,
        quantity_milli: 1_000,
        unit: Unit::Task,
    };
    assert_eq!(
        execute(
            &mut s,
            1,
            Command::ClassifiedList {
                title: Title::try_from("Nana labor").unwrap(),
                description: Memo::new(),
                price: 10,
                side: Side::Sell,
                economic: labor,
                standard: false,
            },
        ),
        Err(Error::Forbidden)
    );
    assert_eq!(
        execute(
            &mut s,
            2,
            Command::ClassifiedTransfer {
                to: MemberId(1),
                amount: 10,
                memo: Memo::try_from("work").unwrap(),
                economic: labor,
            },
        ),
        Err(Error::Forbidden)
    );

    let good = EconomicDetails {
        kind: EconomicKind::Good,
        thing: 0,
        quantity_milli: 1_500,
        unit: Unit::Batch,
    };
    let first = execute(
        &mut s,
        2,
        Command::ClassifiedList {
            title: Title::try_from("Mexican wedding cookies").unwrap(),
            description: Memo::new(),
            price: 50,
            side: Side::Sell,
            economic: good,
            standard: true,
        },
    )
    .unwrap();
    execute(
        &mut s,
        1,
        Command::Buy {
            listing: first.sequence,
        },
    )
    .unwrap();
    execute(
        &mut s,
        2,
        Command::ClassifiedList {
            title: Title::try_from("MEXICAN WEDDING COOKIES").unwrap(),
            description: Memo::new(),
            price: 70,
            side: Side::Sell,
            economic: good,
            standard: false,
        },
    )
    .unwrap();

    assert_eq!(s.state().things.len(), 1);
    assert!(s.state().things[0].standard);
    assert_eq!(
        s.state().listings.last().unwrap().economic.thing,
        first.sequence
    );
    let purchase = s
        .state()
        .history
        .iter()
        .find(|t| t.listing == Some(first.sequence))
        .unwrap();
    assert_eq!(purchase.economic.quantity_milli, 1_500);
    assert_eq!(purchase.economic.kind, EconomicKind::Good);

    let reopened = Service::open(memory).unwrap();
    assert_eq!(reopened.state().things.len(), 1);
    assert_eq!(
        reopened.state().things[0].name.as_str(),
        "Mexican wedding cookies"
    );
}

#[test]
fn failed_persistence_does_not_publish_and_latches_until_replay() {
    let (mut s, memory) = household();
    *memory.1.borrow_mut() = true;
    assert_eq!(execute(&mut s, 1, issue(100)), Err(Error::Storage));
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 0);
    *memory.1.borrow_mut() = false;
    assert_eq!(execute(&mut s, 1, issue(100)), Err(Error::Storage));
    let mut recovered = Service::open(memory).unwrap();
    execute(&mut recovered, 1, issue(100)).unwrap();
}

#[test]
fn retries_are_durable_and_cannot_change_payload() {
    let (mut s, memory) = household();
    let receipt = s.execute(MemberId(1), 3, issue(100)).unwrap();
    let mut s = Service::open(memory.clone()).unwrap();
    assert_eq!(
        s.execute(MemberId(1), 3, issue(100)),
        Ok(Receipt {
            replayed: true,
            ..receipt
        })
    );
    assert_eq!(s.execute(MemberId(1), 3, issue(101)), Err(Error::Conflict));
    execute(&mut s, 1, issue(10)).unwrap();
    assert_eq!(
        s.execute(MemberId(1), 3, issue(100)),
        Err(Error::StaleRequest)
    );
    assert_eq!(memory.0.borrow().len(), 4);
}

#[test]
fn purchase_is_atomic_and_reversal_cannot_be_repeated() {
    let (mut s, memory) = household();
    let listing = execute(
        &mut s,
        1,
        Command::List {
            description: Memo::new(),
            title: Title::try_from("Switch time").unwrap(),
            price: 10,
            side: Side::Sell,
            details: None,
        },
    )
    .unwrap()
    .sequence;
    assert_eq!(
        execute(&mut s, 2, Command::Buy { listing }),
        Err(Error::InsufficientFunds)
    );
    assert_eq!(s.state().listings[0].status, ListingStatus::Active);
    execute(&mut s, 1, issue(10)).unwrap();
    let tx = execute(&mut s, 2, Command::Buy { listing })
        .unwrap()
        .sequence;
    assert_eq!(
        execute(&mut s, 2, Command::Buy { listing }),
        Err(Error::Conflict)
    );
    execute(
        &mut s,
        1,
        Command::Reverse {
            transaction: tx,
            memo: Memo::try_from("Not delivered").unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        execute(
            &mut s,
            1,
            Command::Reverse {
                transaction: tx,
                memo: Memo::new()
            }
        ),
        Err(Error::Conflict)
    );
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 10);
    assert_eq!(s.state().listings[0].status, ListingStatus::Sold);
    Service::open(memory)
        .unwrap()
        .state()
        .check_invariants()
        .unwrap();
}

#[test]
fn want_ad_pays_the_accepting_member() {
    let (mut s, _) = household();
    execute(&mut s, 1, issue(20)).unwrap();
    let listing = execute(
        &mut s,
        2,
        Command::List {
            description: Memo::new(),
            title: Title::try_from("Cookies please").unwrap(),
            price: 5,
            side: Side::Buy,
            details: None,
        },
    )
    .unwrap()
    .sequence;
    execute(&mut s, 1, Command::Buy { listing }).unwrap();
    assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 5);
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 15);
}

#[test]
fn bounded_history_preserves_lifetime_balances_and_retry_watermarks() {
    let (mut s, memory) = household();
    for _ in 0..HISTORY + 100 {
        execute(&mut s, 1, issue(1)).unwrap();
    }
    assert_eq!(s.state().history.len(), HISTORY);
    assert_eq!(
        s.state().member(MemberId(2)).unwrap().balance,
        (HISTORY + 100) as i64
    );
    assert_eq!(
        execute(
            &mut s,
            1,
            Command::Reverse {
                transaction: 3,
                memo: Memo::new()
            }
        ),
        Err(Error::NotFound)
    );
    let s = Service::open(memory).unwrap();
    assert_eq!(s.state().history.len(), HISTORY);
    s.state().check_invariants().unwrap();
}

#[test]
fn member_and_listing_capacities_fail_without_partial_mutation() {
    let (mut s, _) = household();
    for i in 2..MEMBERS {
        execute(
            &mut s,
            1,
            Command::AddMember {
                name: Name::try_from(format!("Member{i}").as_str()).unwrap(),
                token_hash: [i as u8; 32],
            },
        )
        .unwrap();
    }
    assert_eq!(
        execute(
            &mut s,
            1,
            Command::AddMember {
                name: Name::try_from("Too many").unwrap(),
                token_hash: [99; 32]
            }
        ),
        Err(Error::Capacity)
    );
    for _ in 0..LISTINGS {
        execute(
            &mut s,
            1,
            Command::List {
                description: Memo::new(),
                title: Title::try_from("Thing").unwrap(),
                price: 1,
                side: Side::Sell,
                details: None,
            },
        )
        .unwrap();
    }
    assert_eq!(
        execute(
            &mut s,
            1,
            Command::List {
                description: Memo::new(),
                title: Title::try_from("Too many").unwrap(),
                price: 1,
                side: Side::Sell,
                details: None,
            }
        ),
        Err(Error::Capacity)
    );
    let listing = s.state().listings[0].id;
    execute(&mut s, 1, Command::Cancel { listing }).unwrap();
    execute(
        &mut s,
        1,
        Command::List {
            description: Memo::new(),
            title: Title::try_from("Reuse closed slot").unwrap(),
            price: 1,
            side: Side::Sell,
            details: None,
        },
    )
    .unwrap();
    assert_eq!(s.state().listings.len(), LISTINGS);
}

#[test]
fn malformed_frames_and_out_of_order_events_fail_closed() {
    let (_, memory) = household();
    memory.0.borrow_mut()[0][30] ^= 1;
    assert!(matches!(Service::open(memory), Err(Error::CorruptJournal)));
    let (_, memory) = household();
    memory.0.borrow_mut().swap(0, 1);
    assert!(matches!(Service::open(memory), Err(Error::CorruptJournal)));
}

#[test]
fn api_uses_typed_commands_and_never_serializes_credentials() {
    let (mut s, _) = household();
    let auth = common::login(&mut s);
    let mut output = vec![0; api::RESPONSE_LIMIT];
    let body = br#"{"request_id":3,"command":{"issue":{"to":2,"amount":8,"memo":"chores"}}}"#;
    assert_eq!(
        api::handle(&mut s, "POST", "/api/v1/commands", &auth, body, &mut output).0,
        200
    );
    let (status, len) = api::handle(&mut s, "GET", "/api/v1/state", &auth, b"", &mut output);
    assert_eq!(status, 200);
    let json = std::str::from_utf8(&output[..len]).unwrap();
    assert!(!json.contains("token_hash"));
    assert!(!json.contains("last_command"));
    assert!(!json.contains(TOKEN));
    assert_eq!(
        api::handle(&mut s, "GET", "/api/v1/state", "", b"", &mut output).0,
        401
    );
    for bad in [
        br#"{"request_id":4,"command":{"issue":{"to":2,"amount":8.5,"memo":"x"}}}"#.as_slice(),
        br#"{"request_id":4,"command":{"wat":{}}}"#,
        br#"{"request_id":4,"command":{"issue":{"to":2,"amount":8,"memo":"x","extra":1}}}"#,
    ] {
        assert_eq!(
            api::handle(&mut s, "POST", "/api/v1/commands", &auth, bad, &mut output).0,
            400
        );
    }
}

#[test]
fn all_records_fit_and_storage_full_is_explicit() {
    let (mut s, memory) = household();
    for _ in 2..MAX_RECORDS {
        execute(&mut s, 1, issue(1)).unwrap();
    }
    assert_eq!(execute(&mut s, 1, issue(1)), Err(Error::Capacity));
    assert_eq!(memory.0.borrow().len(), MAX_RECORDS);
    let reopened = Service::open(memory).unwrap();
    assert_eq!(reopened.state().sequence, MAX_RECORDS as u64);
}

#[test]
fn escaped_unicode_text_round_trips() {
    let (mut s, memory) = household();
    let memo = Memo::try_from("Nana says \"merci\"\n日本語 🍪").unwrap();
    execute(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(2),
            amount: 1,
            memo: memo.clone(),
        },
    )
    .unwrap();
    let restored = Service::open(memory).unwrap();
    assert_eq!(restored.state().history[0].memo, memo);
}

#[test]
fn surrogate_pairs_decode_but_escaped_backslashes_stay_literal() {
    let (mut s, _) = household();
    let mut output = vec![0; api::RESPONSE_LIMIT];
    let auth = common::login(&mut s);
    for (request_id, encoded, expected) in [
        (3, r"\ud83c\udf6a", "🍪"),
        (4, r"\\ud83c\\udf6a", r"\ud83c\udf6a"),
        (5, r"\u65e5\u672c", "日本"),
    ] {
        let body = format!(
            r#"{{"request_id":{request_id},"command":{{"issue":{{"to":2,"amount":1,"memo":"{encoded}"}}}}}}"#
        );
        assert_eq!(
            api::handle(
                &mut s,
                "POST",
                "/api/v1/commands",
                &auth,
                body.as_bytes(),
                &mut output
            )
            .0,
            200
        );
        assert_eq!(s.state().history.back().unwrap().memo, expected);
    }
    for encoded in [r"\ud83c", r"\udf6a", r"\ud83c\u0061", r"\uZZZZ"] {
        let body = format!(
            r#"{{"request_id":6,"command":{{"issue":{{"to":2,"amount":1,"memo":"{encoded}"}}}}}}"#
        );
        assert_eq!(
            api::handle(
                &mut s,
                "POST",
                "/api/v1/commands",
                &auth,
                body.as_bytes(),
                &mut output
            )
            .0,
            400
        );
    }
}

#[test]
fn full_state_with_worst_case_json_escaping_fits_response_budget() {
    let (mut s, _) = household();
    for i in 2..MEMBERS {
        let name = format!("{}{}", "\u{1}".repeat(38), char::from(b'A' + i as u8));
        execute(
            &mut s,
            1,
            Command::AddMember {
                name: Name::try_from(name.as_str()).unwrap(),
                token_hash: [i as u8; 32],
            },
        )
        .unwrap();
    }
    for _ in 0..LISTINGS {
        execute(
            &mut s,
            1,
            Command::List {
                description: Memo::new(),
                title: Title::try_from("\u{1}".repeat(80).as_str()).unwrap(),
                price: MAX_AMOUNT,
                side: Side::Sell,
                details: None,
            },
        )
        .unwrap();
    }
    let listing_ids: Vec<_> = s.state().listings.iter().map(|l| l.id).collect();
    for listing in listing_ids {
        execute(
            &mut s,
            1,
            Command::UpdateListing {
                listing,
                title: None,
                description: Some(Memo::try_from("\u{1}".repeat(96).as_str()).unwrap()),
                price: None,
            },
        )
        .unwrap();
    }
    for index in 0..HISTORY {
        execute(
            &mut s,
            1,
            if index % 2 == 0 {
                Command::Issue {
                    to: MemberId(2),
                    amount: MAX_AMOUNT,
                    memo: Memo::try_from("\u{1}".repeat(96).as_str()).unwrap(),
                }
            } else {
                Command::Retire {
                    from: MemberId(2),
                    amount: MAX_AMOUNT,
                    memo: Memo::try_from("\u{1}".repeat(96).as_str()).unwrap(),
                }
            },
        )
        .unwrap();
    }
    let mut output = vec![0; api::RESPONSE_LIMIT];
    let auth = common::login(&mut s);
    let (status, size) = api::handle(&mut s, "GET", "/api/v1/state", &auth, b"", &mut output);
    assert_eq!(status, 200);
    assert!(size < api::RESPONSE_LIMIT);
}

#[test]
fn balances_cannot_exceed_browser_exact_integer_range() {
    let (s, _) = household();
    let mut state = State::default();
    state.members = s.state().members.clone();
    state.members[1].balance = MAX_SEQUENCE as i64;
    state.issuance_balance = -(MAX_SEQUENCE as i64);
    assert_eq!(state.validate(MemberId(1), &issue(1)), Err(Error::Overflow));
    state.check_invariants().unwrap();
}

#[test]
fn ambiguous_commit_recovers_once_and_retry_returns_original_receipt() {
    struct Ambiguous(Memory);
    impl Journal for Ambiguous {
        fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
            self.0.read(index, frame)
        }
        fn append(&mut self, index: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
            self.0.append(index, frame)?;
            Err(Error::Storage) // durable data, but acknowledgement was lost
        }
    }
    let (_, memory) = household();
    let mut s = Service::open(Ambiguous(memory.clone())).unwrap();
    assert_eq!(s.execute(MemberId(1), 3, issue(10)), Err(Error::Storage));
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 0);
    let mut output = vec![0; api::RESPONSE_LIMIT];
    assert_eq!(
        api::handle(
            &mut s,
            "GET",
            "/api/v1/state",
            &format!("Bearer {TOKEN}"),
            b"",
            &mut output
        )
        .0,
        503
    );
    let mut restored = Service::open(memory).unwrap();
    assert_eq!(restored.state().member(MemberId(2)).unwrap().balance, 10);
    assert_eq!(
        restored.execute(MemberId(1), 3, issue(10)),
        Ok(Receipt {
            sequence: 3,
            replayed: true
        })
    );
    assert_eq!(restored.state().member(MemberId(2)).unwrap().balance, 10);
}

#[test]
fn binary_event_fits_without_json_escape_expansion() {
    let (mut s, _) = household();
    assert!(execute(
        &mut s,
        1,
        Command::List {
            title: Title::try_from("\u{1}".repeat(80).as_str()).unwrap(),
            description: Memo::try_from("\u{1}".repeat(96).as_str()).unwrap(),
            price: 1,
            side: Side::Sell,
            details: None,
        }
    )
    .is_ok());
    assert_eq!(s.state().listings.len(), 1);
}
