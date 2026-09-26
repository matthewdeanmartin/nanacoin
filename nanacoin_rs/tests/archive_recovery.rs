mod common;
use nanacoin::{
    api,
    domain::*,
    journal::{archive::PAGE_BYTES, checkpoint::ROW_BYTES, *},
};
use std::{
    cell::RefCell,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    rc::Rc,
};

fn now() -> u64 {
    1_800_000_000
}
fn issue() -> Command {
    Command::Issue {
        to: MemberId(1),
        amount: 7,
        memo: Memo::new(),
    }
}
fn add<J: Journal>(s: &mut Service<J>) {
    let request = s.state().member(MemberId(1)).unwrap().last_request + 1;
    s.execute(MemberId(1), request, issue()).unwrap();
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nanacoin-archive-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("ledger")
    }
    fn open(&self) -> Service<file::FileJournal> {
        Service::open_with_clock(file::FileJournal::open(self.path()).unwrap(), now).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn page<J: Journal>(s: &mut Service<J>, path: &str) -> serde_json::Value {
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let (status, len) = api::handle(s, "GET", path, "", &[], &mut out);
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&out[..len]));
    serde_json::from_slice(&out[..len]).unwrap()
}

#[test]
fn file_archive_survives_many_rotations_reopens_and_pages_across_hot_cold_boundary() {
    let dir = Temp::new();
    let mut s = dir.open();
    common::provision(&mut s);
    let payments = HISTORY + 101;
    for n in 0..payments - 1 {
        add(&mut s);
        if n % 137 == 136 {
            s.checkpoint(MemberId(1)).unwrap();
            drop(s);
            s = dir.open();
        }
    }
    let key = format!("g{}:archive-retry", s.generation());
    let receipt = s.execute_keyed(MemberId(1), &key, issue()).unwrap();
    let before_tx = s.state().transactions;
    let history = serde_json::to_value(&s.state().history).unwrap();
    let totals = s.state().ledger.epochs[0].flows;
    assert_eq!(
        s.state().member(MemberId(1)).unwrap().balance,
        payments as i64 * 7
    );
    drop(s);
    s = dir.open();
    assert_eq!(s.state().transactions, before_tx);
    assert_eq!(serde_json::to_value(&s.state().history).unwrap(), history);
    assert_eq!(s.state().ledger.epochs[0].flows, totals);
    assert_eq!(
        s.execute_keyed(MemberId(1), &key, issue()).unwrap(),
        Receipt {
            replayed: true,
            ..receipt
        }
    );
    let mut path = "/api/v1/transactions?limit=13".to_owned();
    let mut ids = Vec::new();
    for _ in 0..payments {
        let p = page(&mut s, &path);
        for t in p["transactions"].as_array().unwrap() {
            ids.push(
                t["id"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("tx-")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap(),
            );
        }
        match p["next_cursor"].as_str() {
            Some(c) => path = format!("/api/v1/transactions?limit=13&cursor={c}"),
            None => break,
        }
    }
    assert_eq!(ids.len(), payments);
    assert!(
        ids.windows(2).all(|v| v[0] > v[1]),
        "duplicates or non-descending page boundary"
    );
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let s = dir.open();
    assert_eq!(s.state().ledger.epochs[0].flows, totals);
    assert_eq!(s.state().transactions, before_tx);
    assert_eq!(
        s.state().member(MemberId(1)).unwrap().balance,
        payments as i64 * 7
    );
    s.state().check_invariants().unwrap();
}

#[test]
fn committed_file_archive_corruption_fails_closed_and_reset_starts_fresh() {
    let dir = Temp::new();
    let mut s = dir.open();
    common::provision(&mut s);
    add(&mut s);
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut archive = std::fs::OpenOptions::new()
        .write(true)
        .open(dir.0.join("ledger.archive"))
        .unwrap();
    archive.seek(SeekFrom::Start(32)).unwrap();
    archive.write_all(&[255]).unwrap();
    archive.sync_all().unwrap();
    drop(archive);
    assert!(matches!(
        Service::open_with_clock(file::FileJournal::open(dir.path()).unwrap(), now),
        Err(Error::CorruptJournal)
    ));

    let other = Temp::new();
    let mut s = other.open();
    common::provision(&mut s);
    add(&mut s);
    s.checkpoint(MemberId(1)).unwrap();
    let old_incarnation = s.state().archive.incarnation;
    s.reset_economy(MemberId(1)).unwrap();
    drop(s);
    let mut s = other.open();
    assert!(s.state().history.is_empty());
    assert!(s.state().ledger.audit.is_empty());
    assert_eq!(s.state().transactions, 0);
    assert_ne!(s.state().archive.incarnation, old_incarnation);
    common::provision(&mut s);
    add(&mut s);
    s.checkpoint(MemberId(1)).unwrap();
    drop(s);
    let mut s = other.open();
    assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 7);
    assert_eq!(
        page(&mut s, "/api/v1/transactions")["transactions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[derive(Default)]
struct Disk {
    generation: u64,
    logs: [Vec<[u8; FRAME_SIZE]>; 2],
    rows: [Vec<Vec<u8>>; 2],
    pages: std::collections::BTreeMap<usize, Vec<u8>>,
    fail_before_head: bool,
    fail_after_head: bool,
}
#[derive(Clone, Default)]
struct Memory(Rc<RefCell<Disk>>);
impl Journal for Memory {
    fn read(&mut self, i: usize, out: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        let d = self.0.borrow();
        if let Some(v) = d.logs[d.generation as usize % 2].get(i) {
            *out = *v;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, i: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        let bank = d.generation as usize % 2;
        assert_eq!(d.logs[bank].len(), i);
        d.logs[bank].push(*frame);
        Ok(())
    }
    fn generation(&self) -> u64 {
        self.0.borrow().generation
    }
    fn supports_checkpoint(&self) -> bool {
        true
    }
    fn checkpoint_rows(&self) -> usize {
        let d = self.0.borrow();
        d.rows[d.generation as usize % 2].len()
    }
    fn read_checkpoint(&mut self, i: usize, out: &mut [u8; ROW_BYTES]) -> Result<usize, Error> {
        let d = self.0.borrow();
        let v = &d.rows[d.generation as usize % 2][i];
        out[..v.len()].copy_from_slice(v);
        Ok(v.len())
    }
    fn begin_checkpoint(&mut self) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        let bank = 1 - d.generation as usize % 2;
        d.logs[bank].clear();
        d.rows[bank].clear();
        Ok(())
    }
    fn write_checkpoint(&mut self, i: usize, bytes: &[u8]) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        let bank = 1 - d.generation as usize % 2;
        assert_eq!(d.rows[bank].len(), i);
        d.rows[bank].push(bytes.to_vec());
        Ok(())
    }
    fn commit_checkpoint(&mut self, rows: usize) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        if d.fail_before_head {
            return Err(Error::Storage);
        }
        d.generation += 1;
        let bank = d.generation as usize % 2;
        assert_eq!(d.rows[bank].len(), rows);
        if d.fail_after_head {
            return Err(Error::Storage);
        }
        d.logs[1 - bank].clear();
        d.rows[1 - bank].clear();
        Ok(())
    }
    fn supports_archive(&self) -> bool {
        true
    }
    fn read_archive(&mut self, slot: usize, out: &mut [u8; PAGE_BYTES]) -> Result<usize, Error> {
        let d = self.0.borrow();
        let v = d.pages.get(&slot).ok_or(Error::CorruptJournal)?;
        out[..v.len()].copy_from_slice(v);
        Ok(v.len())
    }
    fn write_archive(&mut self, slot: usize, bytes: &[u8]) -> Result<(), Error> {
        self.0.borrow_mut().pages.insert(slot, bytes.to_vec());
        Ok(())
    }
}

#[test]
fn archive_before_manifest_and_ambiguous_manifest_both_recover_exactly_once() {
    for after_head in [false, true] {
        let disk = Memory::default();
        let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
        common::provision(&mut s);
        add(&mut s);
        s.checkpoint(MemberId(1)).unwrap();
        for _ in 0..25 {
            add(&mut s);
        }
        let key = format!("g{}:durable-retry", s.generation());
        let receipt = s.execute_keyed(MemberId(1), &key, issue()).unwrap();
        let before = serde_json::to_value(s.state()).unwrap();
        let flows = s.state().ledger.epochs[0].flows;
        let tx = s.state().transactions;
        {
            let mut d = disk.0.borrow_mut();
            d.fail_after_head = after_head;
            d.fail_before_head = !after_head;
        }
        assert_eq!(s.checkpoint(MemberId(1)), Err(Error::Storage));
        assert!(s.storage_failed());
        drop(s);
        {
            let mut d = disk.0.borrow_mut();
            d.fail_after_head = false;
            d.fail_before_head = false;
        }
        let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
        assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
        assert_eq!(s.state().ledger.epochs[0].flows, flows);
        assert_eq!(s.state().transactions, tx);
        assert_eq!(
            s.execute_keyed(MemberId(1), &key, issue()).unwrap(),
            Receipt {
                replayed: true,
                ..receipt
            }
        );
        s.checkpoint(MemberId(1)).unwrap();
        drop(s);
        let s = Service::open_with_clock(disk, now).unwrap();
        assert_eq!(s.state().ledger.epochs[0].flows, flows);
        assert_eq!(s.state().transactions, tx);
        s.state().check_invariants().unwrap();
    }
}

#[test]
fn retained_floor_paging_keeps_exact_lifetime_totals_and_snapshot_excludes_new_payments() {
    let disk = Memory::default();
    let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
    common::provision(&mut s);
    for _ in 0..850 {
        add(&mut s);
        s.checkpoint(MemberId(1)).unwrap();
    }
    let floor = s.state().archive.first_page;
    assert!(floor > 0);
    let totals = s.state().ledger.epochs[0].flows;
    drop(s);
    let mut s = Service::open_with_clock(disk, now).unwrap();
    assert_eq!(s.state().ledger.epochs[0].flows, totals);
    assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 850 * 7);
    assert_eq!(s.state().transactions, 850);
    let mut path = "/api/v1/transactions?limit=17".to_owned();
    let mut ids = Vec::new();
    let mut ended = false;
    for n in 0..300 {
        let p = page(&mut s, &path);
        assert_eq!(p["history_truncated"], true);
        assert_eq!(p["snapshot_upper"], 850);
        for t in p["transactions"].as_array().unwrap() {
            ids.push(
                t["id"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("tx-")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap(),
            );
        }
        if n == 0 {
            add(&mut s);
        }
        match p["next_cursor"].as_str() {
            Some(c) => path = format!("/api/v1/transactions?limit=17&cursor={c}"),
            None => {
                ended = true;
                break;
            }
        }
    }
    assert!(ended);
    assert_eq!(ids.len(), archive::RETAIN_PAGES as usize);
    assert!(ids.windows(2).all(|v| v[0] > v[1]));
    assert_eq!(s.state().member(MemberId(1)).unwrap().balance, 851 * 7);
    s.state().check_invariants().unwrap();
}

#[test]
fn audit_only_pruning_evicts_unrecoverable_payment_before_accepting_refund() {
    for automatic in [false, true] {
        let disk = Memory::default();
        let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
        common::provision(&mut s);
        add(&mut s);
        let execute = |s: &mut Service<Memory>, command: Command| {
            let request = s.state().member(MemberId(1)).unwrap().last_request + 1;
            s.execute(MemberId(1), request, command).unwrap()
        };
        execute(
            &mut s,
            Command::AddMember {
                name: "Alice".try_into().unwrap(),
                token_hash: [7; 32],
            },
        );
        let payment = execute(
            &mut s,
            Command::Transfer {
                to: MemberId(2),
                amount: 7,
                memo: Memo::new(),
            },
        )
        .sequence;
        s.checkpoint(MemberId(1)).unwrap();
        let update = || Command::UpdateMember {
            member: MemberId(1),
            display_name: Some("Nana".try_into().unwrap()),
            password: None,
            role: None,
            disabled: None,
            mastodon_id: None,
        };
        for _ in 0..if automatic { 767 } else { 800 } {
            execute(&mut s, update());
            s.checkpoint(MemberId(1)).unwrap();
        }
        if automatic {
            assert!(s.state().original_payment(payment).is_ok());
            for _ in 0..nanacoin::ledger::AUDIT_CACHE {
                execute(&mut s, update());
            }
            assert!(s.state().original_payment(payment).is_ok());
        }
        let request = s.state().member(MemberId(1)).unwrap().last_request + 1;
        let command = Command::Refund {
            transaction: payment,
            amount: 1,
            memo: Memo::new(),
        };
        let before = s.state().sequence;
        let generation = s.generation();
        assert_eq!(
            s.execute(MemberId(1), request, command.clone()),
            Err(Error::NotFound)
        );
        assert_eq!(s.state().sequence, before);
        assert_eq!(s.journal_records(), 0);
        assert_eq!(s.generation(), generation + u64::from(automatic));
        assert_eq!(s.state().archive.first_transaction, s.state().transactions);
        assert!(s.state().history.is_empty());
        drop(s);
        let mut s = Service::open_with_clock(disk, now).unwrap();
        assert!(s.state().history.is_empty());
        assert_eq!(
            s.execute(MemberId(1), request, command),
            Err(Error::NotFound)
        );
        assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 7);
    }
}
