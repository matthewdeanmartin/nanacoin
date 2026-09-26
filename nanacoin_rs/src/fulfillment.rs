//! Physical obligations are independent of payment and offer settlement.
use crate::domain::*;
use serde::{Deserialize, Serialize};

pub const CAPACITY: usize = 128;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Todo,
    Done,
    Disputed,
    Reversed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Complete,
    Dispute,
    WithdrawDispute,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Kind {
    Work,
    Goods,
    Cash,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Update {
    pub sequence: u64,
    pub at: u64,
    pub actor: MemberId,
    pub status: Status,
    pub reason: Memo,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fulfillment {
    pub transaction: u64,
    pub payment: Transaction,
    pub provider: MemberId,
    pub recipient: MemberId,
    pub description: TransactionMemo,
    pub kind: Kind,
    pub status: Status,
    /// Recent activity; the complete audit remains in the journal until compaction.
    pub updates: heapless::Vec<Update, 4>,
}
impl State {
    pub(crate) fn creates_fulfillment(&self, c: &Command) -> bool {
        matches!(c, Command::Buy { .. } | Command::AcceptOffer { .. })
            || matches!(c, Command::ClassifiedTransfer { economic, amount, .. } if *amount > 0 && matches!(economic.kind, EconomicKind::Labor | EconomicKind::Good))
    }
    pub(crate) fn fulfillment_room(&self) -> bool {
        self.fulfillments.len() < CAPACITY
            || self.fulfillments.iter().any(|f| {
                matches!(f.status, Status::Done | Status::Reversed)
                    && !self.history.iter().any(|t| t.id == f.transaction)
            })
    }
    pub(crate) fn validate_fulfillment(
        &self,
        actor: MemberId,
        transaction: u64,
        action: Action,
        reason: &Memo,
    ) -> Result<(), Error> {
        let f = self
            .fulfillments
            .iter()
            .find(|f| f.transaction == transaction)
            .ok_or(Error::NotFound)?;
        if reason.chars().any(char::is_control) {
            return Err(Error::InvalidInput);
        }
        let allowed = match action {
            Action::Complete => actor == f.provider || actor == f.recipient,
            _ => actor == f.recipient,
        };
        if !allowed {
            return Err(Error::Forbidden);
        }
        let valid = match action {
            Action::Complete => f.status == Status::Todo,
            Action::Dispute => matches!(f.status, Status::Todo | Status::Done),
            Action::WithdrawDispute => f.status == Status::Disputed,
        };
        if !valid {
            return Err(Error::Conflict);
        }
        if action == Action::Dispute && reason.trim().is_empty() {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    pub(crate) fn apply_fulfillment(
        &mut self,
        transaction: u64,
        action: Action,
        reason: &Memo,
        event: &Event,
    ) {
        let f = self
            .fulfillments
            .iter_mut()
            .find(|f| f.transaction == transaction)
            .unwrap();
        f.status = match action {
            Action::Dispute => Status::Disputed,
            _ => Status::Done,
        };
        f.update(event.sequence, event.timestamp, event.actor, reason.clone());
    }
    pub(crate) fn track_fulfillment(&mut self, tx: &Transaction) {
        if let Some(original) = tx.reverses {
            if self.ledger.reversed_by(original).is_none() {
                return;
            }
            if let Some(f) = self
                .fulfillments
                .iter_mut()
                .find(|f| f.transaction == original)
            {
                f.status = Status::Reversed;

                f.update(tx.id, tx.created_at, tx.actor, Memo::new());
            }
            return;
        }
        if tx.usd
            || tx.amount == 0
            || tx.loan.is_some()
            || tx.lotto.is_some()
            || tx.quote.is_some()
            || tx.meta.art.is_some()
        {
            return;
        }
        if tx.listing.is_none()
            && !matches!(tx.economic.kind, EconomicKind::Labor | EconomicKind::Good)
        {
            return;
        }
        let cash = tx
            .listing
            .and_then(|id| self.listings.iter().find(|l| l.id == id))
            .is_some_and(|l| l.details.kind == "currency");
        let work = tx.economic.kind == EconomicKind::Labor
            || tx
                .listing
                .and_then(|id| self.listings.iter().find(|l| l.id == id))
                .is_some_and(|l| l.details.kind == "service");
        if self.fulfillments.len() == CAPACITY {
            let index = self
                .fulfillments
                .iter()
                .position(|f| {
                    matches!(f.status, Status::Done | Status::Reversed)
                        && !self.history.iter().any(|t| t.id == f.transaction)
                })
                .unwrap();
            self.fulfillments.remove(index);
        }
        let mut f = Fulfillment {
            transaction: tx.id,
            payment: tx.clone(),
            provider: tx.to,
            recipient: tx.from,
            description: tx
                .listing
                .and_then(|id| self.listings.iter().find(|l| l.id == id))
                .map(|l| TransactionMemo::try_from(l.title.as_str()).unwrap())
                .unwrap_or_else(|| tx.memo.clone()),
            kind: if cash {
                Kind::Cash
            } else if work {
                Kind::Work
            } else {
                Kind::Goods
            },
            status: Status::Todo,
            updates: heapless::Vec::new(),
        };
        f.update(tx.id, tx.created_at, tx.actor, Memo::new());
        self.fulfillments.push(f);
    }
    pub(crate) fn check_fulfillments(&self) -> Result<(), Error> {
        if self.fulfillments.len() > CAPACITY {
            return Err(Error::CorruptJournal);
        }
        for (i, f) in self.fulfillments.iter().enumerate() {
            if f.payment.id != f.transaction
                || f.payment.to != f.provider
                || f.payment.from != f.recipient
                || f.payment.amount <= 0
                || f.payment.amount > MAX_AMOUNT
                || f.payment.usd
                || f.payment.reverses.is_some()
                || self.ledger.reversed_by(f.transaction).is_some()
                    != (f.status == Status::Reversed)
                || f.transaction == 0
                || f.transaction > self.sequence
                || f.provider == f.recipient
                || self.member(f.provider).is_err()
                || self.member(f.recipient).is_err()
                || self.fulfillments[..i]
                    .iter()
                    .any(|other| other.transaction == f.transaction)
                || f.updates.last().map(|u| u.status) != Some(f.status)
                || f.updates
                    .iter()
                    .any(|u| u.sequence > self.sequence || self.member(u.actor).is_err())
            {
                return Err(Error::CorruptJournal);
            }
        }
        Ok(())
    }
}
impl Fulfillment {
    fn update(&mut self, sequence: u64, at: u64, actor: MemberId, reason: Memo) {
        if self.updates.is_full() {
            self.updates.remove(0);
        }
        self.updates
            .push(Update {
                sequence,
                at,
                actor,
                status: self.status,
                reason,
            })
            .unwrap();
    }
}
