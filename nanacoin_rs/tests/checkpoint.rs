mod common;
use nanacoin::{
    api,
    auth::PasswordVerifier,
    domain::*,
    journal::{checkpoint::ROW_BYTES, *},
    offers::*,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct Disk {
    generation: u64,
    logs: [Vec<[u8; FRAME_SIZE]>; 2],
    rows: [Vec<Vec<u8>>; 2],
}
#[derive(Clone, Default)]
struct Memory {
    disk: Rc<RefCell<Disk>>,
    fail_at: Rc<Cell<usize>>,
    step: Rc<Cell<usize>>,
}
impl Memory {
    fn step(&self) -> Result<(), Error> {
        let step = self.step.get() + 1;
        self.step.set(step);
        if self.fail_at.get() == step {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}
impl Journal for Memory {
    fn read(&mut self, n: usize, out: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        let d = self.disk.borrow();
        if let Some(row) = d.logs[d.generation as usize % 2].get(n) {
            *out = *row;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, n: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        let mut d = self.disk.borrow_mut();
        let b = d.generation as usize % 2;
        assert_eq!(n, d.logs[b].len());
        d.logs[b].push(*frame);
        Ok(())
    }
    fn generation(&self) -> u64 {
        self.disk.borrow().generation
    }
    fn supports_checkpoint(&self) -> bool {
        true
    }
    fn checkpoint_rows(&self) -> usize {
        let d = self.disk.borrow();
        d.rows[d.generation as usize % 2].len()
    }
    fn read_checkpoint(&mut self, n: usize, out: &mut [u8; ROW_BYTES]) -> Result<usize, Error> {
        let d = self.disk.borrow();
        let row = &d.rows[d.generation as usize % 2][n];
        out[..row.len()].copy_from_slice(row);
        Ok(row.len())
    }
    fn begin_checkpoint(&mut self) -> Result<(), Error> {
        self.step()?;
        let mut d = self.disk.borrow_mut();
        let b = 1 - d.generation as usize % 2;
        d.logs[b].clear();
        d.rows[b].clear();
        Ok(())
    }
    fn write_checkpoint(&mut self, _: usize, row: &[u8]) -> Result<(), Error> {
        self.step()?;
        let mut d = self.disk.borrow_mut();
        let b = 1 - d.generation as usize % 2;
        d.rows[b].push(row.to_vec());
        Ok(())
    }
    fn commit_checkpoint(&mut self, rows: usize) -> Result<(), Error> {
        self.step()?; // before publication
        {
            let mut d = self.disk.borrow_mut();
            d.generation += 1;
            assert_eq!(d.rows[d.generation as usize % 2].len(), rows);
        }
        self.step()?; // lost acknowledgement, but publication survived
        let mut d = self.disk.borrow_mut();
        let old = 1 - d.generation as usize % 2;
        d.logs[old].clear();
        d.rows[old].clear();
        Ok(())
    }
}
fn now() -> u64 {
    1_700_000_000
}
fn exec<J: Journal>(s: &mut Service<J>, actor: u8, cmd: Command) -> Receipt {
    let id = s.state().member(MemberId(actor)).unwrap().last_request + 1;
    s.execute(MemberId(actor), id, cmd).unwrap()
}
fn issue() -> Command {
    Command::Issue {
        to: MemberId(1),
        amount: 1,
        memo: Memo::new(),
    }
}

#[test]
fn checkpoint_preserves_passwords_balances_open_deals_deadlines_and_retries() {
    let disk = Memory::default();
    let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
    common::provision(&mut s);
    exec(
        &mut s,
        1,
        Command::CreateMember {
            username: "alice".try_into().unwrap(),
            display_name: "Alice 🍪".try_into().unwrap(),
            password: PasswordVerifier::hash("1234").unwrap(),
            role: Role::User,
            grant: 100,
        },
    );
    exec(
        &mut s,
        1,
        Command::IssueUsd {
            to: MemberId(2),
            cents: 1234,
            memo: Memo::new(),
        },
    );
    let listing = exec(
        &mut s,
        1,
        Command::List {
            title: "Cookies".try_into().unwrap(),
            description: "Fresh 🍪".try_into().unwrap(),
            price: 10,
            side: Side::Sell,
            details: None,
        },
    )
    .sequence;
    let offer = OfferId(
        exec(
            &mut s,
            2,
            Command::MakeOffer {
                listing,
                amount: 8,
                message: "Tomorrow".try_into().unwrap(),
            },
        )
        .sequence,
    );
    let accepted = s
        .execute_keyed(MemberId(1), "legacy-accept", Command::AcceptOffer { offer })
        .unwrap();
    let deadline = s
        .state()
        .offer(offer)
        .unwrap()
        .settlement()
        .unwrap()
        .settles_at;
    exec(
        &mut s,
        2,
        Command::PostQuote {
            side: nanacoin::forex::QuoteSide::ASK,
            cents_per_coin: 10,
            coins: 2,
            expires_at: now() + 3600,
        },
    );
    let before = serde_json::to_value(s.state()).unwrap();
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut s = Service::open_with_clock(disk, now).unwrap();
    assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
    assert_eq!(
        s.state()
            .offer(offer)
            .unwrap()
            .settlement()
            .unwrap()
            .settles_at,
        deadline
    );
    assert_eq!(
        s.execute_keyed(MemberId(1), "legacy-accept", Command::AcceptOffer { offer })
            .unwrap(),
        Receipt {
            replayed: true,
            ..accepted
        }
    );
    assert_eq!(
        s.execute_keyed(MemberId(1), "unknown-legacy", issue()),
        Err(Error::StaleRequest)
    );
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "alice", "1234");
    assert!(!nana.is_empty() && !alice.is_empty());
    s.execute_keyed(
        MemberId(2),
        "g1:undo",
        Command::UnacceptOffer {
            offer,
            reason: "Changed plans".try_into().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 100);
    s.state().check_invariants().unwrap();
}

#[test]
fn grows_beyond_old_limit_with_bounded_storage_and_disjoint_cash_ids() {
    let disk = Memory::default();
    let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
    common::provision(&mut s);
    for n in 0..9000 {
        let key = format!("g{}:payment-{n}", s.generation());
        s.execute_keyed(MemberId(1), &key, issue()).unwrap();
        assert!(s.journal_records() <= 2048);
    }
    assert!(s.state().sequence > 8192);
    assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 9000);
    assert_eq!(s.state().history.len(), HISTORY);
    assert_eq!(
        s.execute_keyed(MemberId(1), "g0:payment-0", issue()),
        Err(Error::StaleRequest)
    );
    drop(s);
    let s = Service::open_with_clock(disk, now).unwrap();
    assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 9000);
    for t in &s.state().history {
        assert!((1..=4096).contains(&(t.id % 8192)));
    }
    assert_eq!(next_sequence(4096), Some(8193));
    assert_eq!(next_sequence(12288), Some(16385));
}

#[test]
fn power_loss_at_each_checkpoint_boundary_recovers_a_complete_generation() {
    for reset in [false, true] {
        // Small fixture has header, member, history, key: exercise every
        // row boundary, pre-publication and post-publication lost response.
        for fail in 1..=7 {
            let disk = Memory::default();
            let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
            common::provision(&mut s);
            s.execute_keyed(MemberId(1), "pay", issue()).unwrap();
            disk.fail_at.set(fail);
            let result = if reset {
                s.reset_economy(MemberId(1))
            } else {
                s.checkpoint(MemberId(1))
            };
            if result.is_err() {
                assert!(s.storage_failed());
            }
            drop(s);
            disk.fail_at.set(0);
            let s = Service::open_with_clock(disk.clone(), now).unwrap();
            if reset && disk.generation() > 0 {
                assert!(s.state().members.is_empty());
            } else {
                assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 1);
            }
            s.state().check_invariants().unwrap();
        }
    }
}

#[test]
fn reset_requires_nana_confirmation_and_fresh_state_and_revokes_sessions() {
    let disk = Memory::default();
    let mut s = Service::open(disk.clone()).unwrap();
    common::provision(&mut s);
    exec(
        &mut s,
        1,
        Command::CreateMember {
            username: "alice".try_into().unwrap(),
            display_name: "Alice".try_into().unwrap(),
            password: PasswordVerifier::hash("1234").unwrap(),
            role: Role::User,
            grant: 100,
        },
    );
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "alice", "1234");
    let body = serde_json::json!({"expected_generation":0,"expected_sequence":s.state().sequence,"confirmation":"RESET ECONOMY"});
    assert_eq!(
        common::call(&mut s, "/api/v1/admin/reset", "", body.clone()).0,
        401
    );
    assert_eq!(
        common::call(&mut s, "/api/v1/admin/reset", &alice, body.clone()).0,
        403
    );
    let mut bad = body.clone();
    bad["confirmation"] = "reset".into();
    assert_eq!(
        common::call(&mut s, "/api/v1/admin/reset", &nana, bad).0,
        400
    );
    let mut stale = body.clone();
    stale["expected_sequence"] = 0.into();
    assert_eq!(
        common::call(&mut s, "/api/v1/admin/reset", &nana, stale).0,
        409
    );
    assert_eq!(
        common::call(&mut s, "/api/v1/admin/reset", &nana, body.clone()).0,
        200
    );
    assert!(s.state().members.is_empty());
    assert_eq!(
        common::call(&mut s, "/api/v1/admin/reset", &nana, body).0,
        401
    );
    drop(s);
    let mut s = Service::open(disk).unwrap();
    assert!(s.state().members.is_empty());
    common::provision(&mut s);
    assert_eq!(s.generation(), 1);
    assert_eq!(s.state().sequence, 1);
    assert_eq!(
        s.execute_keyed(MemberId(1), "pay", issue()),
        Err(Error::StaleRequest)
    );
    let mut output = vec![0; api::RESPONSE_LIMIT];
    assert_eq!(
        api::handle(&mut s, "GET", "/api/v1/state", &nana, b"", &mut output).0,
        401
    );
}

#[test]
fn corrupt_checkpoint_does_not_fall_back_to_retired_history() {
    let disk = Memory::default();
    let mut s = Service::open(disk.clone()).unwrap();
    common::provision(&mut s);
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    disk.disk.borrow_mut().rows[1][1][20] ^= 1;
    assert!(matches!(Service::open(disk), Err(Error::CorruptJournal)));
}

#[cfg(feature = "desktop")]
#[test]
fn desktop_generations_restart_and_reset() {
    use nanacoin::journal::file::FileJournal;
    let dir = std::env::temp_dir().join(format!("nanacoin-checkpoint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("economy.journal");
    let mut s = Service::open(FileJournal::open(&path).unwrap()).unwrap();
    common::provision(&mut s);
    s.execute_keyed(MemberId(1), "pay", issue()).unwrap();
    for generation in 1..=3 {
        s.checkpoint(MemberId(1)).unwrap();
        drop(s);
        s = Service::open(FileJournal::open(&path).unwrap()).unwrap();
        assert_eq!(s.generation(), generation);
        assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 1);
        assert!(FileJournal::open(&path).is_err());
    }
    s.reset_economy(MemberId(1)).unwrap();
    drop(s);
    let s = Service::open(FileJournal::open(&path).unwrap()).unwrap();
    assert!(s.state().members.is_empty());
    drop(s);
    // Only files created in this test's explicitly named temporary directory.
    for entry in std::fs::read_dir(&dir).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}
