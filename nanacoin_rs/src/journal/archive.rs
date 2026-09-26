//! Immutable pages published only by the checkpoint's archive head.
use super::Journal;
use crate::{
    domain::{Error, State, Transaction, HISTORY},
    ledger::{Audit, AUDIT_CACHE},
};
use serde::{Deserialize, Serialize};

pub const PAGE_BYTES: usize = 4096;
pub const SLOTS: usize = 1024;
pub const RETAIN_PAGES: u64 = 768;
const HEADER: usize = 32;
const MAX_STAGE: u64 = SLOTS as u64 - RETAIN_PAGES;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveHead {
    pub first_page: u64,
    pub next_page: u64,
    pub transactions: u64,
    /// Transactions below this ordinal have been pruned from committed pages.
    pub first_transaction: u64,
    pub audit_sequence: u64,
    pub incarnation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ArchiveRecord {
    Transaction(Transaction),
    Audit(Audit),
}
#[derive(Serialize)]
enum RecordRef<'a> {
    Transaction(&'a Transaction),
    Audit(&'a Audit),
}

fn validate_head(h: &ArchiveHead) -> Result<(), Error> {
    if h.first_page > h.next_page
        || h.next_page - h.first_page > RETAIN_PAGES
        || h.first_transaction > h.transactions
    {
        return Err(Error::CorruptJournal);
    }
    Ok(())
}

struct Writer<'a, J> {
    journal: &'a mut J,
    old: &'a ArchiveHead,
    next: u64,
    page: [u8; PAGE_BYTES],
    used: usize,
    count: u32,
}
impl<J: Journal> Writer<'_, J> {
    fn flush(&mut self) -> Result<(), Error> {
        if self.count == 0 {
            return Ok(());
        }
        if self.next - self.old.next_page >= MAX_STAGE || self.next == u64::MAX {
            return Err(Error::Capacity);
        }
        let slot = self.next % SLOTS as u64;
        // A failed checkpoint must leave every previously published page intact.
        if (self.old.first_page..self.old.next_page).any(|p| p % SLOTS as u64 == slot) {
            return Err(Error::Capacity);
        }
        self.page[..4].copy_from_slice(b"NCA2");
        self.page[4..12].copy_from_slice(&self.next.to_le_bytes());
        self.page[12..20].copy_from_slice(&self.old.incarnation.to_le_bytes());
        self.page[20..24].copy_from_slice(&(self.used as u32).to_le_bytes());
        self.page[24..28].copy_from_slice(&self.count.to_le_bytes());
        let mut crc = crc32fast::Hasher::new();
        crc.update(&self.page[..28]);
        crc.update(&self.page[HEADER..self.used]);
        self.page[28..32].copy_from_slice(&crc.finalize().to_le_bytes());
        self.journal
            .write_archive(slot as usize, &self.page[..self.used])?;
        self.next += 1;
        self.used = HEADER;
        self.count = 0;
        self.page.fill(0);
        Ok(())
    }
    fn push(&mut self, record: RecordRef<'_>) -> Result<(), Error> {
        // Serialize directly into the page. A full page is sealed, then retry.
        let encoded = if self.used + 2 <= PAGE_BYTES {
            postcard::to_slice(&record, &mut self.page[self.used + 2..]).map(|v| v.len())
        } else {
            Err(postcard::Error::SerializeBufferFull)
        };
        let len = match encoded {
            Ok(len) => len,
            Err(postcard::Error::SerializeBufferFull) => {
                self.flush()?;
                postcard::to_slice(&record, &mut self.page[self.used + 2..])
                    .map_err(|_| Error::Capacity)?
                    .len()
            }
            Err(_) => return Err(Error::Capacity),
        };
        self.page[self.used..self.used + 2].copy_from_slice(&(len as u16).to_le_bytes());
        self.used += 2 + len;
        self.count += 1;
        Ok(())
    }
}

pub fn prepare<J: Journal>(j: &mut J, s: &State, old: &ArchiveHead) -> Result<ArchiveHead, Error> {
    validate_head(old)?;
    if !j.supports_archive() {
        return Err(Error::Storage);
    }
    if old.transactions > s.transactions || old.audit_sequence > s.sequence {
        return Err(Error::CorruptJournal);
    }
    preflight(s, old)?;
    let mut w = Writer {
        journal: j,
        old,
        next: old.next_page,
        page: [0; PAGE_BYTES],
        used: HEADER,
        count: 0,
    };
    let mut ordinal = old.transactions;
    for t in s
        .history
        .iter()
        .filter(|t| t.meta.ordinal >= old.transactions)
    {
        if t.meta.ordinal != ordinal {
            return Err(Error::Capacity);
        }
        w.push(RecordRef::Transaction(t))?;
        ordinal += 1;
    }
    if ordinal != s.transactions {
        return Err(Error::Capacity);
    }
    let mut audit_sequence = old.audit_sequence;
    for audit in s
        .ledger
        .audit
        .iter()
        .filter(|a| a.sequence > old.audit_sequence)
    {
        if crate::domain::next_sequence(audit_sequence) != Some(audit.sequence) {
            return Err(Error::Capacity);
        }
        w.push(RecordRef::Audit(audit))?;
        audit_sequence = audit.sequence;
    }
    if audit_sequence != s.sequence {
        return Err(Error::Capacity);
    }
    w.flush()?;
    let next_page = w.next;
    let first_page = old.first_page.max(next_page.saturating_sub(RETAIN_PAGES));
    let mut first_transaction = old.first_transaction;
    for page in old.first_page..first_page {
        scan_page(w.journal, old, page, |record| {
            if let ArchiveRecord::Transaction(t) = record {
                first_transaction = first_transaction.max(t.meta.ordinal + 1);
            }
            Ok(())
        })?;
    }
    Ok(ArchiveHead {
        first_page,
        next_page,
        first_transaction,
        transactions: s.transactions,
        audit_sequence: s.sequence,
        incarnation: old.incarnation,
    })
}

fn preflight(s: &State, old: &ArchiveHead) -> Result<(), Error> {
    let mut buffer = [0; PAGE_BYTES - HEADER - 2];
    let mut used = HEADER;
    let mut pages = 0u64;
    let mut count = |record: RecordRef<'_>| -> Result<(), Error> {
        let len = postcard::to_slice(&record, &mut buffer)
            .map_err(|_| Error::Capacity)?
            .len()
            + 2;
        if used + len > PAGE_BYTES {
            pages += 1;
            used = HEADER;
        }
        used += len;
        Ok(())
    };
    let mut ordinal = old.transactions;
    for t in s
        .history
        .iter()
        .filter(|t| t.meta.ordinal >= old.transactions)
    {
        if t.meta.ordinal != ordinal {
            return Err(Error::Capacity);
        }
        count(RecordRef::Transaction(t))?;
        ordinal += 1;
    }
    let mut sequence = old.audit_sequence;
    for a in s
        .ledger
        .audit
        .iter()
        .filter(|a| a.sequence > old.audit_sequence)
    {
        if crate::domain::next_sequence(sequence) != Some(a.sequence) {
            return Err(Error::Capacity);
        }
        a.validate_public()?;
        count(RecordRef::Audit(a))?;
        sequence = a.sequence;
    }
    if used > HEADER {
        pages += 1;
    }
    if ordinal != s.transactions
        || sequence != s.sequence
        || pages > MAX_STAGE
        || old.next_page.checked_add(pages).is_none()
    {
        return Err(Error::Capacity);
    }
    Ok(())
}

pub fn scan_page<J: Journal>(
    j: &mut J,
    h: &ArchiveHead,
    page: u64,
    mut visit: impl FnMut(ArchiveRecord) -> Result<(), Error>,
) -> Result<(), Error> {
    validate_head(h)?;
    if page < h.first_page || page >= h.next_page {
        return Err(Error::NotFound);
    }
    let mut bytes = [0; PAGE_BYTES];
    let used = j.read_archive((page % SLOTS as u64) as usize, &mut bytes)?;
    if !(HEADER..=PAGE_BYTES).contains(&used)
        || &bytes[..4] != b"NCA2"
        || u64::from_le_bytes(bytes[4..12].try_into().unwrap()) != page
        || u64::from_le_bytes(bytes[12..20].try_into().unwrap()) != h.incarnation
        || u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize != used
    {
        return Err(Error::CorruptJournal);
    }
    let mut crc = crc32fast::Hasher::new();
    crc.update(&bytes[..28]);
    crc.update(&bytes[HEADER..used]);
    if crc.finalize() != u32::from_le_bytes(bytes[28..32].try_into().unwrap()) {
        return Err(Error::CorruptJournal);
    }
    let count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    if count == 0 {
        return Err(Error::CorruptJournal);
    }
    let mut offset = HEADER;
    for _ in 0..count {
        if offset + 2 > used {
            return Err(Error::CorruptJournal);
        }
        let len = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        if len == 0 || len > used - offset {
            return Err(Error::CorruptJournal);
        }
        let (record, remaining): (ArchiveRecord, _) =
            postcard::take_from_bytes(&bytes[offset..offset + len])
                .map_err(|_| Error::CorruptJournal)?;
        if !remaining.is_empty() {
            return Err(Error::CorruptJournal);
        }
        match &record {
            ArchiveRecord::Transaction(t)
                if t.meta.ordinal >= h.transactions || t.meta.ordinal < h.first_transaction =>
            {
                return Err(Error::CorruptJournal)
            }
            ArchiveRecord::Audit(a) if a.sequence > h.audit_sequence => {
                return Err(Error::CorruptJournal)
            }
            _ => {}
        }
        if let ArchiveRecord::Audit(a) = &record {
            a.validate_public()?;
        }
        visit(record)?;
        offset += len;
    }
    if offset != used {
        return Err(Error::CorruptJournal);
    }
    Ok(())
}
pub fn scan<J: Journal>(
    j: &mut J,
    h: &ArchiveHead,
    mut visit: impl FnMut(ArchiveRecord) -> Result<(), Error>,
) -> Result<(), Error> {
    validate_head(h)?;
    for page in h.first_page..h.next_page {
        scan_page(j, h, page, &mut visit)?;
    }
    Ok(())
}
pub fn restore_history<J: Journal>(j: &mut J, h: &ArchiveHead, s: &mut State) -> Result<(), Error> {
    s.history.clear();
    s.ledger.audit.clear();
    let mut last_tx = None;
    let mut last_audit = None;
    scan(j, h, |record| {
        match record {
            ArchiveRecord::Transaction(t) => {
                if last_tx.is_none() && t.meta.ordinal != h.first_transaction {
                    return Err(Error::CorruptJournal);
                }
                if last_tx.is_some_and(|last| t.meta.ordinal != last + 1) {
                    return Err(Error::CorruptJournal);
                }
                last_tx = Some(t.meta.ordinal);
                if s.history.len() == HISTORY {
                    s.history.pop_front();
                }
                s.history.push_back(t);
            }
            ArchiveRecord::Audit(a) => {
                if last_audit.is_none() && h.first_page == 0 && a.sequence != 1 {
                    return Err(Error::CorruptJournal);
                }
                if last_audit
                    .is_some_and(|last| crate::domain::next_sequence(last) != Some(a.sequence))
                {
                    return Err(Error::CorruptJournal);
                }
                last_audit = Some(a.sequence);
                if s.ledger.audit.len() == AUDIT_CACHE {
                    s.ledger.audit.pop_front();
                }
                s.ledger.audit.push_back(a);
            }
        }
        Ok(())
    })?;
    if last_tx.is_some_and(|last| last + 1 != h.transactions)
        || (h.transactions > h.first_transaction && last_tx.is_none())
        || (h.audit_sequence > 0 && last_audit != Some(h.audit_sequence))
    {
        return Err(Error::CorruptJournal);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{Command, MemberId},
        journal::FRAME_SIZE,
        ledger::AuditAction,
    };
    #[derive(Default)]
    struct Memory {
        pages: std::collections::BTreeMap<usize, std::vec::Vec<u8>>,
        writes: usize,
        fail_at: Option<usize>,
    }
    impl Journal for Memory {
        fn read(&mut self, _: usize, _: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
            Ok(false)
        }
        fn append(&mut self, _: usize, _: &[u8; FRAME_SIZE]) -> Result<(), Error> {
            Ok(())
        }
        fn supports_archive(&self) -> bool {
            true
        }
        fn read_archive(
            &mut self,
            slot: usize,
            out: &mut [u8; PAGE_BYTES],
        ) -> Result<usize, Error> {
            let p = self.pages.get(&slot).ok_or(Error::CorruptJournal)?;
            out[..p.len()].copy_from_slice(p);
            Ok(p.len())
        }
        fn write_archive(&mut self, slot: usize, bytes: &[u8]) -> Result<(), Error> {
            self.writes += 1;
            self.pages.insert(slot, bytes.to_vec());
            if self.fail_at == Some(self.writes) {
                Err(Error::Storage)
            } else {
                Ok(())
            }
        }
    }
    fn audit(sequence: u64) -> Audit {
        Audit {
            sequence,
            actor: MemberId(1),
            at: sequence,
            action: AuditAction::Business(Command::BuyTickets { lotto: 1, count: 1 }),
        }
    }
    fn append_page(j: &mut Memory, h: ArchiveHead) -> ArchiveHead {
        let a = audit(h.audit_sequence + 1);
        let mut w = Writer {
            journal: j,
            old: &h,
            next: h.next_page,
            page: [0; PAGE_BYTES],
            used: HEADER,
            count: 0,
        };
        w.push(RecordRef::Audit(&a)).unwrap();
        w.flush().unwrap();
        ArchiveHead {
            first_page: h.first_page.max(w.next.saturating_sub(RETAIN_PAGES)),
            next_page: w.next,
            audit_sequence: a.sequence,
            ..h
        }
    }
    #[test]
    fn full_ring_staging_keeps_every_committed_page_after_ambiguous_failure() {
        let mut j = Memory::default();
        let mut h = ArchiveHead::default();
        for _ in 0..1100 {
            h = append_page(&mut j, h);
        }
        let committed = j.pages.clone();
        j.fail_at = Some(j.writes + 2);
        let a = audit(h.audit_sequence + 1);
        let mut w = Writer {
            journal: &mut j,
            old: &h,
            next: h.next_page,
            page: [0; PAGE_BYTES],
            used: HEADER,
            count: 0,
        };
        w.push(RecordRef::Audit(&a)).unwrap();
        w.flush().unwrap();
        w.push(RecordRef::Audit(&a)).unwrap();
        assert_eq!(w.flush(), Err(Error::Storage));
        for p in h.first_page..h.next_page {
            let slot = (p % SLOTS as u64) as usize;
            assert_eq!(j.pages[&slot], committed[&slot]);
        }
        let mut count = 0;
        scan(&mut j, &h, |_| {
            count += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(count, RETAIN_PAGES);
        j.fail_at = None;
        let next = append_page(&mut j, h);
        assert_eq!(next.next_page, h.next_page + 1);
    }
    #[test]
    fn staged_page_limit_rejects_live_slot_overwrite() {
        let mut j = Memory::default();
        let mut h = ArchiveHead::default();
        for _ in 0..RETAIN_PAGES {
            h = append_page(&mut j, h);
        }
        let a = audit(h.audit_sequence + 1);
        let mut w = Writer {
            journal: &mut j,
            old: &h,
            next: h.next_page,
            page: [0; PAGE_BYTES],
            used: HEADER,
            count: 0,
        };
        for _ in 0..MAX_STAGE {
            w.push(RecordRef::Audit(&a)).unwrap();
            w.flush().unwrap();
        }
        w.push(RecordRef::Audit(&a)).unwrap();
        assert_eq!(w.flush(), Err(Error::Capacity));
        scan(&mut j, &h, |_| Ok(())).unwrap();
    }
    #[test]
    fn uncommitted_orphans_are_invisible_and_page_identity_is_checked() {
        let mut j = Memory::default();
        let empty = ArchiveHead::default();
        let h = append_page(&mut j, empty);
        scan(&mut j, &empty, |_| panic!("orphan must remain invisible")).unwrap();
        let wrong = ArchiveHead {
            incarnation: 1,
            ..h
        };
        assert_eq!(scan(&mut j, &wrong, |_| Ok(())), Err(Error::CorruptJournal));
        j.pages.get_mut(&0).unwrap()[32] ^= 1;
        assert_eq!(scan(&mut j, &h, |_| Ok(())), Err(Error::CorruptJournal));
    }
    #[test]
    fn valid_crc_does_not_hide_missing_tail_or_credential_bearing_audit() {
        let mut j = Memory::default();
        let h = append_page(&mut j, ArchiveHead::default());
        let mut s = State::default();
        let missing = ArchiveHead {
            audit_sequence: 2,
            ..h
        };
        assert_eq!(
            restore_history(&mut j, &missing, &mut s),
            Err(Error::CorruptJournal)
        );
        let missing = ArchiveHead {
            transactions: 1,
            ..h
        };
        assert_eq!(
            restore_history(&mut j, &missing, &mut s),
            Err(Error::CorruptJournal)
        );
        let empty = ArchiveHead::default();
        let secret = Audit {
            sequence: 1,
            actor: MemberId(1),
            at: 1,
            action: AuditAction::Business(Command::AddMember {
                name: "Nana".try_into().unwrap(),
                token_hash: [123; 32],
            }),
        };
        let mut w = Writer {
            journal: &mut j,
            old: &empty,
            next: 0,
            page: [0; PAGE_BYTES],
            used: HEADER,
            count: 0,
        };
        w.push(RecordRef::Audit(&secret)).unwrap();
        w.flush().unwrap();
        assert_eq!(
            scan(&mut j, &h, |_| panic!(
                "credential-bearing audit must not reach caller"
            )),
            Err(Error::CorruptJournal)
        );
    }
    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn missing_unarchived_history_or_audit_fails_instead_of_losing_records() {
        let mut j = Memory::default();
        let mut s = State::default();
        s.transactions = 1;
        assert_eq!(
            prepare(&mut j, &s, &ArchiveHead::default()),
            Err(Error::Capacity)
        );
        s.transactions = 0;
        s.sequence = 1;
        assert_eq!(
            prepare(&mut j, &s, &ArchiveHead::default()),
            Err(Error::Capacity)
        );
        s.ledger.audit.push_back(audit(1));
        let h = prepare(&mut j, &s, &ArchiveHead::default()).unwrap();
        restore_history(&mut j, &h, &mut s).unwrap();
        assert_eq!(s.ledger.audit.len(), 1);
    }
}
