//! Immutable transaction facts, durable correction annotations and epoch totals.
use crate::domain::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const CORRECTIONS: usize = crate::board::CORRECTIONS;

#[cfg(test)]
mod integrity_tests {
    use super::*;
    fn state() -> State {
        let mut s = State::default();
        for (sequence, command) in [
            (
                1,
                Command::AddMember {
                    name: "Nana".try_into().unwrap(),
                    token_hash: [1; 32],
                },
            ),
            (
                2,
                Command::Issue {
                    to: MemberId(1),
                    amount: 7,
                    memo: Memo::new(),
                },
            ),
        ] {
            s.replay(&Event {
                timestamp: sequence,
                client_key: None,
                version: 2,
                sequence,
                actor: MemberId(1),
                request_id: sequence,
                command,
            })
            .unwrap();
        }
        s.check_invariants().unwrap();
        s
    }
    #[test]
    fn corrupt_corrections_are_rejected() {
        for correction in [
            Correction {
                original: 0,
                original_amount: 7,
                refunded: 1,
                full: None,
            },
            Correction {
                original: 2,
                original_amount: 7,
                refunded: -1,
                full: None,
            },
            Correction {
                original: 2,
                original_amount: 7,
                refunded: 8,
                full: None,
            },
            Correction {
                original: 2,
                original_amount: 7,
                refunded: 1,
                full: Some(3),
            },
            Correction {
                original: 2,
                original_amount: 7,
                refunded: 7,
                full: None,
            },
        ] {
            let mut s = state();
            s.ledger.corrections.push(correction);
            assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
        }
        let mut s = state();
        let c = Correction {
            original: 2,
            original_amount: 7,
            refunded: 1,
            full: None,
        };
        s.ledger.corrections.push(c.clone());
        s.ledger.corrections.push(c);
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
    }
    #[test]
    fn malformed_epochs_ordinals_and_missing_audit_tail_fail_closed() {
        let mut s = state();
        s.ledger.epochs[0].flows[0] = i128::MAX;
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
        let mut s = state();
        s.money_epoch = 1;
        s.ledger.epochs.push(Epoch {
            decimals: 4,
            exponent: i16::MIN,
            flows: [0; FLOW_COUNT],
        });
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
        let mut s = state();
        s.history.back_mut().unwrap().meta.ordinal = 1;
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
        let mut s = state();
        s.ledger.audit.pop_back();
        assert_eq!(s.check_invariants(), Err(Error::CorruptJournal));
    }
}
pub const EPOCHS: usize = 32;
pub const AUDIT_CACHE: usize = crate::board::AUDIT_CACHE;
pub const FLOW_COUNT: usize = 20;
pub const FLOW_NAMES: [&str; FLOW_COUNT] = [
    "issued_to_nana",
    "issued_to_members",
    "issued_for_lotto_interest",
    "retired",
    "corrections",
    "net_issuance",
    "usd_recorded",
    "coins_bought_back",
    "usd_paid_out",
    "coins_sold",
    "usd_taken_in",
    "interest_received",
    "interest_paid",
    "lent",
    "repaid",
    "bought_from_members",
    "sold_to_members",
    "paid_out",
    "received_other",
    "transactions",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionMeta {
    pub ordinal: u64,
    pub group: u64,
    pub epoch: u64,
    pub decimals: u8,
    pub gift_request: Option<u64>,
    pub art: Option<u64>,
    /// Original minor units refunded, even if the cash leg is in a later epoch.
    pub refund_units: i64,
    pub original_amount: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Correction {
    pub original: u64,
    pub original_amount: i64,
    pub refunded: i64,
    pub full: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Epoch {
    pub decimals: u8,
    /// Exponent converting preceding epoch minor units to this epoch.
    pub exponent: i16,
    pub flows: [i128; FLOW_COUNT],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
// Bounded inline variants preserve allocation-free command/replay paths.
#[allow(clippy::large_enum_variant)]
pub enum AuditAction {
    Business(Command),
    Identity {
        member: MemberId,
        name: Name,
        role: Option<Role>,
        disabled: Option<bool>,
        credentials_changed: bool,
        created: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Audit {
    pub sequence: u64,
    pub actor: MemberId,
    pub at: u64,
    pub action: AuditAction,
}

impl Audit {
    /// Public audit records must never contain credential-bearing commands,
    /// including when read from a correctly checksummed but malformed page.
    pub(crate) fn validate_public(&self) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            let (identity, member, created, name, role, disabled, credentials) = match &self.action
            {
                AuditAction::Identity {
                    member,
                    name,
                    role,
                    disabled,
                    credentials_changed,
                    created,
                } => (
                    true,
                    member.0 as i64,
                    *created,
                    !name.is_empty(),
                    role.is_some(),
                    disabled.is_some(),
                    *credentials_changed,
                ),
                _ => (false, 0, false, false, false, false, false),
            };
            let secrets = matches!(
                &self.action,
                AuditAction::Business(
                    Command::Provision { .. }
                        | Command::CreateMember { .. }
                        | Command::CreateBot { .. }
                        | Command::AddMember { .. }
                        | Command::UpdateMember { .. }
                        | Command::MigrateMember { .. }
                        | Command::SetApiKey { .. }
                        | Command::SetReadKey { .. }
                )
            );
            crate::cobol::policy(
                57,
                &[
                    1,
                    crate::cobol::ledger_number(self.sequence),
                    self.actor.0.into(),
                    identity.into(),
                    member,
                    created.into(),
                    name.into(),
                    role.into(),
                    disabled.into(),
                    credentials.into(),
                    secrets.into(),
                ],
            )
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if self.sequence == 0 || self.sequence > MAX_SEQUENCE || self.actor.0 as usize > MEMBERS
            {
                return Err(Error::CorruptJournal);
            }
            match &self.action {
                AuditAction::Business(
                    Command::Provision { .. }
                    | Command::CreateMember { .. }
                    | Command::AddMember { .. }
                    | Command::UpdateMember { .. }
                    | Command::MigrateMember { .. }
                    | Command::SetApiKey { .. }
                    | Command::CreateBot { .. }
                    | Command::SetReadKey { .. },
                ) => Err(Error::CorruptJournal),
                AuditAction::Identity { member, .. }
                    if member.0 == 0 || member.0 as usize > MEMBERS =>
                {
                    Err(Error::CorruptJournal)
                }
                AuditAction::Identity {
                    created: true,
                    name,
                    disabled,
                    credentials_changed,
                    ..
                } if name.is_empty() || disabled.is_some() || !credentials_changed => {
                    Err(Error::CorruptJournal)
                }
                _ => Ok(()),
            }
        }
    }
}

#[derive(Debug)]
pub struct Ledger {
    pub corrections: std::vec::Vec<Correction>,
    pub epochs: std::vec::Vec<Epoch>,
    pub audit: VecDeque<Audit>,
}
impl Default for Ledger {
    fn default() -> Self {
        let mut s = Self {
            corrections: std::vec::Vec::with_capacity(CORRECTIONS),
            epochs: std::vec::Vec::with_capacity(EPOCHS),
            audit: VecDeque::with_capacity(AUDIT_CACHE),
        };
        s.clear();
        s
    }
}
impl Ledger {
    pub fn clear(&mut self) {
        self.corrections.clear();
        self.epochs.clear();
        self.audit.clear();
        self.epochs.push(Epoch {
            decimals: 4,
            exponent: 0,
            flows: [0; FLOW_COUNT],
        });
    }
    pub fn refunded(&self, id: u64) -> i64 {
        self.corrections
            .iter()
            .find(|c| c.original == id)
            .map_or(0, |c| c.refunded)
    }
    pub fn reversed_by(&self, id: u64) -> Option<u64> {
        self.corrections
            .iter()
            .find(|c| c.original == id)
            .and_then(|c| c.full)
    }
    #[cfg(not(feature = "cobol-core"))]
    pub fn audit(&mut self, e: &Event) {
        let action = match &e.command {
            Command::Provision { display_name, .. } => AuditAction::Identity {
                member: MemberId(1),
                name: display_name.clone(),
                role: Some(Role::Nana),
                disabled: None,
                credentials_changed: true,
                created: true,
            },
            Command::CreateMember {
                display_name, role, ..
            } => AuditAction::Identity {
                member: MemberId(0),
                name: display_name.clone(),
                role: Some(*role),
                disabled: None,
                credentials_changed: true,
                created: true,
            },
            Command::CreateBot { display_name, .. } => AuditAction::Identity {
                member: MemberId(0),
                name: display_name.clone(),
                role: Some(Role::User),
                disabled: None,
                credentials_changed: true,
                created: true,
            },
            Command::SetReadKey { member, .. } => AuditAction::Identity {
                member: *member,
                name: Name::new(),
                role: None,
                disabled: None,
                credentials_changed: true,
                created: false,
            },
            Command::AddMember { name, .. } => AuditAction::Identity {
                member: MemberId(0),
                name: name.clone(),
                role: None,
                disabled: None,
                credentials_changed: true,
                created: true,
            },
            Command::UpdateMember {
                member,
                display_name,
                role,
                disabled,
                password,
                ..
            } => AuditAction::Identity {
                member: *member,
                name: display_name.clone().unwrap_or_default(),
                role: *role,
                disabled: *disabled,
                credentials_changed: password.is_some(),
                created: false,
            },
            Command::MigrateMember {
                member, username, ..
            } => AuditAction::Identity {
                member: *member,
                name: username.clone(),
                role: None,
                disabled: None,
                credentials_changed: true,
                created: false,
            },
            Command::SetApiKey { member, .. } => AuditAction::Identity {
                member: *member,
                name: Name::new(),
                role: None,
                disabled: None,
                credentials_changed: true,
                created: false,
            },
            c => AuditAction::Business(c.clone()),
        };
        if self.audit.len() == AUDIT_CACHE {
            self.audit.pop_front();
        }
        self.audit.push_back(Audit {
            sequence: e.sequence,
            actor: e.actor,
            at: e.timestamp,
            action,
        });
    }
    #[cfg(feature = "cobol-core")]
    pub(crate) fn apply_audit(&mut self, row: Audit, evict: bool) {
        if evict {
            self.audit.pop_front();
        }
        self.audit.push_back(row);
    }
}

impl State {
    pub(crate) fn check_ledger(&self) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        return self.bank_ledger_invariants();
        #[cfg(not(feature = "cobol-core"))]
        {
            if self.transactions > self.sequence.saturating_mul(2)
                || self.transactions > MAX_SEQUENCE * 2
                || self.money_epoch >= EPOCHS as u64
                || self.ledger.epochs.len() != self.money_epoch as usize + 1
                || self.ledger.corrections.len() > CORRECTIONS
                || self.ledger.audit.len() > AUDIT_CACHE
                || self.history.len() > HISTORY
                || self.archive.transactions > self.transactions
                || self.archive.first_transaction > self.archive.transactions
                || self.archive.audit_sequence > self.sequence
                || (self.archive.first_page == 0
                    && self.history.len() as u64 != self.transactions.min(HISTORY as u64))
            {
                return Err(Error::CorruptJournal);
            }
            for (i, epoch) in self.ledger.epochs.iter().enumerate() {
                if epoch.decimals > 8
                    || (i == 0 && (epoch.decimals != 4 || epoch.exponent != 0))
                    || (i > 0
                        && !(-12..=12).contains(
                            &(epoch.decimals as i32
                                - self.ledger.epochs[i - 1].decimals as i32
                                - epoch.exponent as i32),
                        ))
                    || !(0..=self.transactions as i128).contains(&epoch.flows[19])
                    || epoch
                        .flows
                        .iter()
                        .any(|n| n.unsigned_abs() > 32 * MAX_SEQUENCE as u128 * MAX_AMOUNT as u128)
                {
                    return Err(Error::CorruptJournal);
                }
            }
            if self
                .ledger
                .epochs
                .last()
                .is_none_or(|e| e.decimals != self.decimals)
            {
                return Err(Error::CorruptJournal);
            }
            for (i, c) in self.ledger.corrections.iter().enumerate() {
                if c.original == 0
                    || c.original > MAX_SEQUENCE
                    || !(0..=MAX_AMOUNT).contains(&c.original_amount)
                    || !(0..=c.original_amount).contains(&c.refunded)
                    || c.full.is_some() != (c.refunded == c.original_amount)
                    || c.full
                        .is_some_and(|id| id == 0 || id > MAX_SEQUENCE || id == c.original)
                    || self.ledger.corrections[..i]
                        .iter()
                        .any(|other| other.original == c.original)
                {
                    return Err(Error::CorruptJournal);
                }
                if let Ok(original) = self.original_payment(c.original) {
                    if original.reverses.is_some()
                        || c.original_amount != original.amount
                        || c.refunded > original.amount
                        || (c.full.is_some() && c.refunded != original.amount)
                        || (c.full.is_none() && c.refunded == original.amount)
                    {
                        return Err(Error::CorruptJournal);
                    }
                }
            }
            let mut ordinal = None;
            for t in &self.history {
                if t.meta.ordinal >= self.transactions
                    || t.meta.ordinal < self.archive.first_transaction
                    || ordinal.is_some_and(|n| t.meta.ordinal != n + 1)
                    || t.meta.epoch > self.money_epoch
                    || t.meta.group == 0
                    || t.meta.group > self.sequence
                    || t.meta.decimals
                        != if t.usd {
                            2
                        } else {
                            self.ledger.epochs[t.meta.epoch as usize].decimals
                        }
                    || !(0..=MAX_AMOUNT).contains(&t.meta.refund_units)
                    || !(0..=MAX_AMOUNT).contains(&t.meta.original_amount)
                    || (t.reverses.is_some()
                        && (t.meta.refund_units <= 0
                            || t.meta.original_amount <= 0
                            || t.meta.refund_units > t.meta.original_amount))
                {
                    return Err(Error::CorruptJournal);
                }
                ordinal = Some(t.meta.ordinal);
            }
            if ordinal.is_some_and(|last| last + 1 != self.transactions)
                || (self.transactions > 0 && ordinal.is_none() && self.archive.first_page == 0)
            {
                return Err(Error::CorruptJournal);
            }
            let mut audit_sequence = None;
            for a in &self.ledger.audit {
                a.validate_public()?;
                if a.sequence > self.sequence
                    || a.at > self.last_timestamp
                    || audit_sequence.is_some_and(|n| next_sequence(n) != Some(a.sequence))
                {
                    return Err(Error::CorruptJournal);
                }
                audit_sequence = Some(a.sequence);
            }
            if self.sequence > 0 && audit_sequence != Some(self.sequence) {
                return Err(Error::CorruptJournal);
            }
            Ok(())
        }
    }
    pub fn original_payment(&self, id: u64) -> Result<&Transaction, Error> {
        self.history
            .iter()
            .find(|t| t.id == id)
            .or_else(|| {
                self.fulfillments
                    .iter()
                    .find(|f| f.transaction == id)
                    .map(|f| &f.payment)
            })
            .ok_or(Error::NotFound)
    }
    pub fn current_amount(&self, tx: &Transaction, units: i64) -> Result<i64, Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.historical_amount(tx, units)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if tx.usd {
                return Ok(units);
            }
            let exponent: i16 = self
                .ledger
                .epochs
                .iter()
                .skip(tx.meta.epoch as usize + 1)
                .map(|e| e.exponent)
                .sum();
            crate::money::rescale(units, exponent, MAX_AMOUNT)
        }
    }
    pub(crate) fn validate_refund(
        &self,
        actor: MemberId,
        id: u64,
        units: i64,
    ) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            let p = self.correction_policy(actor, id, Some(units))?;
            let tx = self.original_payment(id)?;
            self.validate_posting(
                p.payer,
                p.payee,
                self.current_amount(tx, p.units)?,
                p.overdraft,
            )
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            let tx = self.original_payment(id)?;
            if tx.reverses.is_some()
                || tx.amount == 0
                || tx.usd
                || tx.from == MemberId(0)
                || tx.to == MemberId(0)
                || tx.loan.is_some()
                || tx.lotto.is_some()
                || tx.quote.is_some()
                || tx.meta.art.is_some()
            {
                return Err(Error::Forbidden);
            }
            if actor != tx.to {
                self.admin(actor)?;
            }
            if units <= 0 || units > tx.amount - self.ledger.refunded(id) {
                return Err(Error::Conflict);
            }
            self.correction_room(id)?;
            self.validate_posting(tx.to, tx.from, self.current_amount(tx, units)?, false)
        }
    }
    pub(crate) fn correction_room(&self, id: u64) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.correction_capacity(id)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if self.ledger.corrections.len() == CORRECTIONS
                && !self.ledger.corrections.iter().any(|c| c.original == id)
            {
                Err(Error::Capacity)
            } else {
                Ok(())
            }
        }
    }
    pub(crate) fn apply_refund(&mut self, event: &Event, id: u64, _units: i64, memo: &Memo) {
        let original = self.original_payment(id).unwrap().clone();
        #[cfg(not(feature = "cobol-core"))]
        let units = _units;
        #[cfg(feature = "cobol-core")]
        let amount = self.prepared_amount(0);
        #[cfg(not(feature = "cobol-core"))]
        let amount = self.current_amount(&original, units).unwrap();
        #[cfg(feature = "cobol-core")]
        let p = self
            .prepared
            .correction_policy
            .take()
            .expect("prepared correction policy");
        #[cfg(feature = "cobol-core")]
        let (payer, payee, units) = (p.payer, p.payee, p.units);
        #[cfg(not(feature = "cobol-core"))]
        let (payer, payee) = (original.to, original.from);
        self.record_transaction(Transaction {
            id: event.sequence,
            actor: event.actor,
            created_at: event.timestamp,
            from: payer,
            to: payee,
            amount,
            memo: TransactionMemo::try_from(memo.as_str()).unwrap(),
            reverses: Some(id),
            listing: original.listing,
            usd: original.usd,
            quote: original.quote,
            loan: original.loan,
            lotto: original.lotto,
            economic: original.economic,
            meta: TransactionMeta {
                refund_units: units,
                original_amount: original.amount,
                gift_request: original.meta.gift_request,
                ..TransactionMeta::default()
            },
        });
        if self.ledger.reversed_by(id).is_some() {
            self.mark_offer_reversed(id, event.timestamp);
        }
    }
    pub(crate) fn ledger_post(&mut self, tx: &Transaction) {
        if let Some(id) = tx.reverses {
            if let Ok(original) = self.original_payment(id).cloned() {
                self.commerce_refund(&original, tx.amount);
            }
            #[cfg(feature = "cobol-core")]
            {
                let prepared = self
                    .prepared
                    .correction
                    .take()
                    .expect("prepared correction annotation");
                assert_eq!(prepared.original, id, "prepared correction original");
                if let Some(c) = self
                    .ledger
                    .corrections
                    .iter_mut()
                    .find(|c| c.original == id)
                {
                    *c = prepared;
                } else {
                    self.ledger.corrections.push(prepared);
                }
            }
            #[cfg(not(feature = "cobol-core"))]
            {
                let units = tx.meta.refund_units;
                let original_amount = self
                    .original_payment(id)
                    .map(|t| t.amount)
                    .unwrap_or(tx.meta.original_amount);
                if let Some(c) = self
                    .ledger
                    .corrections
                    .iter_mut()
                    .find(|c| c.original == id)
                {
                    c.refunded += units;
                    if c.original_amount == c.refunded {
                        c.full = Some(tx.id);
                    }
                } else {
                    self.ledger.corrections.push(Correction {
                        original: id,
                        original_amount,
                        refunded: units,
                        full: (original_amount == units).then_some(tx.id),
                    });
                }
            }
        }
        #[cfg(feature = "cobol-core")]
        if !self.prepared_cash() {
            assert_eq!(tx.amount, 0, "empty prepared flow plan");
            return;
        }
        #[cfg(not(feature = "cobol-core"))]
        if tx.amount == 0 {
            return;
        }
        #[cfg(feature = "cobol-core")]
        {
            self.ledger.epochs[tx.meta.epoch as usize].flows = self.prepared_flows();
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            let mut delta = [0i128; FLOW_COUNT];
            delta[19] = 1;
            let reversal = tx.reverses.is_some();
            let amount = i128::from(tx.amount);
            let nana_delta = if tx.to == MemberId(1) {
                amount
            } else if tx.from == MemberId(1) {
                -amount
            } else {
                0
            };
            let mut book = |inbound: usize, outbound: usize| {
                if nana_delta != 0 {
                    let original = if reversal { -nana_delta } else { nana_delta };
                    delta[if original > 0 { inbound } else { outbound }] +=
                        if reversal { -amount } else { amount };
                }
            };
            if tx.usd {
                if tx.from == MemberId(0) {
                    delta[6] += amount;
                } else if tx.to == MemberId(0) {
                    delta[6] -= amount;
                } else if tx.quote.is_some() {
                    book(10, 8);
                }
            } else if tx.from == MemberId(0) || tx.to == MemberId(0) {
                let added = if tx.from == MemberId(0) {
                    amount
                } else {
                    -amount
                };
                delta[5] = added;
                if reversal {
                    delta[4] = added;
                } else if tx.to == MemberId(0) {
                    delta[3] = amount;
                } else if tx.lotto.is_some() {
                    delta[2] = amount;
                } else if tx.to == MemberId(1) {
                    delta[0] = amount;
                } else {
                    delta[1] = amount;
                }
            } else if tx.quote.is_some() {
                book(7, 9);
            } else {
                match tx.economic.kind {
                    EconomicKind::Interest => book(11, 12),
                    EconomicKind::LoanPrincipal => book(14, 13),
                    EconomicKind::Labor | EconomicKind::Good => book(16, 15),
                    _ => book(18, 17),
                }
            }
            // At most MAX_SEQUENCE events, two legs/event and MAX_AMOUNT each:
            // even gross lifetime flows are below 2^104, safely inside i128.
            let totals = &mut self.ledger.epochs[tx.meta.epoch as usize].flows;
            for (total, delta) in totals.iter_mut().zip(delta) {
                *total = total.checked_add(delta).expect("bounded lifetime flows");
            }
        }
    }
}
