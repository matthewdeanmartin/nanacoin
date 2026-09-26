use crate::domain::{Command, Error, Event, MemberId, Receipt, State};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub mod archive;
pub mod checkpoint;

pub const FRAME_SIZE: usize = 1024;
pub const MAX_RECORDS: usize = 4096;

/// A successful append means durable storage. An error may be ambiguous;
/// Service latches read-only until restart/replay instead of reusing the slot.
pub trait Journal {
    fn supports_archive(&self) -> bool {
        false
    }
    fn read_archive(
        &mut self,
        _: usize,
        _: &mut [u8; archive::PAGE_BYTES],
    ) -> Result<usize, Error> {
        Err(Error::Storage)
    }
    fn write_archive(&mut self, _: usize, _: &[u8]) -> Result<(), Error> {
        Err(Error::Storage)
    }
    fn https_only(&self) -> bool {
        false
    }
    fn supports_transport(&self) -> bool {
        false
    }
    fn set_https_only(&mut self, _: bool) -> Result<(), Error> {
        Err(Error::Storage)
    }
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error>;
    fn append(&mut self, index: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error>;
    fn generation(&self) -> u64 {
        0
    }
    fn checkpoint_rows(&self) -> usize {
        0
    }
    fn supports_checkpoint(&self) -> bool {
        false
    }
    fn read_checkpoint(
        &mut self,
        _: usize,
        _: &mut [u8; checkpoint::ROW_BYTES],
    ) -> Result<usize, Error> {
        Err(Error::Storage)
    }
    fn begin_checkpoint(&mut self) -> Result<(), Error> {
        Err(Error::Capacity)
    }
    fn write_checkpoint(&mut self, _: usize, _: &[u8]) -> Result<(), Error> {
        Err(Error::Storage)
    }
    fn commit_checkpoint(&mut self, _: usize) -> Result<(), Error> {
        Err(Error::Storage)
    }
}

pub struct Service<J> {
    // Allocate fixed state once; returning/moving Service must not copy a
    // growing inline state through the small firmware startup stack.
    pub(crate) state: Box<State>,
    pub(crate) auth: crate::auth::Auth,
    pub(crate) journal: J,
    pub(crate) page_rows: std::vec::Vec<crate::domain::Transaction>,
    pub(crate) audit_rows: std::vec::Vec<crate::ledger::Audit>,
    records: usize,
    storage_failed: bool,
    https_only: bool,
    keyed: VecDeque<KeyReceipt>,
    clock: fn() -> u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct KeyReceipt {
    key: [u8; 32],
    actor: MemberId,
    command: [u8; 32],
    sequence: u64,
    timestamp: u64,
}

impl<J: Journal> Service<J> {
    pub fn open(journal: J) -> Result<Self, Error> {
        Self::open_with_clock(journal, unix_time)
    }

    pub fn open_with_clock(mut journal: J, clock: fn() -> u64) -> Result<Self, Error> {
        let https_only = journal.https_only();
        let mut state = Box::new(State::default());
        let mut frame = [0; FRAME_SIZE];
        let mut records = 0;
        let mut keyed = VecDeque::with_capacity(MAX_RECORDS);
        checkpoint::restore(&mut journal, &mut state, &mut keyed)?;
        while records < MAX_RECORDS && journal.read(records, &mut frame)? {
            let event = decode(&frame)?;
            state.replay(&event)?;
            if let Some(key) = event.client_key {
                if keyed
                    .iter()
                    .any(|r: &KeyReceipt| r.actor == event.actor && r.key == key)
                {
                    return Err(Error::CorruptJournal);
                }
                if keyed.len() == MAX_RECORDS {
                    keyed.pop_front();
                }
                keyed.push_back(KeyReceipt {
                    key,
                    actor: event.actor,
                    command: crate::domain::fingerprint(&event.command),
                    sequence: event.sequence,
                    timestamp: event.timestamp,
                });
            }
            records += 1;
        }
        state.check_invariants()?;
        Ok(Self {
            state,
            auth: crate::auth::Auth::default(),
            journal,
            page_rows: std::vec::Vec::with_capacity(100),
            audit_rows: std::vec::Vec::with_capacity(16),
            records,
            storage_failed: false,
            https_only,
            keyed,
            clock,
        })
    }

    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn storage_failed(&self) -> bool {
        self.storage_failed
    }

    pub fn generation(&self) -> u64 {
        self.journal.generation()
    }
    pub fn https_only(&self) -> bool {
        self.https_only
    }
    pub fn supports_transport(&self) -> bool {
        self.journal.supports_transport()
    }
    pub fn require_https(&mut self, actor: MemberId) -> Result<(), Error> {
        self.state.admin(actor)?;
        if !self.supports_transport() || self.storage_failed {
            return Err(Error::Storage);
        }
        self.https_only = true;
        self.auth.clear();
        if self.journal.set_https_only(true).is_err() {
            self.storage_failed = true;
            return Err(Error::Storage);
        }
        Ok(())
    }
    pub fn journal_records(&self) -> usize {
        self.records
    }
    pub fn checkpoint_supported(&self) -> bool {
        self.journal.supports_checkpoint()
    }

    pub(crate) fn diagnostic_storage(&self) -> (usize, usize, usize, usize) {
        (
            self.journal.checkpoint_rows(),
            self.keyed.len(),
            self.keyed.capacity(),
            core::mem::size_of::<KeyReceipt>(),
        )
    }

    pub(crate) fn diagnostic_model_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
            + core::mem::size_of::<State>()
            + self.state.history.capacity() * core::mem::size_of::<crate::domain::Transaction>()
            + self.state.fulfillments.capacity()
                * core::mem::size_of::<crate::fulfillment::Fulfillment>()
            + self.keyed.capacity() * core::mem::size_of::<KeyReceipt>()
            + self.state.ledger.corrections.capacity()
                * core::mem::size_of::<crate::ledger::Correction>()
            + self.state.ledger.epochs.capacity() * core::mem::size_of::<crate::ledger::Epoch>()
            + self.state.ledger.audit.capacity() * core::mem::size_of::<crate::ledger::Audit>()
            + self.page_rows.capacity() * core::mem::size_of::<crate::domain::Transaction>()
            + self.audit_rows.capacity() * core::mem::size_of::<crate::ledger::Audit>()
            + self.state.commerce.requests.capacity()
                * core::mem::size_of::<crate::commerce::GiftRequest>()
            + self.state.commerce.artworks.capacity()
                * core::mem::size_of::<crate::commerce::Artwork>()
    }

    /// Caller holds the service mutex. A failed/ambiguous storage operation
    /// latches the service until restart; never continue from uncertain state.
    pub fn checkpoint(&mut self, actor: MemberId) -> Result<(), Error> {
        self.state.admin(actor)?;
        self.rotate(false)
    }

    pub fn reset_economy(&mut self, actor: MemberId) -> Result<(), Error> {
        self.state.admin(actor)?;
        self.rotate(true)
    }

    fn rotate(&mut self, reset: bool) -> Result<(), Error> {
        if self.storage_failed {
            return Err(Error::Storage);
        }
        if !self.journal.supports_checkpoint() {
            return Err(Error::Capacity);
        }
        let result = if reset {
            checkpoint::save_empty(&mut self.journal, self.state.archive)
        } else {
            self.state.check_invariants()?;
            checkpoint::save(&mut self.journal, &self.state, &self.keyed)
        };
        if result.is_err() {
            self.storage_failed = true;
            return Err(Error::Storage);
        }
        self.state.archive = result.unwrap();
        while self
            .state
            .history
            .front()
            .is_some_and(|t| t.meta.ordinal < self.state.archive.first_transaction)
        {
            self.state.history.pop_front();
        }
        self.records = 0;
        if reset {
            self.state.clear_economy();
            self.keyed.clear();
            self.auth.clear();
        }
        Ok(())
    }

    /// A clock earlier than the last durable event is unavailable for timed deals.
    pub fn now(&self) -> u64 {
        let now = (self.clock)();
        if now < self.state.last_timestamp {
            0
        } else {
            now
        }
    }

    pub(crate) fn event_timestamp(&mut self, sequence: u64) -> Result<u64, Error> {
        if self.storage_failed {
            return Err(Error::Storage);
        }
        self.keyed
            .iter()
            .find(|r| r.sequence == sequence)
            .map(|r| r.timestamp)
            .ok_or(Error::StaleRequest)
    }

    pub fn execute(
        &mut self,
        actor: MemberId,
        request_id: u64,
        command: Command,
    ) -> Result<Receipt, Error> {
        if actor == MemberId(0)
            || matches!(command, Command::RunLoan { .. } | Command::RunLotto { .. })
        {
            return Err(Error::Forbidden);
        }
        self.commit(actor, request_id, command, None)
    }

    /// Durable bounded retry receipts survive checkpoints. Command hashes
    /// reject altered retries; generation tags reject evicted ancient keys.
    pub fn execute_keyed(
        &mut self,
        actor: MemberId,
        key: &str,
        command: Command,
    ) -> Result<Receipt, Error> {
        if self.storage_failed {
            return Err(Error::Storage);
        }
        if key.is_empty() || key.len() > 80 {
            return Err(Error::InvalidInput);
        }
        let digest = crate::auth::digest(key);
        if let Some(receipt) = self
            .keyed
            .iter()
            .find(|r| r.actor == actor && r.key == digest)
        {
            if receipt.command != crate::domain::fingerprint(&command) {
                return Err(Error::Conflict);
            }
            return Ok(Receipt {
                sequence: receipt.sequence,
                replayed: true,
            });
        }
        // Keys from retired generations may be retried only while a receipt
        // remains. Never interpret an ancient retry as a fresh payment.
        let epoch = key
            .strip_prefix('g')
            .and_then(|k| k.split_once(':'))
            .and_then(|(n, _)| n.parse::<u64>().ok());
        if epoch.unwrap_or(0) != self.generation() {
            return Err(Error::StaleRequest);
        }
        let money_epoch = key
            .split(':')
            .nth(1)
            .and_then(|s| s.strip_prefix('m'))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        if money_epoch != self.state.money_epoch {
            return Err(Error::StaleRequest);
        }
        if actor == MemberId(0)
            || matches!(command, Command::RunLoan { .. } | Command::RunLotto { .. })
        {
            return Err(Error::Forbidden);
        }
        let request_id = self.state.member(actor)?.last_request + 1;
        self.commit(actor, request_id, command, Some(digest))
    }

    fn commit(
        &mut self,
        actor: MemberId,
        request_id: u64,
        command: Command,
        client_key: Option<[u8; 32]>,
    ) -> Result<Receipt, Error> {
        if self.storage_failed {
            return Err(Error::Storage);
        }
        if actor != MemberId(0) {
            if let Some(receipt) = self.state.retry(actor, request_id, &command)? {
                return Ok(receipt);
            }
        }
        let now = self.now();
        self.state.validate_at(actor, &command, now)?;
        if self.journal.supports_checkpoint()
            && (self.records >= 2048
                || (self.journal.supports_archive()
                    && (self.state.transactions - self.state.archive.transactions >= 1024
                        || self.records >= 1024)))
        {
            self.rotate(false)?;
            // Retention may evict an unpinned original: revalidate before append.
            self.state.validate_at(actor, &command, now)?;
        }
        if self.records == MAX_RECORDS {
            return Err(Error::Capacity);
        }
        let sequence = crate::domain::next_sequence(self.state.sequence).ok_or(Error::Overflow)?;
        let event = Event {
            timestamp: now.max(self.state.last_timestamp),
            client_key,
            version: 2,
            sequence,
            actor,
            request_id,
            command,
        };
        let frame = encode(&event)?;
        if self.journal.append(self.records, &frame).is_err() {
            self.storage_failed = true;
            return Err(Error::Storage);
        }
        self.state.apply(&event);
        if let Some(key) = client_key {
            if self.keyed.len() == MAX_RECORDS {
                self.keyed.pop_front();
            }
            self.keyed.push_back(KeyReceipt {
                key,
                actor,
                command: crate::domain::fingerprint(&event.command),
                sequence: event.sequence,
                timestamp: event.timestamp,
            });
        }
        if let Command::UpdateMember {
            member,
            password,
            role,
            disabled,
            ..
        } = &event.command
        {
            if password.is_some() || role.is_some() || *disabled == Some(true) {
                self.auth.revoke_member(*member);
            }
        }
        self.records += 1;
        Ok(Receipt {
            sequence,
            replayed: false,
        })
    }

    /// Background work shares the financial owner. At most four bounded events
    /// per tick; no allocation, logged-in user, HTTP request, or timer per loan.
    pub fn tick(&mut self) -> Result<usize, Error> {
        if self.storage_failed {
            return Err(Error::Storage);
        }
        let now = self.now();
        if !(crate::offers::MIN_CLOCK..=crate::domain::MAX_SEQUENCE).contains(&now) {
            return Ok(0);
        }
        let mut ready = heapless::Vec::<(u64, u64), { crate::loans::LOANS }>::new();
        for l in &self.state.loans {
            if self.state.loan_ready(l, now) {
                ready
                    .push((
                        if l.status == crate::loans::LoanStatus::Armed {
                            l.updated_at
                        } else {
                            l.next_due_at
                        },
                        l.id,
                    ))
                    .unwrap();
            }
        }
        ready.sort_unstable();
        let mut completed = 0;
        // Validate all bounded candidates so an overflowing agreement cannot
        // permanently starve later loans. Failed arithmetic never writes.
        for (_, id) in ready {
            if completed == 4 {
                break;
            }
            let command = Command::RunLoan {
                loan: id,
                expected_updated_at: self.state.loan(id)?.updated_at,
            };
            if self.state.validate_at(MemberId(0), &command, now).is_err() {
                continue;
            }
            self.commit(MemberId(0), self.state.sequence + 1, command, None)?;
            completed += 1;
        }
        let ids: heapless::Vec<u64, { crate::lotto::LOTTOS }> = self
            .state
            .lottos
            .iter()
            .filter(|l| l.step < 19 && now >= l.due_at())
            .map(|l| l.id)
            .collect();
        for id in ids {
            if completed == 4 {
                break;
            }
            let l = self.state.lotto(id)?;
            let command = Command::RunLotto {
                lotto: id,
                step: l.step,
                ticket: if l.step == 0 {
                    crate::lotto::random_ticket(l.total_tickets())?
                } else {
                    0
                },
            };
            if self.state.validate_at(MemberId(0), &command, now).is_err() {
                continue;
            }
            self.commit(MemberId(0), self.state.sequence + 1, command, None)?;
            completed += 1;
        }
        Ok(completed)
    }
}

fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn encode(event: &Event) -> Result<[u8; FRAME_SIZE], Error> {
    let mut frame = [0; FRAME_SIZE];
    frame[..4].copy_from_slice(b"NCR2");
    let len = postcard::to_slice(event, &mut frame[12..])
        .map_err(|_| Error::Capacity)?
        .len();
    frame[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    let mut crc = crc32fast::Hasher::new();
    crc.update(&frame[..8]);
    crc.update(&frame[12..12 + len]);
    let checksum = crc.finalize();
    frame[8..12].copy_from_slice(&checksum.to_le_bytes());
    Ok(frame)
}

pub fn decode(frame: &[u8; FRAME_SIZE]) -> Result<Event, Error> {
    let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
    let checksum = u32::from_le_bytes(frame[8..12].try_into().unwrap());
    if &frame[..4] != b"NCR2" || len == 0 || len > FRAME_SIZE - 12 {
        return Err(Error::CorruptJournal);
    }
    let mut crc = crc32fast::Hasher::new();
    crc.update(&frame[..8]);
    crc.update(&frame[12..12 + len]);
    if crc.finalize() != checksum || frame[12 + len..].iter().any(|b| *b != 0) {
        return Err(Error::CorruptJournal);
    }
    let (event, remaining) =
        postcard::take_from_bytes(&frame[12..12 + len]).map_err(|_| Error::CorruptJournal)?;
    if !remaining.is_empty() {
        return Err(Error::CorruptJournal);
    }
    Ok(event)
}

#[cfg(feature = "desktop")]
pub mod file;
