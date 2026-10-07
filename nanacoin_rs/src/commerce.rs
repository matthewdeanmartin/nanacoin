//! Bounded gift requests and externally hosted, unique digital art editions.
use crate::domain::*;
use serde::{Deserialize, Serialize};

pub const REQUESTS: usize = 32;
pub const ARTWORKS: usize = 64;
pub type Locator = heapless::String<192>;
pub type Digest = heapless::String<64>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GiftRequest {
    pub id: u64,
    pub owner: MemberId,
    pub title: Title,
    pub description: Memo,
    pub target: Option<i64>,
    /// Net gifts received in the current currency epoch.
    pub received: i64,
    pub deadline: Option<u64>,
    pub closed: bool,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artwork {
    pub id: u64,
    pub creator: MemberId,
    pub owner: MemberId,
    pub title: Title,
    pub license: Memo,
    pub sha256: Digest,
    pub locator: Locator,
    pub price: Option<i64>,
    pub equipped: bool,
    /// Changes on every ownership or listing mutation, preventing stale purchases.
    pub revision: u64,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commerce {
    pub requests: std::vec::Vec<GiftRequest>,
    pub artworks: std::vec::Vec<Artwork>,
}
impl Default for Commerce {
    fn default() -> Self {
        Self {
            requests: std::vec::Vec::with_capacity(REQUESTS),
            artworks: std::vec::Vec::with_capacity(ARTWORKS),
        }
    }
}
impl Commerce {
    pub fn clear(&mut self) {
        self.requests.clear();
        self.artworks.clear();
    }
    pub fn validate_rescale(&self, exponent: i16) -> Result<(), Error> {
        for r in &self.requests {
            if let Some(v) = r.target {
                crate::money::rescale(v, exponent, MAX_AMOUNT)?;
            }
            crate::money::rescale(r.received, exponent, MAX_SEQUENCE as i64)?;
        }
        for a in &self.artworks {
            if let Some(v) = a.price {
                crate::money::rescale(v, exponent, MAX_AMOUNT)?;
            }
        }
        Ok(())
    }
    /// Used for both reform preview validation and application. Caller supplies exact scaling.
    pub fn rescale(
        &mut self,
        mut scale: impl FnMut(i64) -> Result<i64, Error>,
    ) -> Result<(), Error> {
        for r in &mut self.requests {
            r.target = r.target.map(&mut scale).transpose()?;
            r.received = scale(r.received)?;
        }
        for a in &mut self.artworks {
            a.price = a.price.map(&mut scale).transpose()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
// Bounded inline variants preserve allocation-free command/replay paths.
#[allow(clippy::large_enum_variant)]
pub enum Action {
    CreateRequest {
        title: Title,
        description: Memo,
        target: Option<i64>,
        deadline: Option<u64>,
    },
    CloseRequest {
        request: u64,
    },
    Contribute {
        request: u64,
        amount: i64,
        memo: Memo,
    },
    MintArt {
        title: Title,
        license: Memo,
        sha256: Digest,
        locator: Locator,
    },
    ListArt {
        art: u64,
        price: Option<i64>,
    },
    BuyArt {
        art: u64,
        expected_owner: MemberId,
        expected_revision: u64,
        expected_price: i64,
    },
    GiftArt {
        art: u64,
        to: MemberId,
    },
    EquipArt {
        art: u64,
        equipped: bool,
    },
}
#[cfg(not(feature = "cobol-core"))]
fn positive(n: i64) -> bool {
    (1..=MAX_AMOUNT).contains(&n)
}
#[cfg(not(feature = "cobol-core"))]
fn valid_asset(title: &str, license: &str, hash: &str, url: &str) -> bool {
    #[cfg(not(feature = "cobol-core"))]
    {
        !title.trim().is_empty()
            && !license.trim().is_empty()
            && hash.len() == 64
            && hash.bytes().all(|c| c.is_ascii_hexdigit())
            && url.strip_prefix("https://").is_some_and(|s| {
                !s.is_empty() && !s.starts_with('/') && !s.starts_with('?') && !s.starts_with('#')
            })
            && url.bytes().all(|c| c.is_ascii_graphic())
            && !url.contains('@')
    }
}
impl State {
    pub(crate) fn commerce_refund(&mut self, original: &Transaction, _current_amount: i64) {
        if let Some(id) = original.meta.gift_request {
            let r = self
                .commerce
                .requests
                .iter_mut()
                .find(|r| r.id == id)
                .expect("retained gift request");
            #[cfg(feature = "cobol-core")]
            {
                let (request, received) = self
                    .prepared
                    .gift_refund
                    .take()
                    .expect("prepared gift refund");
                assert_eq!(request, id, "prepared gift refund request");
                r.received = received;
            }
            #[cfg(not(feature = "cobol-core"))]
            {
                r.received = r
                    .received
                    .checked_sub(_current_amount)
                    .expect("validated gift refund");
            }
        }
    }
    pub fn gift_request(&self, id: u64) -> Result<&GiftRequest, Error> {
        self.commerce
            .requests
            .iter()
            .find(|r| r.id == id)
            .ok_or(Error::NotFound)
    }
    pub fn artwork(&self, id: u64) -> Result<&Artwork, Error> {
        self.commerce
            .artworks
            .iter()
            .find(|a| a.id == id)
            .ok_or(Error::NotFound)
    }
    pub(crate) fn validate_commerce(
        &self,
        actor: MemberId,
        action: &Action,
        now: u64,
    ) -> Result<(), Error> {
        if self.member(actor)?.disabled {
            return Err(Error::Disabled);
        }
        match action {
            #[cfg(feature = "cobol-core")]
            Action::MintArt { .. }
            | Action::BuyArt { .. }
            | Action::ListArt { .. }
            | Action::GiftArt { .. }
            | Action::EquipArt { .. } => {
                self.art_plan(actor, action, self.sequence)?;
            }
            #[cfg(feature = "cobol-core")]
            Action::CreateRequest { .. }
            | Action::CloseRequest { .. }
            | Action::Contribute { .. } => {
                self.gift_plan(actor, action, now)?;
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::CreateRequest {
                title,
                target,
                deadline,
                ..
            } => {
                if self.commerce.requests.len() >= REQUESTS {
                    return Err(Error::Capacity);
                }
                if title.trim().is_empty()
                    || target.is_some_and(|v| !positive(v))
                    || deadline.is_some_and(|v| {
                        v <= now || v > MAX_SEQUENCE || now < crate::offers::MIN_CLOCK
                    })
                {
                    return Err(Error::InvalidInput);
                }
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::CloseRequest { request } => {
                let r = self.gift_request(*request)?;
                if r.owner != actor {
                    return Err(Error::Forbidden);
                }
                if r.closed {
                    return Err(Error::Conflict);
                }
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::Contribute {
                request, amount, ..
            } => {
                let r = self.gift_request(*request)?;
                if r.closed
                    || r.deadline
                        .is_some_and(|v| now >= v || now < crate::offers::MIN_CLOCK)
                {
                    return Err(Error::Conflict);
                }
                self.validate_posting(actor, r.owner, *amount, false)?;
                r.received
                    .checked_add(*amount)
                    .filter(|v| *v <= MAX_SEQUENCE as i64)
                    .ok_or(Error::Overflow)?;
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::MintArt {
                title,
                license,
                sha256,
                locator,
            } => {
                if self.commerce.artworks.len() >= ARTWORKS {
                    return Err(Error::Capacity);
                }
                if !valid_asset(title, license, sha256, locator) {
                    return Err(Error::InvalidInput);
                }
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::BuyArt {
                art,
                expected_owner,
                expected_revision,
                expected_price,
            } => {
                let a = self.artwork(*art)?;
                if a.owner != *expected_owner
                    || a.revision != *expected_revision
                    || a.price != Some(*expected_price)
                {
                    return Err(Error::Conflict);
                }
                self.validate_posting(actor, a.owner, *expected_price, false)?;
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::ListArt { art, price } => {
                if self.artwork(*art)?.owner != actor {
                    return Err(Error::Forbidden);
                }
                if price.is_some_and(|v| !positive(v)) {
                    return Err(Error::InvalidInput);
                }
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::GiftArt { art, to } => {
                if self.artwork(*art)?.owner != actor {
                    return Err(Error::Forbidden);
                }
                if *to == actor {
                    return Err(Error::InvalidInput);
                }
                if self.member(*to)?.disabled {
                    return Err(Error::Disabled);
                }
            }
            #[cfg(not(feature = "cobol-core"))]
            Action::EquipArt { art, .. } => {
                if self.artwork(*art)?.owner != actor {
                    return Err(Error::Forbidden);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn apply_commerce(&mut self, event: &Event, action: &Action) {
        #[cfg(feature = "cobol-core")]
        let gift = self.prepared.gift.take();
        #[cfg(feature = "cobol-core")]
        let art_plan = self.prepared.art.take();
        match action {
            Action::CreateRequest {
                title,
                description,
                target,
                deadline,
            } => self.commerce.requests.push(GiftRequest {
                id: event.sequence,
                owner: {
                    #[cfg(feature = "cobol-core")]
                    {
                        gift.as_ref().expect("prepared request").owner
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        event.actor
                    }
                },
                title: title.clone(),
                description: description.clone(),
                target: *target,
                received: {
                    #[cfg(feature = "cobol-core")]
                    {
                        gift.as_ref().expect("prepared request").received
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        0
                    }
                },
                deadline: *deadline,
                closed: {
                    #[cfg(feature = "cobol-core")]
                    {
                        gift.as_ref().expect("prepared request").closed
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        false
                    }
                },
                created_at: event.timestamp,
            }),
            Action::CloseRequest { request } => {
                let r = self
                    .commerce
                    .requests
                    .iter_mut()
                    .find(|r| r.id == *request)
                    .unwrap();
                #[cfg(feature = "cobol-core")]
                {
                    r.closed = gift.as_ref().expect("prepared closure").closed;
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    r.closed = true;
                }
            }
            Action::Contribute {
                request,
                amount,
                memo,
            } => {
                let r = self
                    .commerce
                    .requests
                    .iter_mut()
                    .find(|r| r.id == *request)
                    .unwrap();
                #[cfg(feature = "cobol-core")]
                let to = {
                    let p = gift.as_ref().expect("prepared contribution");
                    r.received = p.received;
                    p.owner
                };
                #[cfg(not(feature = "cobol-core"))]
                let to = {
                    r.received += amount;
                    r.owner
                };
                self.commerce_posting(
                    event,
                    to,
                    *amount,
                    memo.as_str(),
                    EconomicKind::Gift,
                    Some(*request),
                    None,
                );
            }
            Action::MintArt {
                title,
                license,
                sha256,
                locator,
            } => self.commerce.artworks.push(Artwork {
                id: event.sequence,
                creator: event.actor,
                owner: {
                    #[cfg(feature = "cobol-core")]
                    {
                        art_plan.as_ref().expect("prepared mint").owner
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        event.actor
                    }
                },
                title: title.clone(),
                license: license.clone(),
                sha256: sha256.clone(),
                locator: locator.clone(),
                price: {
                    #[cfg(feature = "cobol-core")]
                    {
                        art_plan.as_ref().expect("prepared mint").price
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        None
                    }
                },
                equipped: {
                    #[cfg(feature = "cobol-core")]
                    {
                        art_plan.as_ref().expect("prepared mint").equipped
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        false
                    }
                },
                revision: {
                    #[cfg(feature = "cobol-core")]
                    {
                        art_plan.as_ref().expect("prepared mint").revision
                    }
                    #[cfg(not(feature = "cobol-core"))]
                    {
                        event.sequence
                    }
                },
                created_at: event.timestamp,
            }),
            Action::ListArt { art, price: _price } => {
                let a = self
                    .commerce
                    .artworks
                    .iter_mut()
                    .find(|a| a.id == *art)
                    .unwrap();
                #[cfg(feature = "cobol-core")]
                art_plan.as_ref().expect("prepared art listing").install(a);
                #[cfg(not(feature = "cobol-core"))]
                {
                    a.price = *_price;
                    a.revision = event.sequence;
                }
            }
            Action::BuyArt {
                art,
                expected_price: _expected_price,
                ..
            } => {
                let a = self
                    .commerce
                    .artworks
                    .iter_mut()
                    .find(|a| a.id == *art)
                    .unwrap();
                #[cfg(feature = "cobol-core")]
                let (seller, amount) = {
                    let p = art_plan.as_ref().expect("prepared art purchase");
                    p.install(a);
                    (p.seller, p.amount)
                };
                #[cfg(not(feature = "cobol-core"))]
                let (seller, amount) = {
                    let seller = a.owner;
                    a.owner = event.actor;
                    a.price = None;
                    a.equipped = false;
                    a.revision = event.sequence;
                    (seller, *_expected_price)
                };
                self.commerce_posting(
                    event,
                    seller,
                    amount,
                    "Digital art purchase",
                    EconomicKind::Good,
                    None,
                    Some(*art),
                );
            }
            Action::GiftArt { art, to: _to } => {
                let a = self
                    .commerce
                    .artworks
                    .iter_mut()
                    .find(|a| a.id == *art)
                    .unwrap();
                #[cfg(feature = "cobol-core")]
                art_plan.as_ref().expect("prepared art gift").install(a);
                #[cfg(not(feature = "cobol-core"))]
                {
                    a.owner = *_to;
                    a.price = None;
                    a.equipped = false;
                    a.revision = event.sequence;
                }
            }
            Action::EquipArt {
                art,
                equipped: _equipped,
            } => {
                #[cfg(feature = "cobol-core")]
                {
                    let p = art_plan.as_ref().expect("prepared equipment");
                    for (index, a) in self.commerce.artworks.iter_mut().enumerate() {
                        if p.clear_equipment & (1u64 << index) != 0 {
                            a.equipped = false;
                        }
                    }
                    p.install(
                        self.commerce
                            .artworks
                            .iter_mut()
                            .find(|a| a.id == *art)
                            .unwrap(),
                    );
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    if *_equipped {
                        for a in &mut self.commerce.artworks {
                            if a.owner == event.actor {
                                a.equipped = false;
                            }
                        }
                    }
                    self.commerce
                        .artworks
                        .iter_mut()
                        .find(|a| a.id == *art)
                        .unwrap()
                        .equipped = *_equipped;
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn commerce_posting(
        &mut self,
        event: &Event,
        to: MemberId,
        amount: i64,
        memo: &str,
        kind: EconomicKind,
        gift_request: Option<u64>,
        art: Option<u64>,
    ) {
        self.record_transaction(Transaction {
            id: event.sequence,
            actor: event.actor,
            created_at: event.timestamp,
            from: event.actor,
            to,
            amount,
            memo: TransactionMemo::try_from(memo).unwrap(),
            reverses: None,
            listing: None,
            usd: false,
            quote: None,
            loan: None,
            lotto: None,
            economic: EconomicDetails {
                kind,
                ..Default::default()
            },
            meta: crate::ledger::TransactionMeta {
                gift_request,
                art,
                ..Default::default()
            },
        });
    }
    pub(crate) fn check_commerce(&self) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        return self.bank_commerce_invariants();
        #[cfg(not(feature = "cobol-core"))]
        {
            if self.commerce.requests.len() > REQUESTS || self.commerce.artworks.len() > ARTWORKS {
                return Err(Error::CorruptJournal);
            }
            for (i, r) in self.commerce.requests.iter().enumerate() {
                self.member(r.owner)?;
                if r.id == 0
                    || r.id > self.sequence
                    || r.received < 0
                    || r.received > MAX_SEQUENCE as i64
                    || r.target.is_some_and(|v| !positive(v))
                    || r.title.trim().is_empty()
                    || self.commerce.requests[..i].iter().any(|p| p.id == r.id)
                {
                    return Err(Error::CorruptJournal);
                }
            }
            for (i, a) in self.commerce.artworks.iter().enumerate() {
                self.member(a.owner)?;
                self.member(a.creator)?;
                if a.id == 0
                    || a.id > self.sequence
                    || a.revision < a.id
                    || a.revision > self.sequence
                    || a.price.is_some_and(|v| !positive(v))
                    || !valid_asset(&a.title, &a.license, &a.sha256, &a.locator)
                    || self.commerce.artworks[..i]
                        .iter()
                        .any(|p| p.id == a.id || (p.owner == a.owner && p.equipped && a.equipped))
                {
                    return Err(Error::CorruptJournal);
                }
            }
            Ok(())
        }
    }
}
