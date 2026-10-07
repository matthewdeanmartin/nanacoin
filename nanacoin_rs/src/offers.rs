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
    pub economic: EconomicDetails,
    pub epoch: u64,
    pub original_amount: i64,
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
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::offer_projection(self, now, None)[9] != 0
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            now >= MIN_CLOCK && matches!(self.phase, OfferPhase::Accepted(s) if now < s.settles_at)
        }
    }
    pub fn status(&self, now: u64) -> &'static str {
        #[cfg(feature = "cobol-core")]
        {
            match crate::cobol::offer_projection(self, now, None)[8] {
                0 => "OPEN",
                1 => "ACCEPTED",
                2 => "DECLINED",
                3 => "WITHDRAWN",
                4 => "REVERSED",
                5 => "SETTLED",
                _ => unreachable!("valid offer phase"),
            }
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            match self.phase {
                OfferPhase::Open => "OPEN",
                OfferPhase::Accepted(s) if now >= s.settles_at => "SETTLED",
                OfferPhase::Accepted(_) => "ACCEPTED",
                OfferPhase::Declined => "DECLINED",
                OfferPhase::Withdrawn => "WITHDRAWN",
                OfferPhase::Reversed(_) => "REVERSED",
            }
        }
    }
    pub fn visible_to(&self, member: &Member) -> bool {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::offer_projection(self, 0, Some(member))[10] != 0
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            !member.disabled
                && (member.role == Role::Nana
                    || member.id == self.owner
                    || member.id == self.offerer)
        }
    }
    pub(crate) fn recyclable(&self, now: u64) -> bool {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::offer_projection(self, now, None)[11] != 0
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            match self.phase {
                OfferPhase::Open => false,
                OfferPhase::Accepted(s) => now >= s.settles_at,
                _ => true,
            }
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
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::recyclable(
                0,
                listing.status == ListingStatus::Active,
                self.offers
                    .iter()
                    .filter(|o| o.listing == listing.id)
                    .any(|o| crate::cobol::offer_projection(o, now, None)[12] != 0),
            )
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            listing.status != ListingStatus::Active
                && !self.offers.iter().any(|o| {
                    o.listing == listing.id
                        && matches!(o.phase, OfferPhase::Accepted(s) if now < s.settles_at)
                })
        }
    }

    pub(crate) fn validate_offer(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.offer_plan(actor, command, now).map(|_| ())
        }
        #[cfg(not(feature = "cobol-core"))]
        {
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
                    if l.is_good_deed() && self.member(actor)?.kind == MemberKind::Bot {
                        return Err(Error::BotGoodDeed);
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
                    let (payer, payee) = settlement_parties(l, o);
                    if l.economic.kind == EconomicKind::Labor
                        && self.member(payee)?.role == Role::Nana
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
                    // A good deed's payout came from issuance and never closed its listing.
                    if s.payer != MemberId(0)
                        && self.listing(o.listing)?.status != ListingStatus::Sold
                    {
                        return Err(Error::Conflict);
                    }
                    self.correction_room(s.transaction)?;
                    if self.ledger.refunded(s.transaction) != 0
                        || self.ledger.reversed_by(s.transaction).is_some()
                    {
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
    }

    pub(crate) fn apply_offer(&mut self, event: &Event) {
        let now = event.timestamp;
        #[cfg(feature = "cobol-core")]
        let plan = self
            .prepared
            .offer
            .take()
            .expect("prepared offer transition");
        match &event.command {
            Command::MakeOffer {
                listing,
                amount,
                message,
            } => {
                let owner = self.listing(*listing).unwrap().owner;
                if self.offers.is_full() {
                    #[cfg(feature = "cobol-core")]
                    let index = plan.recycle.expect("prepared offer recycling index");
                    #[cfg(not(feature = "cobol-core"))]
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
                        phase: {
                            #[cfg(feature = "cobol-core")]
                            {
                                plan.phase(None)
                            }
                            #[cfg(not(feature = "cobol-core"))]
                            {
                                OfferPhase::Open
                            }
                        },
                    })
                    .unwrap();
            }
            Command::AcceptOffer { offer } => {
                let o = self.offer(*offer).unwrap();
                let l = self.listing(o.listing).unwrap();
                #[cfg(feature = "cobol-core")]
                let (payer, payee) = (plan.payer, plan.payee);
                #[cfg(not(feature = "cobol-core"))]
                let (payer, payee) = settlement_parties(l, o);
                #[cfg(not(feature = "cobol-core"))]
                let good_deed = l.is_good_deed();
                let tx = Transaction {
                    meta: crate::ledger::TransactionMeta::default(),
                    id: event.sequence,
                    actor: event.actor,
                    created_at: event.timestamp,
                    from: payer,
                    to: payee,
                    amount: o.amount,
                    memo: TransactionMemo::try_from(l.title.as_str()).unwrap(),
                    reverses: None,
                    listing: Some(l.id),
                    usd: false,
                    quote: None,
                    loan: None,
                    lotto: None,
                    economic: l.economic,
                };
                let listing_id = l.id;
                let o = self.offers.iter_mut().find(|o| o.id == *offer).unwrap();
                let settlement = Settlement {
                    economic: tx.economic,
                    epoch: self.money_epoch,
                    original_amount: tx.amount,
                    transaction: event.sequence,
                    payer,
                    payee,
                    settles_at: {
                        #[cfg(feature = "cobol-core")]
                        {
                            plan.settles_at
                        }
                        #[cfg(not(feature = "cobol-core"))]
                        {
                            now + self.offer_settles_after
                        }
                    },
                };
                #[cfg(feature = "cobol-core")]
                {
                    o.phase = plan.phase(Some(settlement));
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    o.phase = OfferPhase::Accepted(settlement);
                }
                o.updated_at = now;
                let listing = self
                    .listings
                    .iter_mut()
                    .find(|l| l.id == listing_id)
                    .unwrap();
                listing.updated_at = now;
                #[cfg(feature = "cobol-core")]
                let mutate_listing = plan.mutate_listing;
                #[cfg(not(feature = "cobol-core"))]
                let mutate_listing = !good_deed;
                if mutate_listing {
                    listing.status = ListingStatus::Sold;
                    listing.buyer = Some(payer);
                    listing.sold_tx = Some(event.sequence);
                }
                self.record_transaction(tx);
            }
            Command::UnacceptOffer { offer, reason } => {
                let o = self.offer(*offer).unwrap();
                let s = o.settlement().unwrap();
                let tx = Transaction {
                    meta: crate::ledger::TransactionMeta {
                        refund_units: s.original_amount,
                        original_amount: s.original_amount,
                        ..Default::default()
                    },
                    id: event.sequence,
                    actor: event.actor,
                    created_at: event.timestamp,
                    from: s.payee,
                    to: s.payer,
                    amount: o.amount,
                    memo: reason.clone(),
                    reverses: Some(s.transaction),
                    listing: Some(o.listing),
                    usd: false,
                    quote: None,
                    loan: None,
                    lotto: None,
                    economic: s.economic,
                };
                let listing_id = o.listing;

                self.mark_offer_reversed(s.transaction, now);
                #[cfg(feature = "cobol-core")]
                {
                    self.offers
                        .iter_mut()
                        .find(|o| o.id == *offer)
                        .unwrap()
                        .phase = plan.phase(Some(s));
                }
                // A good deed retires its reward; its listing stayed open (or was
                // since cancelled by Nana) and is left as it is.
                #[cfg(feature = "cobol-core")]
                let mutate_listing = plan.mutate_listing;
                #[cfg(not(feature = "cobol-core"))]
                let mutate_listing = s.payer != MemberId(0);
                if mutate_listing {
                    let listing = self
                        .listings
                        .iter_mut()
                        .find(|l| l.id == listing_id)
                        .unwrap();
                    listing.updated_at = now;
                    listing.status = ListingStatus::Active;
                    listing.buyer = None;
                    listing.sold_tx = None;
                }
                self.record_transaction(tx);
            }
            Command::DeclineOffer { offer } | Command::WithdrawOffer { offer } => {
                let o = self.offers.iter_mut().find(|o| o.id == *offer).unwrap();
                #[cfg(feature = "cobol-core")]
                {
                    o.phase = plan.phase(None);
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    o.phase = if matches!(event.command, Command::DeclineOffer { .. }) {
                        OfferPhase::Declined
                    } else {
                        OfferPhase::Withdrawn
                    };
                }
                o.updated_at = now;
            }
            _ => unreachable!("validated offer command"),
        }
    }

    pub(crate) fn mark_offer_reversed(&mut self, transaction: u64, now: u64) {
        #[cfg(feature = "cobol-core")]
        {
            let _ = transaction;
            for (index, offer) in self.offers.iter_mut().enumerate() {
                if self.prepared.reversed_offers & (1 << index) != 0 {
                    let OfferPhase::Accepted(s) = offer.phase else {
                        unreachable!("prepared accepted offer link")
                    };
                    offer.phase = OfferPhase::Reversed(s);
                    offer.updated_at = now;
                }
            }
        }
        #[cfg(not(feature = "cobol-core"))]
        {
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
}

/// Who pays whom when an offer is accepted. Good deeds pay from issuance.
#[cfg(not(feature = "cobol-core"))]
pub(crate) fn settlement_parties(listing: &Listing, offer: &Offer) -> (MemberId, MemberId) {
    match listing.side {
        _ if listing.is_good_deed() => (MemberId(0), offer.offerer),
        Side::Sell => (offer.offerer, offer.owner),
        Side::Buy => (offer.owner, offer.offerer),
    }
}
