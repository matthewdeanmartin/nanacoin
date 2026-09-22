//! Bounded negotiated deals. Commands carry intent; journal events carry time.
//! An acceptance retains its postings so undo does not depend on recent history.
use crate::domain::*;
use serde::{Deserialize, Serialize};

pub const OFFERS: usize = 32;
pub const DEFAULT_SETTLEMENT: u64 = 48 * 60 * 60;
pub const MIN_CLOCK: u64 = 1_577_836_800; // 2020: reject an unsynchronized board clock.
pub type OfferMessage = heapless::String<140>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settlement {
    pub transaction: u64,
    pub payer: MemberId,
    pub payee: MemberId,
    pub settles_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferPhase {
    Open,
    Accepted(Settlement),
    Declined,
    Withdrawn,
    Reversed(Settlement),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Offer {
    pub id: OfferId,
    pub listing: u64,
    pub owner: MemberId,
    pub listing_side: Side,
    pub listing_title: Title,
    pub offerer: MemberId,
    pub amount: i64,
    pub message: OfferMessage,
    pub created_at: u64,
    pub updated_at: u64,
    pub phase: OfferPhase,
}

impl Offer {
    pub fn settlement(&self) -> Option<Settlement> {
        match self.phase {
            OfferPhase::Accepted(s) | OfferPhase::Reversed(s) => Some(s),
            _ => None,
        }
    }
    pub fn reversible(&self, now: u64) -> bool {
        now >= MIN_CLOCK && matches!(self.phase, OfferPhase::Accepted(s) if now < s.settles_at)
    }
    pub fn status(&self, now: u64) -> &'static str {
        match self.phase {
            OfferPhase::Open => "OPEN",
            OfferPhase::Accepted(s) if now >= s.settles_at => "SETTLED",
            OfferPhase::Accepted(_) => "ACCEPTED",
            OfferPhase::Declined => "DECLINED",
            OfferPhase::Withdrawn => "WITHDRAWN",
            OfferPhase::Reversed(_) => "REVERSED",
        }
    }
    pub fn visible_to(&self, member: &Member) -> bool {
        !member.disabled
            && (member.role == Role::Nana || member.id == self.owner || member.id == self.offerer)
    }
    fn recyclable(&self, now: u64) -> bool {
        match self.phase {
            OfferPhase::Open => false,
            OfferPhase::Accepted(s) => now >= s.settles_at,
            _ => true,
        }
    }
}

impl State {
    pub fn offer(&self, id: OfferId) -> Result<&Offer, Error> {
        self.offers
            .iter()
            .find(|o| o.id == id)
            .ok_or(Error::NotFound)
    }

    pub(crate) fn listing_recyclable(&self, listing: &Listing, now: u64) -> bool {
        listing.status != ListingStatus::Active
            && !self.offers.iter().any(|o| {
                o.listing == listing.id
                    && matches!(o.phase, OfferPhase::Accepted(s) if now < s.settles_at)
            })
    }

    pub(crate) fn validate_offer(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<(), Error> {
        if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
            return Err(Error::Unavailable);
        }
        match command {
            Command::MakeOffer {
                listing,
                amount,
                message,
            } => {
                if !(1..=MAX_AMOUNT).contains(amount) || message.chars().any(char::is_control) {
                    return Err(Error::InvalidInput);
                }
                let l = self.listing(*listing)?;
                if l.status != ListingStatus::Active {
                    return Err(Error::ListingClosed);
                }
                if actor == l.owner {
                    return Err(Error::SelfDeal);
                }
                if self.offers.is_full() && !self.offers.iter().any(|o| o.recyclable(now)) {
                    return Err(Error::Capacity);
                }
            }
            Command::AcceptOffer { offer } => {
                let o = self.offer(*offer)?;
                if actor != o.owner {
                    return Err(Error::Forbidden);
                }
                if o.phase != OfferPhase::Open {
                    return Err(Error::OfferClosed);
                }
                let l = self.listing(o.listing)?;
                if l.status != ListingStatus::Active {
                    return Err(Error::ListingClosed);
                }
                if self.member(o.offerer)?.disabled || self.member(o.owner)?.disabled {
                    return Err(Error::Disabled);
                }
                let (payer, payee) = match l.side {
                    Side::Sell => (o.offerer, o.owner),
                    Side::Buy => (o.owner, o.offerer),
                };
                if l.economic.kind == EconomicKind::Labor && self.member(payee)?.role == Role::Nana
                {
                    return Err(Error::Forbidden);
                }
                self.validate_posting(payer, payee, o.amount, false)?;
                now.checked_add(self.offer_settles_after)
                    .filter(|v| *v <= MAX_SEQUENCE)
                    .ok_or(Error::Overflow)?;
            }
            Command::UnacceptOffer { offer, reason } => {
                let o = self.offer(*offer)?;
                if actor != o.owner && actor != o.offerer {
                    self.admin(actor)?;
                }
                let OfferPhase::Accepted(s) = o.phase else {
                    return Err(Error::OfferClosed);
                };
                if now >= s.settles_at {
                    return Err(Error::OfferSettled);
                }
                if reason.chars().any(char::is_control) {
                    return Err(Error::InvalidInput);
                }
                // An ordinary purchase cannot have replaced this sale: the listing is pinned.
                if self.listing(o.listing)?.status != ListingStatus::Sold {
                    return Err(Error::Conflict);
                }
                self.validate_posting(s.payee, s.payer, o.amount, true)?;
            }
            Command::DeclineOffer { offer } | Command::WithdrawOffer { offer } => {
                let o = self.offer(*offer)?;
                if o.phase != OfferPhase::Open {
                    return Err(Error::OfferClosed);
                }
                let permitted = if matches!(command, Command::DeclineOffer { .. }) {
                    o.owner
                } else {
                    o.offerer
                };
                if actor != permitted {
                    self.admin(actor)?;
                }
            }
            _ => unreachable!("offer command dispatch"),
        }
        Ok(())
    }

    pub(crate) fn apply_offer(&mut self, event: &Event) {
        let now = event.timestamp;
        match &event.command {
            Command::MakeOffer {
                listing,
                amount,
                message,
            } => {
                let owner = self.listing(*listing).unwrap().owner;
                if self.offers.is_full() {
                    let index = self
                        .offers
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| o.recyclable(now))
                        .min_by_key(|(_, o)| (o.updated_at, o.id.0))
                        .unwrap()
                        .0;
                    self.offers.remove(index);
                }
                self.offers
                    .push(Offer {
                        id: OfferId(event.sequence),
                        listing: *listing,
                        listing_side: self.listing(*listing).unwrap().side,
                        listing_title: self.listing(*listing).unwrap().title.clone(),
                        owner,
                        offerer: event.actor,
                        amount: *amount,
                        message: message.clone(),
                        created_at: now,
                        updated_at: now,
                        phase: OfferPhase::Open,
                    })
                    .unwrap();
            }
            Command::AcceptOffer { offer } => {
                let o = self.offer(*offer).unwrap();
                let l = self.listing(o.listing).unwrap();
                let (payer, payee) = match l.side {
                    Side::Sell => (o.offerer, o.owner),
                    Side::Buy => (o.owner, o.offerer),
                };
                let tx = Transaction {
                    id: event.sequence,
                    actor: event.actor,
                    created_at: event.timestamp,
                    from: payer,
                    to: payee,
                    amount: o.amount,
                    memo: TransactionMemo::try_from(l.title.as_str()).unwrap(),
                    reverses: None,
                    reversed: false,
                    listing: Some(l.id),
                    usd: false,
                    quote: None,
                    loan: None,
                    lotto: None,
                    economic: l.economic,
                };
                let listing_id = l.id;
                let o = self.offers.iter_mut().find(|o| o.id == *offer).unwrap();
                o.phase = OfferPhase::Accepted(Settlement {
                    transaction: event.sequence,
                    payer,
                    payee,
                    settles_at: now + self.offer_settles_after,
                });
                o.updated_at = now;
                let listing = self
                    .listings
                    .iter_mut()
                    .find(|l| l.id == listing_id)
                    .unwrap();
                listing.updated_at = now;
                listing.status = ListingStatus::Sold;
                listing.buyer = Some(payer);
                listing.sold_tx = Some(event.sequence);
                self.record_transaction(tx);
            }
            Command::UnacceptOffer { offer, reason } => {
                let o = self.offer(*offer).unwrap();
                let s = o.settlement().unwrap();
                let tx = Transaction {
                    id: event.sequence,
                    actor: event.actor,
                    created_at: event.timestamp,
                    from: s.payee,
                    to: s.payer,
                    amount: o.amount,
                    memo: reason.clone(),
                    reverses: Some(s.transaction),
                    reversed: false,
                    listing: Some(o.listing),
                    usd: false,
                    quote: None,
                    loan: None,
                    lotto: None,
                    economic: self
                        .history
                        .iter()
                        .find(|t| t.id == s.transaction)
                        .map(|t| t.economic)
                        .unwrap_or_default(),
                };
                let listing_id = o.listing;
                if let Some(original) = self.history.iter_mut().find(|t| t.id == s.transaction) {
                    original.reversed = true;
                }
                self.mark_offer_reversed(s.transaction, now);
                let listing = self
                    .listings
                    .iter_mut()
                    .find(|l| l.id == listing_id)
                    .unwrap();
                listing.updated_at = now;
                listing.status = ListingStatus::Active;
                listing.buyer = None;
                listing.sold_tx = None;
                self.record_transaction(tx);
            }
            Command::DeclineOffer { offer } | Command::WithdrawOffer { offer } => {
                let o = self.offers.iter_mut().find(|o| o.id == *offer).unwrap();
                o.phase = if matches!(event.command, Command::DeclineOffer { .. }) {
                    OfferPhase::Declined
                } else {
                    OfferPhase::Withdrawn
                };
                o.updated_at = now;
            }
            _ => unreachable!("validated offer command"),
        }
    }

    pub(crate) fn mark_offer_reversed(&mut self, transaction: u64, now: u64) {
        for o in &mut self.offers {
            if let OfferPhase::Accepted(s) = o.phase {
                if s.transaction == transaction {
                    o.phase = OfferPhase::Reversed(s);
                    o.updated_at = now;
                }
            }
        }
    }
}
