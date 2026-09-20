#![cfg(feature = "desktop")]
mod common;
use nanacoin::{
    domain::*,
    journal::{file::FileJournal, *},
};
use std::{fs::OpenOptions, io::Write};

#[test]
fn file_lock_replay_partial_tail_and_corruption() {
    let path =
        std::env::temp_dir().join(format!("nanacoin-rs-{}-journal-test", std::process::id()));
    let journal = FileJournal::open(&path).unwrap();
    assert!(
        FileJournal::open(&path).is_err(),
        "second writer must not open"
    );
    let mut service = Service::open(journal).unwrap();
    common::provision(&mut service);
    service
        .execute(
            MemberId(1),
            2,
            Command::Issue {
                to: MemberId(1),
                amount: 12,
                memo: Memo::new(),
            },
        )
        .unwrap();
    drop(service);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"partial final write")
        .unwrap();
    let service = Service::open(FileJournal::open(&path).unwrap()).unwrap();
    assert_eq!(service.state().member(MemberId(1)).unwrap().balance, 12);
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        2 * FRAME_SIZE as u64
    );
    drop(service);
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .write_all(b"BAD!")
        .unwrap();
    assert!(matches!(
        Service::open(FileJournal::open(&path).unwrap()),
        Err(Error::CorruptJournal)
    ));
    std::fs::remove_file(path).unwrap();
}
