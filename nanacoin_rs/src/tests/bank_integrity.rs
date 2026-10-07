//! Verify corrupt record rejection through the shared invariant entry point.
use crate::{commerce, domain::*, fulfillment};

fn replay(s: &mut State, actor: u8, command: Command) {
    let sequence = s.sequence + 1;
    s.replay(&Event {
        version: 2,
        sequence,
        timestamp: sequence,
        actor: MemberId(actor),
        request_id: sequence,
        client_key: None,
        command,
    })
    .unwrap();
}

fn members() -> State {
    let mut s = State::default();
    for (name, hash) in [("Nana", 1), ("Alice", 2)] {
        replay(
            &mut s,
            1,
            Command::AddMember {
                name: name.try_into().unwrap(),
                token_hash: [hash; 32],
            },
        );
    }
    s
}

fn commerce_state() -> State {
    let mut s = members();
    replay(
        &mut s,
        2,
        Command::Commerce {
            action: commerce::Action::MintArt {
                title: "Sunrise".try_into().unwrap(),
                license: "Profile display".try_into().unwrap(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .try_into()
                    .unwrap(),
                locator: "https://example.org/sunrise.png".try_into().unwrap(),
            },
        },
    );
    replay(
        &mut s,
        2,
        Command::Commerce {
            action: commerce::Action::CreateRequest {
                title: "Supplies".try_into().unwrap(),
                description: Memo::new(),
                target: Some(100),
                deadline: None,
            },
        },
    );
    s.check_invariants().unwrap();
    s
}

#[test]
fn corrupt_commerce_records_fail_shared_integrity_checks() {
    let mutations: [fn(&mut State); 7] = [
        |s| s.commerce.requests[0].received = -1,
        |s| s.commerce.requests[0].target = Some(0),
        |s| s.commerce.requests.push(s.commerce.requests[0].clone()),
        |s| s.commerce.artworks[0].revision = s.sequence + 1,
        |s| s.commerce.artworks[0].locator = "https://user@example.org/art.png".try_into().unwrap(),
        |s| s.commerce.artworks.push(s.commerce.artworks[0].clone()),
        |s| {
            s.commerce.artworks[0].equipped = true;
            let mut art = s.commerce.artworks[0].clone();
            art.id = 1;
            s.commerce.artworks.push(art);
        },
    ];
    for mutate in mutations {
        let mut s = commerce_state();
        mutate(&mut s);
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
    }
    let mut s = commerce_state();
    s.commerce.requests[0].owner = MemberId(32);
    assert_eq!(s.check_invariants(), Err(Error::NotFound));
}

fn fulfillment_state() -> State {
    let mut s = members();
    replay(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(1),
            amount: 10,
            memo: Memo::new(),
        },
    );
    replay(
        &mut s,
        1,
        Command::Transfer {
            to: MemberId(2),
            amount: 1,
            memo: Memo::new(),
        },
    );
    let payment = s.history.back().unwrap().clone();
    let mut updates = heapless::Vec::new();
    updates
        .push(fulfillment::Update {
            sequence: s.sequence,
            at: s.last_timestamp,
            actor: MemberId(1),
            status: fulfillment::Status::Todo,
            reason: Memo::new(),
        })
        .unwrap();
    s.fulfillments.push(fulfillment::Fulfillment {
        transaction: payment.id,
        payment,
        provider: MemberId(2),
        recipient: MemberId(1),
        description: TransactionMemo::new(),
        kind: fulfillment::Kind::Goods,
        status: fulfillment::Status::Todo,
        updates,
    });
    s.check_invariants().unwrap();
    s
}

#[test]
fn corrupt_fulfillment_links_and_updates_fail_shared_integrity_checks() {
    let mutations: [fn(&mut State); 7] = [
        |s| s.fulfillments[0].payment.to = MemberId(1),
        |s| s.fulfillments[0].payment.usd = true,
        |s| s.fulfillments[0].status = fulfillment::Status::Reversed,
        |s| s.fulfillments[0].updates.clear(),
        |s| s.fulfillments[0].updates[0].actor = MemberId(32),
        |s| s.fulfillments[0].updates[0].sequence = s.sequence + 1,
        |s| s.fulfillments.push(s.fulfillments[0].clone()),
    ];
    for mutate in mutations {
        let mut s = fulfillment_state();
        mutate(&mut s);
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
    }
}
