use crate::auth::PasswordVerifier;
use crate::forex::{Quote, QuoteSide, QUOTES};
use crate::offers::{Offer, OfferId, OfferMessage, DEFAULT_SETTLEMENT, OFFERS};
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use subtle::ConstantTimeEq;

pub const MEMBERS: usize = 16;
pub const LISTINGS: usize = 48;
pub const HISTORY: usize = 365;
pub const THINGS: usize = 64;
pub const MAX_AMOUNT: i64 = 1_000_000_000_000_000;
pub const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;
/// Preserve legacy cash-leg IDs (event + 4096), reserving alternating blocks
/// for cash legs as the event journal grows beyond its original capacity.
pub fn next_sequence(sequence: u64) -> Option<u64> {
    sequence
        .checked_add(if sequence % 8192 == 4096 { 4097 } else { 1 })
        .filter(|s| *s <= MAX_SEQUENCE - 4096)
}
pub type Name = String<40>;
pub type MastodonId = String<128>;
pub type Memo = String<96>;
/// Ledger text also holds the TinyGo offer-undo reason (up to 140 bytes).
pub type TransactionMemo = String<140>;
pub type Title = String<80>;
pub type TokenHash = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidInput,
    Unauthorized,
    Forbidden,
    NotFound,
    Capacity,
    InsufficientFunds,
    Overflow,
    Conflict,
    StaleRequest,
    Storage,
    CorruptJournal,
    InvalidCredentials,
    Disabled,
    RateLimited,
    Unavailable,
    OfferClosed,
    OfferSettled,
    ListingClosed,
    SelfDeal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    CreateLotto {
        terms: crate::lotto::LottoTerms,
    },
    BuyTickets {
        lotto: u64,
        count: u32,
    },
    RunLotto {
        lotto: u64,
        step: u8,
        ticket: u64,
    },
    OfferLoan {
        terms: crate::loans::LoanTerms,
    },
    AcceptLoan {
        loan: u64,
    },
    CloseLoan {
        loan: u64,
    },
    RepayLoan {
        loan: u64,
        amount: i64,
    },
    /// Internal only. HTTP actors can never submit scheduler occurrences.
    RunLoan {
        loan: u64,
        expected_updated_at: u64,
    },
    ReformCurrency {
        decimals: u8,
        power: i8,
        expected_epoch: u64,
        expected_sequence: u64,
    },
    /// Historical journal format only. Never accepted by the HTTP API.
    AddMember {
        name: Name,
        token_hash: TokenHash,
    },
    Provision {
        household_name: Name,
        username: Name,
        display_name: Name,
        password: PasswordVerifier,
    },
    CreateMember {
        username: Name,
        display_name: Name,
        password: PasswordVerifier,
        role: Role,
        #[serde(default)]
        grant: i64,
        #[serde(default)]
        mastodon_id: MastodonId,
    },
    UpdateMember {
        member: MemberId,
        display_name: Option<Name>,
        password: Option<PasswordVerifier>,
        role: Option<Role>,
        disabled: Option<bool>,
        #[serde(default)]
        mastodon_id: Option<MastodonId>,
    },
    /// Local-only upgrade of an existing token account; preserves its identity.
    MigrateMember {
        member: MemberId,
        username: Name,
        password: PasswordVerifier,
    },
    Issue {
        to: MemberId,
        amount: i64,
        memo: Memo,
    },
    Retire {
        from: MemberId,
        amount: i64,
        memo: Memo,
    },
    Transfer {
        to: MemberId,
        amount: i64,
        memo: Memo,
    },
    ClassifiedTransfer {
        to: MemberId,
        amount: i64,
        memo: Memo,
        economic: EconomicDetails,
    },
    List {
        title: Title,
        #[serde(default)]
        description: Memo,
        price: i64,
        side: Side,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<ListingDetails>,
    },
    ClassifiedList {
        title: Title,
        #[serde(default)]
        description: Memo,
        price: i64,
        side: Side,
        #[serde(default)]
        economic: EconomicDetails,
        #[serde(default)]
        standard: bool,
    },
    IssueUsd {
        to: MemberId,
        cents: i64,
        memo: Memo,
    },
    PostQuote {
        side: QuoteSide,
        cents_per_coin: i64,
        coins: i64,
        expires_at: u64,
    },
    TakeQuote {
        quote: u64,
    },
    CancelQuote {
        quote: u64,
    },
    Cancel {
        listing: u64,
    },
    Buy {
        listing: u64,
    },
    Reverse {
        transaction: u64,
        memo: Memo,
    },
    Configure {
        household_name: Name,
        initial_grant: i64,
        currency: Name,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offer_settles_after: Option<u64>,
    },
    UpdateListing {
        listing: u64,
        title: Option<Title>,
        description: Option<Memo>,
        price: Option<i64>,
    },
    MakeOffer {
        listing: u64,
        amount: i64,
        message: OfferMessage,
    },
    AcceptOffer {
        offer: OfferId,
    },
    UnacceptOffer {
        offer: OfferId,
        reason: TransactionMemo,
    },
    DeclineOffer {
        offer: OfferId,
    },
    WithdrawOffer {
        offer: OfferId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Nana,
    User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Sell,
    Buy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EconomicKind {
    Labor,
    Good,
    Gift,
    /// Lending cash flows are financial transfers, not household production.
    LoanPrincipal,
    Interest,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Unit {
    #[default]
    Each,
    Batch,
    Task,
    Minute,
    Hour,
    Gram,
    Kilogram,
    Milliliter,
    Liter,
    Load,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicDetails {
    #[serde(default)]
    pub kind: EconomicKind,
    #[serde(default)]
    pub thing: u64,
    /// Exact thousandths of the selected unit. Zero means legacy/unspecified.
    #[serde(default)]
    pub quantity_milli: u32,
    #[serde(default)]
    pub unit: Unit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thing {
    pub id: u64,
    pub name: Title,
    pub kind: EconomicKind,
    pub unit: Unit,
    pub standard: bool,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
    pub id: MemberId,
    pub name: Name,
    pub username: Name,
    #[serde(default)]
    pub mastodon_id: MastodonId,
    pub role: Role,
    pub disabled: bool,
    #[serde(skip)]
    pub(crate) password: Option<PasswordVerifier>,
    pub balance: i64,
    pub usd_cents: i64,
    pub created_at: u64,
    pub last_request: u64,
    #[serde(skip)]
    pub(crate) token_hash: TokenHash,
    #[serde(skip)]
    pub(crate) last_command: TokenHash,
    #[serde(skip)]
    pub(crate) last_sequence: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListingDetails {
    #[serde(default)]
    pub kind: String<16>,
    #[serde(default)]
    pub currency: String<8>,
    #[serde(default)]
    pub minor_units: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Listing {
    pub id: u64,
    pub owner: MemberId,
    pub title: Title,
    pub description: Memo,
    pub price: i64,
    pub side: Side,
    pub status: ListingStatus,
    pub buyer: Option<MemberId>,
    pub sold_tx: Option<u64>,
    pub details: ListingDetails,
    #[serde(default)]
    pub economic: EconomicDetails,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: u64,
    pub actor: MemberId,
    pub created_at: u64,
    /// Corrections may overdraw a member; ordinary spending may not.
    pub from: MemberId,
    pub to: MemberId,
    pub amount: i64,
    pub memo: TransactionMemo,
    pub reverses: Option<u64>,
    pub reversed: bool,
    pub listing: Option<u64>,
    pub usd: bool,
    pub quote: Option<u64>,
    #[serde(default)]
    pub loan: Option<u64>,
    pub lotto: Option<u64>,
    #[serde(default)]
    pub economic: EconomicDetails,
}

#[derive(Debug, Serialize)]
pub struct State {
    pub decimals: u8,
    pub money_epoch: u64,
    /// Debt payments cannot immediately cause more automatic borrowing.
    pub(crate) credit_blocked: u16,
    #[serde(skip)]
    pub loans: Vec<crate::loans::Loan, { crate::loans::LOANS }>,
    #[serde(skip)]
    pub lottos: Vec<crate::lotto::Lotto, { crate::lotto::LOTTOS }>,
    pub lotto_escrow: i64,
    pub household_name: Name,
    pub initial_grant: i64,
    pub currency: Name,
    pub offer_settles_after: u64,
    #[serde(skip)]
    pub offers: Vec<Offer, OFFERS>,
    #[serde(skip)]
    pub(crate) last_timestamp: u64,
    pub sequence: u64,
    pub transactions: u64,
    pub issuance_balance: i64,
    pub usd_issuance_balance: i64,
    pub quotes: Vec<Quote, QUOTES>,
    pub members: Vec<Member, MEMBERS>,
    pub listings: Vec<Listing, LISTINGS>,
    pub things: Vec<Thing, THINGS>,
    /// Oldest first. Eviction never discards balances or retry watermarks.
    pub history: VecDeque<Transaction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Server time, never supplied by a command. Old journal records omit it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_key: Option<[u8; 32]>,
    pub version: u8,
    pub sequence: u64,
    pub actor: MemberId,
    pub request_id: u64,
    pub command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Receipt {
    pub sequence: u64,
    pub replayed: bool,
}

pub fn token_hash(token: &str) -> Result<TokenHash, Error> {
    if !(32..=128).contains(&token.len()) || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(Error::InvalidInput);
    }
    Ok(Sha256::digest(token.as_bytes()).into())
}

impl Default for State {
    fn default() -> Self {
        Self {
            decimals: 4,
            money_epoch: 0,
            credit_blocked: 0,
            loans: Vec::new(),
            lottos: Vec::new(),
            lotto_escrow: 0,
            household_name: Name::new(),
            initial_grant: 1_000_000,
            currency: Name::try_from("NanaCoin").unwrap(),
            offer_settles_after: DEFAULT_SETTLEMENT,
            offers: Vec::new(),
            last_timestamp: 0,
            sequence: 0,
            transactions: 0,
            issuance_balance: 0,
            usd_issuance_balance: 0,
            quotes: Vec::new(),
            members: Vec::new(),
            listings: Vec::new(),
            things: Vec::new(),
            history: VecDeque::with_capacity(HISTORY),
        }
    }
}

impl State {
    /// Reset in place: retain fixed capacities and avoid a large stack copy.
    pub(crate) fn clear_economy(&mut self) {
        self.decimals = 4;
        self.money_epoch = 0;
        self.credit_blocked = 0;
        self.loans.clear();
        self.lottos.clear();
        self.lotto_escrow = 0;
        self.household_name.clear();
        self.initial_grant = 1_000_000;
        self.currency = Name::try_from("NanaCoin").unwrap();
        self.offer_settles_after = DEFAULT_SETTLEMENT;
        self.last_timestamp = 0;
        self.sequence = 0;
        self.transactions = 0;
        self.issuance_balance = 0;
        self.usd_issuance_balance = 0;
        self.members.clear();
        self.listings.clear();
        self.things.clear();
        self.offers.clear();
        self.quotes.clear();
        self.history.clear();
    }
    /// For the local migration tool only; never used for API authentication.
    pub fn authenticate_legacy(&self, token: &str) -> Result<MemberId, Error> {
        let hash = token_hash(token).map_err(|_| Error::Unauthorized)?;
        let mut found = None;
        for member in &self.members {
            if member.password.is_none() && bool::from(member.token_hash.ct_eq(&hash)) {
                found = Some(member.id);
            }
        }
        found.ok_or(Error::Unauthorized)
    }

    pub fn member(&self, id: MemberId) -> Result<&Member, Error> {
        self.members
            .iter()
            .find(|m| m.id == id)
            .ok_or(Error::NotFound)
    }

    pub fn thing(&self, id: u64) -> Result<&Thing, Error> {
        self.things
            .iter()
            .find(|t| t.id == id)
            .ok_or(Error::NotFound)
    }

    pub(crate) fn listing(&self, id: u64) -> Result<&Listing, Error> {
        self.listings
            .iter()
            .find(|l| l.id == id)
            .ok_or(Error::NotFound)
    }

    pub(crate) fn admin(&self, actor: MemberId) -> Result<(), Error> {
        if (self.members.is_empty() && actor == MemberId(1))
            || self
                .member(actor)
                .is_ok_and(|m| m.role == Role::Nana && !m.disabled)
        {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }

    fn posting(&self, from: MemberId, to: MemberId, amount: i64) -> Result<(), Error> {
        self.validate_posting(from, to, amount, false)
    }

    pub(crate) fn validate_posting(
        &self,
        from: MemberId,
        to: MemberId,
        amount: i64,
        correction: bool,
    ) -> Result<(), Error> {
        if from == to || !(1..=MAX_AMOUNT).contains(&amount) {
            return Err(Error::InvalidInput);
        }
        let balance = |id| {
            if id == MemberId(0) {
                Ok(self.issuance_balance)
            } else {
                let member = self.member(id)?;
                if !correction && member.disabled {
                    return Err(Error::Disabled);
                }
                Ok(member.balance)
            }
        };
        let debit = balance(from)?.checked_sub(amount).ok_or(Error::Overflow)?;
        let credit = balance(to)?.checked_add(amount).ok_or(Error::Overflow)?;
        // JSON numbers must remain exact for the browser as well as Rust.
        if debit < -(MAX_SEQUENCE as i64) || credit > MAX_SEQUENCE as i64 {
            return Err(Error::Overflow);
        }
        if !correction && from != MemberId(0) && debit < 0 {
            return Err(Error::InsufficientFunds);
        }
        Ok(())
    }

    pub fn validate(&self, actor: MemberId, command: &Command) -> Result<(), Error> {
        self.validate_at(actor, command, self.last_timestamp)
    }

    pub(crate) fn validate_at(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<(), Error> {
        if matches!(command, Command::RunLotto { .. }) {
            return self.validate_lotto(actor, command, now);
        }
        if matches!(command, Command::RunLoan { .. }) {
            return self.validate_loan(actor, command, now);
        }
        if !self.members.is_empty() {
            if self.member(actor)?.disabled {
                return Err(Error::Disabled);
            }
        } else if actor != MemberId(1)
            || !matches!(
                command,
                Command::AddMember { .. } | Command::Provision { .. }
            )
        {
            return Err(Error::Forbidden);
        }
        match command {
            Command::CreateLotto { .. } | Command::BuyTickets { .. } | Command::RunLotto { .. } => {
                self.validate_lotto(actor, command, now)?
            }
            Command::OfferLoan { .. }
            | Command::AcceptLoan { .. }
            | Command::CloseLoan { .. }
            | Command::RepayLoan { .. }
            | Command::RunLoan { .. } => self.validate_loan(actor, command, now)?,
            Command::ReformCurrency {
                decimals,
                power,
                expected_epoch,
                expected_sequence,
            } => {
                self.admin(actor)?;
                if *expected_epoch != self.money_epoch || *expected_sequence != self.sequence {
                    return Err(Error::Conflict);
                }
                self.validate_reform(*decimals, *power)?;
            }
            Command::Provision {
                household_name,
                username,
                display_name,
                password,
            } => {
                if !self.members.is_empty() {
                    return Err(Error::Forbidden);
                }
                valid_name(household_name)?;
                self.validate_new_member(username, display_name, password)?;
            }
            Command::CreateMember {
                username,
                display_name,
                password,
                grant,
                mastodon_id,
                ..
            } => {
                self.admin(actor)?;
                self.validate_new_member(username, display_name, password)?;
                valid_mastodon_id(mastodon_id)?;
                if !(0..=MAX_AMOUNT).contains(grant) {
                    return Err(Error::InvalidInput);
                }
                if self
                    .issuance_balance
                    .checked_sub(*grant)
                    .is_none_or(|n| n < -(MAX_SEQUENCE as i64))
                {
                    return Err(Error::Overflow);
                }
            }
            Command::Configure {
                household_name,
                initial_grant,
                currency,
                offer_settles_after,
            } => {
                self.admin(actor)?;
                valid_name(household_name)?;
                valid_name(currency)?;
                if !(0..=MAX_AMOUNT).contains(initial_grant) {
                    return Err(Error::InvalidInput);
                }
                if offer_settles_after.is_some_and(|v| v > MAX_SEQUENCE.saturating_sub(now)) {
                    return Err(Error::InvalidInput);
                }
            }
            Command::UpdateMember {
                member,
                display_name,
                password,
                role,
                disabled,
                mastodon_id,
            } => {
                let target = self.member(*member)?;
                if target.password.is_none() && password.is_some() {
                    return Err(Error::InvalidInput);
                }
                if actor != *member || role.is_some() || disabled.is_some() {
                    self.admin(actor)?;
                }
                if let Some(name) = display_name {
                    valid_name(name)?;
                }
                if let Some(value) = mastodon_id {
                    valid_mastodon_id(value)?;
                }
                if password.as_ref().is_some_and(|p| !p.valid()) {
                    return Err(Error::InvalidInput);
                }
                if target.role == Role::Nana
                    && !target.disabled
                    && (*role == Some(Role::User) || *disabled == Some(true))
                    && self
                        .members
                        .iter()
                        .filter(|m| m.role == Role::Nana && !m.disabled)
                        .count()
                        == 1
                {
                    return Err(Error::Forbidden);
                }
            }
            Command::MigrateMember {
                member,
                username,
                password,
            } => {
                if actor != *member {
                    self.admin(actor)?;
                }
                if self.member(*member)?.password.is_some() {
                    return Err(Error::Conflict);
                }
                valid_name(username)?;
                if !password.valid() {
                    return Err(Error::InvalidInput);
                }
                if self.members.iter().any(|m| m.username == *username) {
                    return Err(Error::Conflict);
                }
            }
            Command::AddMember { name, token_hash } => {
                self.admin(actor)?;
                if name.trim().is_empty() || *token_hash == [0; 32] {
                    return Err(Error::InvalidInput);
                }
                if self.members.is_full() {
                    return Err(Error::Capacity);
                }
                if self
                    .members
                    .iter()
                    .any(|m| m.name == *name || m.token_hash == *token_hash)
                {
                    return Err(Error::Conflict);
                }
            }
            Command::Issue { to, amount, .. } => {
                self.admin(actor)?;
                self.member(*to)?;
                self.posting(MemberId(0), *to, *amount)?;
            }
            Command::Retire { from, amount, .. } => {
                self.admin(actor)?;
                self.member(*from)?;
                self.posting(*from, MemberId(0), *amount)?;
            }
            Command::Transfer { to, amount, .. } => {
                self.member(*to)?;
                self.posting(actor, *to, *amount)?;
            }
            Command::ClassifiedTransfer {
                to,
                amount,
                economic,
                ..
            } => {
                let recipient = self.member(*to)?;
                self.posting(actor, *to, *amount)?;
                valid_economic(economic)?;
                if economic.kind == EconomicKind::Labor && recipient.role == Role::Nana {
                    return Err(Error::Forbidden);
                }
                if economic.thing != 0 {
                    let thing = self.thing(economic.thing)?;
                    if thing.kind != economic.kind || thing.unit != economic.unit {
                        return Err(Error::InvalidInput);
                    }
                }
            }
            Command::List {
                title,
                price,
                details,
                ..
            } => {
                if let Some(d) = details {
                    if !["", "item", "service", "currency"].contains(&d.kind.as_str())
                        || d.currency.chars().any(char::is_control)
                        || d.minor_units.unsigned_abs() > MAX_SEQUENCE
                        || (d.kind == "currency" && (d.currency.is_empty() || d.minor_units <= 0))
                    {
                        return Err(Error::InvalidInput);
                    }
                }
                if title.trim().is_empty() || !(1..=MAX_AMOUNT).contains(price) {
                    return Err(Error::InvalidInput);
                }
                if self.listings.is_full()
                    && self
                        .listings
                        .iter()
                        .all(|l| !self.listing_recyclable(l, now))
                {
                    return Err(Error::Capacity);
                }
            }
            Command::ClassifiedList {
                title,
                price,
                side,
                economic,
                ..
            } => {
                if title.trim().is_empty() || !(1..=MAX_AMOUNT).contains(price) {
                    return Err(Error::InvalidInput);
                }
                valid_economic(economic)?;
                if *side == Side::Sell
                    && economic.kind == EconomicKind::Labor
                    && self.member(actor)?.role == Role::Nana
                {
                    return Err(Error::Forbidden);
                }
                if economic.thing != 0 {
                    let thing = self.thing(economic.thing)?;
                    if thing.kind != economic.kind || thing.unit != economic.unit {
                        return Err(Error::InvalidInput);
                    }
                } else if let Some(thing) = self
                    .things
                    .iter()
                    .find(|t| t.name.as_str().eq_ignore_ascii_case(title.as_str()))
                {
                    if thing.kind != economic.kind || thing.unit != economic.unit {
                        return Err(Error::Conflict);
                    }
                } else if self.things.is_full()
                    && !self.things.iter().any(|t| {
                        !t.standard
                            && !self.listings.iter().any(|l| {
                                l.status == ListingStatus::Active && l.economic.thing == t.id
                            })
                    })
                {
                    return Err(Error::Capacity);
                }
                if self.listings.is_full()
                    && self
                        .listings
                        .iter()
                        .all(|l| !self.listing_recyclable(l, now))
                {
                    return Err(Error::Capacity);
                }
            }
            Command::Cancel { listing } => {
                let l = self.listing(*listing)?;
                if actor != l.owner {
                    self.admin(actor)?;
                }
                if l.status != ListingStatus::Active {
                    return Err(Error::Conflict);
                }
            }
            Command::UpdateListing {
                listing,
                title,
                price,
                ..
            } => {
                let l = self.listing(*listing)?;
                if l.owner != actor {
                    self.admin(actor)?;
                }
                if l.status != ListingStatus::Active {
                    return Err(Error::Conflict);
                }
                if title.as_ref().is_some_and(|t| t.trim().is_empty())
                    || price.is_some_and(|p| !(1..=MAX_AMOUNT).contains(&p))
                {
                    return Err(Error::InvalidInput);
                }
            }
            Command::Buy { listing } => {
                let l = self.listing(*listing)?;
                if l.status != ListingStatus::Active {
                    return Err(Error::Conflict);
                }
                let (from, to) = match l.side {
                    Side::Sell => (actor, l.owner),
                    Side::Buy => (l.owner, actor),
                };
                if l.economic.kind == EconomicKind::Labor && self.member(to)?.role == Role::Nana {
                    return Err(Error::Forbidden);
                }
                self.posting(from, to, l.price)?;
            }
            Command::Reverse { transaction, .. } => {
                let tx = self
                    .history
                    .iter()
                    .find(|t| t.id == *transaction)
                    .ok_or(Error::NotFound)?;
                let nana = self.member(actor)?.role == Role::Nana;
                if tx.loan.is_some() || tx.lotto.is_some() {
                    return Err(Error::Forbidden);
                }
                // Nana may correct any ordinary ledger error. A member may
                // only refund money they received, and never issuance, USD,
                // or a Forex leg. That makes Refund safe to expose beside a
                // public ledger row without turning it into arbitrary spend.
                if !nana
                    && (tx.to != actor || tx.from == MemberId(0) || tx.usd || tx.quote.is_some())
                {
                    return Err(Error::Forbidden);
                }
                if tx.reversed || tx.reverses.is_some() {
                    return Err(Error::Conflict);
                }
                self.validate_currency_posting(tx.to, tx.from, tx.amount, nana, tx.usd)?;
            }
            Command::IssueUsd { .. }
            | Command::PostQuote { .. }
            | Command::TakeQuote { .. }
            | Command::CancelQuote { .. } => self.validate_forex(actor, command, now)?,
            Command::MakeOffer { .. }
            | Command::AcceptOffer { .. }
            | Command::UnacceptOffer { .. }
            | Command::DeclineOffer { .. }
            | Command::WithdrawOffer { .. } => self.validate_offer(actor, command, now)?,
        }
        Ok(())
    }

    fn validate_new_member(
        &self,
        username: &Name,
        display_name: &Name,
        password: &PasswordVerifier,
    ) -> Result<(), Error> {
        valid_name(username)?;
        valid_name(display_name)?;
        if !password.valid() {
            return Err(Error::InvalidInput);
        }
        if self.members.is_full() {
            return Err(Error::Capacity);
        }
        if self.members.iter().any(|m| m.username == *username) {
            return Err(Error::Conflict);
        }
        Ok(())
    }

    pub fn needs_login_migration(&self) -> bool {
        self.members.iter().any(|m| m.password.is_none())
    }

    /// Replay and live commits use the same validation. Mutation is private and
    /// infallible after validation, so failed storage never changes RAM.
    pub fn replay(&mut self, event: &Event) -> Result<(), Error> {
        if event.version != 2
            || Some(event.sequence) != next_sequence(self.sequence)
            || event.sequence > MAX_SEQUENCE
            || event.request_id == 0
            || event.request_id > MAX_SEQUENCE
        {
            return Err(Error::CorruptJournal);
        }
        if event.actor != MemberId(0)
            && !self.members.is_empty()
            && event.request_id <= self.member(event.actor)?.last_request
        {
            return Err(Error::CorruptJournal);
        }
        if event.timestamp < self.last_timestamp {
            return Err(Error::CorruptJournal);
        }
        self.validate_at(event.actor, &event.command, event.timestamp)?;
        self.apply(event);
        Ok(())
    }

    pub fn retry(
        &self,
        actor: MemberId,
        request_id: u64,
        command: &Command,
    ) -> Result<Option<Receipt>, Error> {
        if request_id == 0 || request_id > MAX_SEQUENCE {
            return Err(Error::InvalidInput);
        }
        if self.members.is_empty() {
            return Ok(None);
        }
        let m = self.member(actor)?;
        if request_id < m.last_request {
            return Err(Error::StaleRequest);
        }
        if request_id == m.last_request {
            if m.last_command != fingerprint(command) {
                return Err(Error::Conflict);
            }
            return Ok(Some(Receipt {
                sequence: m.last_sequence,
                replayed: true,
            }));
        }
        Ok(None)
    }

    pub(crate) fn apply(&mut self, event: &Event) {
        let actor = event.actor;
        let mut posting = None;
        let mut posting_economic = EconomicDetails::default();
        let mut usd = false;
        match &event.command {
            Command::CreateLotto { .. } | Command::BuyTickets { .. } | Command::RunLotto { .. } => {
                self.apply_lotto(event)
            }
            Command::OfferLoan { .. }
            | Command::AcceptLoan { .. }
            | Command::CloseLoan { .. }
            | Command::RepayLoan { .. }
            | Command::RunLoan { .. } => self.apply_loan(event),
            Command::ReformCurrency {
                decimals, power, ..
            } => self.apply_reform(*decimals, *power),
            Command::Provision {
                household_name,
                username,
                display_name,
                password,
            } => {
                self.household_name = household_name.clone();
                self.add_password_member(
                    username,
                    display_name,
                    password,
                    Role::Nana,
                    event.timestamp,
                );
            }
            Command::CreateMember {
                username,
                display_name,
                password,
                role,
                grant,
                mastodon_id,
            } => {
                self.add_password_member(username, display_name, password, *role, event.timestamp);
                self.members.last_mut().unwrap().mastodon_id = mastodon_id.clone();
                if *grant > 0 {
                    posting = Some((
                        MemberId(0),
                        MemberId(self.members.len() as u8),
                        *grant,
                        Memo::try_from("Initial household allocation").unwrap(),
                        None,
                        None,
                    ));
                }
            }
            Command::Configure {
                household_name,
                initial_grant,
                currency,
                offer_settles_after,
            } => {
                self.household_name = household_name.clone();
                self.initial_grant = *initial_grant;
                self.currency = currency.clone();
                if let Some(seconds) = offer_settles_after {
                    self.offer_settles_after = if *seconds == 0 {
                        DEFAULT_SETTLEMENT
                    } else {
                        *seconds
                    };
                }
            }
            Command::UpdateMember {
                member,
                display_name,
                password,
                role,
                disabled,
                mastodon_id,
            } => {
                let m = self.members.iter_mut().find(|m| m.id == *member).unwrap();
                if let Some(name) = display_name {
                    m.name = name.clone();
                }
                if let Some(password) = password {
                    m.password = Some(password.clone());
                    m.token_hash = [0; 32];
                }
                if let Some(role) = role {
                    m.role = *role;
                }
                if let Some(disabled) = disabled {
                    m.disabled = *disabled;
                }
                if let Some(value) = mastodon_id {
                    m.mastodon_id = value.clone();
                }
            }
            Command::MigrateMember {
                member,
                username,
                password,
            } => {
                if self.household_name.is_empty() {
                    self.household_name = Name::try_from("NanaCoin").unwrap();
                }
                let m = self.members.iter_mut().find(|m| m.id == *member).unwrap();
                m.username = username.clone();
                m.password = Some(password.clone());
                m.token_hash = [0; 32];
            }
            Command::AddMember { name, token_hash } => {
                let id = MemberId(self.members.len() as u8 + 1);
                self.members
                    .push(Member {
                        id,
                        name: name.clone(),
                        username: Name::new(),
                        mastodon_id: MastodonId::new(),
                        role: if id == MemberId(1) {
                            Role::Nana
                        } else {
                            Role::User
                        },
                        disabled: false,
                        password: None,
                        balance: 0,
                        usd_cents: 0,
                        created_at: event.timestamp,
                        last_request: 0,
                        token_hash: *token_hash,
                        last_command: [0; 32],
                        last_sequence: 0,
                    })
                    .unwrap();
            }
            Command::Issue { to, amount, memo } => {
                posting = Some((MemberId(0), *to, *amount, memo.clone(), None, None))
            }
            Command::Retire { from, amount, memo } => {
                posting = Some((*from, MemberId(0), *amount, memo.clone(), None, None))
            }
            Command::Transfer { to, amount, memo } => {
                posting = Some((actor, *to, *amount, memo.clone(), None, None))
            }
            Command::ClassifiedTransfer {
                to,
                amount,
                memo,
                economic,
            } => {
                posting_economic = *economic;
                posting = Some((actor, *to, *amount, memo.clone(), None, None));
            }
            Command::List {
                title,
                description,
                price,
                side,
                details,
            } => {
                if self.listings.is_full() {
                    let i = self
                        .listings
                        .iter()
                        .position(|l| self.listing_recyclable(l, event.timestamp))
                        .unwrap();
                    self.listings.remove(i);
                }
                self.listings
                    .push(Listing {
                        id: event.sequence,
                        owner: actor,
                        title: title.clone(),
                        description: description.clone(),
                        price: *price,
                        side: *side,
                        status: ListingStatus::Active,
                        buyer: None,
                        sold_tx: None,
                        details: details.clone().unwrap_or_default(),
                        economic: EconomicDetails::default(),
                        created_at: event.timestamp,
                        updated_at: event.timestamp,
                    })
                    .unwrap();
            }
            Command::ClassifiedList {
                title,
                description,
                price,
                side,
                economic,
                standard,
            } => {
                if self.listings.is_full() {
                    let i = self
                        .listings
                        .iter()
                        .position(|l| self.listing_recyclable(l, event.timestamp))
                        .unwrap();
                    self.listings.remove(i);
                }
                let mut resolved = *economic;
                let resolved_title = if economic.thing == 0 {
                    if let Some(thing) = self
                        .things
                        .iter_mut()
                        .find(|t| t.name.as_str().eq_ignore_ascii_case(title.as_str()))
                    {
                        resolved.thing = thing.id;
                        thing.updated_at = event.timestamp;
                        thing.standard |= *standard;
                        thing.name.clone()
                    } else {
                        if self.things.is_full() {
                            let i = self
                                .things
                                .iter()
                                .enumerate()
                                .filter(|(_, t)| {
                                    !t.standard
                                        && !self.listings.iter().any(|l| {
                                            l.status == ListingStatus::Active
                                                && l.economic.thing == t.id
                                        })
                                })
                                .min_by_key(|(_, t)| (t.updated_at, t.id))
                                .unwrap()
                                .0;
                            self.things.remove(i);
                        }
                        resolved.thing = event.sequence;
                        self.things
                            .push(Thing {
                                id: event.sequence,
                                name: title.clone(),
                                kind: economic.kind,
                                unit: economic.unit,
                                standard: *standard,
                                updated_at: event.timestamp,
                            })
                            .unwrap();
                        title.clone()
                    }
                } else {
                    let thing = self
                        .things
                        .iter_mut()
                        .find(|t| t.id == economic.thing)
                        .unwrap();
                    thing.updated_at = event.timestamp;
                    thing.standard |= *standard;
                    thing.name.clone()
                };
                self.listings
                    .push(Listing {
                        id: event.sequence,
                        owner: actor,
                        title: resolved_title,
                        description: description.clone(),
                        price: *price,
                        side: *side,
                        status: ListingStatus::Active,
                        buyer: None,
                        sold_tx: None,
                        details: ListingDetails::default(),
                        economic: resolved,
                        created_at: event.timestamp,
                        updated_at: event.timestamp,
                    })
                    .unwrap();
            }
            Command::Cancel { listing } => {
                let l = self.listings.iter_mut().find(|l| l.id == *listing).unwrap();
                l.status = ListingStatus::Cancelled;
                l.updated_at = event.timestamp;
            }
            Command::UpdateListing {
                listing,
                title,
                description,
                price,
            } => {
                let l = self.listings.iter_mut().find(|l| l.id == *listing).unwrap();
                l.updated_at = event.timestamp;
                if let Some(title) = title {
                    l.title = title.clone();
                }
                if let Some(description) = description {
                    l.description = description.clone();
                }
                if let Some(price) = price {
                    l.price = *price;
                }
            }
            Command::Buy { listing } => {
                let l = self.listings.iter_mut().find(|l| l.id == *listing).unwrap();
                l.status = ListingStatus::Sold;
                l.updated_at = event.timestamp;
                let (from, to) = match l.side {
                    Side::Sell => (actor, l.owner),
                    Side::Buy => (l.owner, actor),
                };
                l.buyer = Some(from);
                l.sold_tx = Some(event.sequence);
                posting_economic = l.economic;
                posting = Some((
                    from,
                    to,
                    l.price,
                    Memo::try_from("Marketplace purchase").unwrap(),
                    None,
                    Some(*listing),
                ));
            }
            Command::Reverse { transaction, memo } => {
                let tx = self
                    .history
                    .iter_mut()
                    .find(|t| t.id == *transaction)
                    .unwrap();
                tx.reversed = true;
                usd = tx.usd;
                posting_economic = tx.economic;
                posting = Some((
                    tx.to,
                    tx.from,
                    tx.amount,
                    memo.clone(),
                    Some(*transaction),
                    tx.listing,
                ));
                self.mark_offer_reversed(*transaction, event.timestamp);
            }
            Command::IssueUsd { .. }
            | Command::PostQuote { .. }
            | Command::TakeQuote { .. }
            | Command::CancelQuote { .. } => self.apply_forex(event),
            Command::MakeOffer { .. }
            | Command::AcceptOffer { .. }
            | Command::UnacceptOffer { .. }
            | Command::DeclineOffer { .. }
            | Command::WithdrawOffer { .. } => self.apply_offer(event),
        }
        if let Some((from, to, amount, memo, reverses, listing)) = posting {
            self.record_transaction(Transaction {
                id: event.sequence,
                actor,
                created_at: event.timestamp,
                from,
                to,
                amount,
                memo: TransactionMemo::try_from(memo.as_str()).unwrap(),
                reverses,
                reversed: false,
                listing,
                usd,
                quote: None,
                loan: None,
                lotto: None,
                economic: posting_economic,
            });
        }
        self.last_timestamp = event.timestamp;
        self.sequence = event.sequence;
        if actor == MemberId(0) {
            return;
        }
        let m = self.members.iter_mut().find(|m| m.id == actor).unwrap();
        m.last_request = event.request_id;
        m.last_command = fingerprint(&event.command);
        m.last_sequence = event.sequence;
    }

    pub(crate) fn record_transaction(&mut self, tx: Transaction) {
        if !tx.usd {
            for id in [tx.from, tx.to] {
                if id != MemberId(0) && id != crate::lotto::ESCROW {
                    let bit = 1u16 << (id.0 - 1);
                    if tx.loan.is_some() {
                        self.credit_blocked |= bit;
                    } else {
                        self.credit_blocked &= !bit;
                    }
                }
            }
        }
        self.transactions += 1;
        for (id, delta) in [(tx.from, -tx.amount), (tx.to, tx.amount)] {
            if id == crate::lotto::ESCROW {
                self.lotto_escrow += delta;
            } else if id == MemberId(0) {
                if tx.usd {
                    self.usd_issuance_balance += delta;
                } else {
                    self.issuance_balance += delta;
                }
            } else {
                let member = self.members.iter_mut().find(|m| m.id == id).unwrap();
                if tx.usd {
                    member.usd_cents += delta;
                } else {
                    member.balance += delta;
                }
            }
        }
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(tx);
    }

    pub fn check_invariants(&self) -> Result<(), Error> {
        self.check_lottos()?;
        if self.decimals > 8 || self.money_epoch > MAX_SEQUENCE {
            return Err(Error::CorruptJournal);
        }
        for (index, l) in self.loans.iter().enumerate() {
            if l.id == 0
                || l.id > self.sequence
                || self.loans[..index].iter().any(|other| other.id == l.id)
                || l.lender == l.terms.borrower
                || self.member(l.lender).is_err()
                || self.member(l.terms.borrower).is_err()
                || !(1..=MAX_AMOUNT).contains(&l.terms.amount)
                || !(1..=l.terms.amount).contains(&l.terms.installment)
                || ![1, 7, 30, 365].contains(&l.terms.rate_days)
                || ![1, 7, 30].contains(&l.terms.payment_days)
                || !(0..=l.terms.amount).contains(&l.principal)
                || l.interest < 0
                || l.principal
                    .checked_add(l.interest)
                    .is_none_or(|v| v > MAX_SEQUENCE as i64)
                || !(0..=l.principal).contains(&l.principal_due)
                || !(0..=l.interest).contains(&l.interest_due)
                || l.remainder >= l.denominator()
                || l.accrued_at > self.last_timestamp
                || l.updated_at > self.last_timestamp
                || (l.status == crate::loans::LoanStatus::Active
                    && !(crate::offers::MIN_CLOCK..=MAX_SEQUENCE).contains(&l.next_due_at))
            {
                return Err(Error::CorruptJournal);
            }
        }
        for q in &self.quotes {
            if q.nc_scale != crate::money::scale(self.decimals)
                || !(1..=MAX_AMOUNT).contains(&q.coins)
                || !(1..=MAX_AMOUNT).contains(&q.cents_per_coin)
            {
                return Err(Error::CorruptJournal);
            }
            let product = q.coins as i128 * q.cents_per_coin as i128;
            if product % q.nc_scale as i128 != 0
                || !(1..=MAX_AMOUNT as i128).contains(&(product / q.nc_scale as i128))
            {
                return Err(Error::CorruptJournal);
            }
        }
        let sum =
            self.members
                .iter()
                .try_fold(self.issuance_balance + self.lotto_escrow, |sum, m| {
                    if m.balance.unsigned_abs() > MAX_SEQUENCE {
                        None
                    } else {
                        sum.checked_add(m.balance)
                    }
                });
        let usd_sum = self
            .members
            .iter()
            .try_fold(self.usd_issuance_balance, |sum, m| {
                if m.usd_cents.unsigned_abs() > MAX_SEQUENCE {
                    None
                } else {
                    sum.checked_add(m.usd_cents)
                }
            });
        if sum == Some(0) && usd_sum == Some(0) {
            Ok(())
        } else {
            Err(Error::CorruptJournal)
        }
    }

    fn add_password_member(
        &mut self,
        username: &Name,
        display_name: &Name,
        password: &PasswordVerifier,
        role: Role,
        created_at: u64,
    ) {
        let id = MemberId(self.members.len() as u8 + 1);
        self.members
            .push(Member {
                id,
                username: username.clone(),
                mastodon_id: MastodonId::new(),
                name: display_name.clone(),
                role,
                disabled: false,
                password: Some(password.clone()),
                token_hash: [0; 32],
                balance: 0,
                usd_cents: 0,
                created_at,
                last_request: 0,
                last_command: [0; 32],
                last_sequence: 0,
            })
            .unwrap();
    }
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

fn valid_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        Err(Error::InvalidInput)
    } else {
        Ok(())
    }
}

fn valid_mastodon_id(value: &str) -> Result<(), Error> {
    if value.is_empty() {
        return Ok(());
    }
    let value = value.strip_prefix('@').unwrap_or(value);
    let mut parts = value.split('@');
    let user = parts.next().unwrap_or_default();
    let host = parts.next().unwrap_or_default();
    if user.is_empty()
        || host.is_empty()
        || parts.next().is_some()
        || !host.contains('.')
        || value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '/')
    {
        Err(Error::InvalidInput)
    } else {
        Ok(())
    }
}

fn valid_economic(value: &EconomicDetails) -> Result<(), Error> {
    if value.quantity_milli == 0 || value.quantity_milli > 1_000_000_000 {
        Err(Error::InvalidInput)
    } else {
        Ok(())
    }
}

pub(crate) fn fingerprint(command: &Command) -> TokenHash {
    let mut buffer = [0; 2048];
    let len =
        serde_json_core::to_slice(command, &mut buffer).expect("bounded command fits event buffer");
    Sha256::digest(&buffer[..len]).into()
}
