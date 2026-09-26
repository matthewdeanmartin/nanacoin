//! Bounded quote book. One durable command commits both currencies atomically.
use crate::{domain::*, journal::MAX_RECORDS, offers::MIN_CLOCK};
use serde::{Deserialize, Serialize};

pub const QUOTES: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteSide {
    BID,
    ASK,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteStatus {
    Open,
    Filled,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quote {
    pub nc_scale: i64,
    pub id: u64,
    pub maker: MemberId,
    pub side: QuoteSide,
    pub cents_per_coin: i64,
    pub coins: i64,
    pub expires_at: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub status: QuoteStatus,
    pub taker: Option<MemberId>,
    pub coin_tx: Option<u64>,
    pub cash_tx: Option<u64>,
}
impl Quote {
    pub fn live(&self, now: u64) -> bool {
        self.status == QuoteStatus::Open
            && (MIN_CLOCK..=MAX_SEQUENCE).contains(&now)
            && (self.expires_at == 0 || now < self.expires_at)
    }
    pub fn cents(&self) -> i64 {
        (self.coins as i128 * self.cents_per_coin as i128 / self.nc_scale as i128) as i64
    }
    fn parties(&self, taker: MemberId) -> (MemberId, MemberId) {
        match self.side {
            QuoteSide::ASK => (self.maker, taker),
            QuoteSide::BID => (taker, self.maker),
        }
    }
}
impl State {
    pub(crate) fn quote(&self, id: u64) -> Result<&Quote, Error> {
        self.quotes
            .iter()
            .find(|q| q.id == id)
            .ok_or(Error::NotFound)
    }
    pub(crate) fn validate_currency_posting(
        &self,
        from: MemberId,
        to: MemberId,
        amount: i64,
        correction: bool,
        usd: bool,
    ) -> Result<(), Error> {
        if !usd {
            return self.validate_posting(from, to, amount, correction);
        }
        if from == to || !(1..=MAX_AMOUNT).contains(&amount) {
            return Err(Error::InvalidInput);
        }
        let balance = |id| {
            if id == MemberId(0) {
                Ok(self.usd_issuance_balance)
            } else {
                let m = self.member(id)?;
                if !correction && m.disabled {
                    return Err(Error::Disabled);
                }
                Ok(m.usd_cents)
            }
        };
        let debit = balance(from)?.checked_sub(amount).ok_or(Error::Overflow)?;
        let credit = balance(to)?.checked_add(amount).ok_or(Error::Overflow)?;
        if debit.unsigned_abs() > MAX_SEQUENCE || credit.unsigned_abs() > MAX_SEQUENCE {
            return Err(Error::Overflow);
        }
        if !correction && from != MemberId(0) && debit < 0 {
            return Err(Error::InsufficientFunds);
        }
        Ok(())
    }
    pub(crate) fn validate_forex(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<(), Error> {
        match command {
            Command::IssueUsd { to, cents, .. } => {
                self.admin(actor)?;
                self.validate_currency_posting(MemberId(0), *to, *cents, false, true)?;
            }
            Command::PostQuote {
                cents_per_coin,
                coins,
                expires_at,
                ..
            } => {
                if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
                    return Err(Error::Unavailable);
                }
                let total = *coins as i128 * *cents_per_coin as i128;
                let scale = crate::money::scale(self.decimals) as i128;
                if !(1..=MAX_AMOUNT).contains(cents_per_coin)
                    || !(1..=MAX_AMOUNT).contains(coins)
                    || total % scale != 0
                    || !(1..=MAX_AMOUNT as i128).contains(&(total / scale))
                    || (*expires_at != 0 && *expires_at <= now)
                    || *expires_at > MAX_SEQUENCE
                {
                    return Err(Error::InvalidInput);
                }
                if self.quotes.is_full() && self.quotes.iter().all(|q| q.live(now)) {
                    return Err(Error::Capacity);
                }
            }
            Command::TakeQuote { quote } => {
                if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
                    return Err(Error::Unavailable);
                }
                let q = self.quote(*quote)?;
                if q.maker == actor {
                    return Err(Error::SelfDeal);
                }
                if !q.live(now) {
                    return Err(Error::Conflict);
                }
                let (seller, buyer) = q.parties(actor);
                self.validate_currency_posting(seller, buyer, q.coins, false, false)?;
                self.validate_currency_posting(buyer, seller, q.cents(), false, true)?;
            }
            Command::CancelQuote { quote } => {
                let q = self.quote(*quote)?;
                if q.maker != actor {
                    self.admin(actor)?;
                }
                if q.status != QuoteStatus::Open {
                    return Err(Error::Conflict);
                }
            }
            _ => unreachable!("forex command"),
        }
        Ok(())
    }
    pub(crate) fn apply_forex(&mut self, event: &Event) {
        match &event.command {
            Command::IssueUsd { to, cents, memo } => self.record_transaction(Transaction {
                meta: crate::ledger::TransactionMeta::default(),
                id: event.sequence,
                actor: event.actor,
                created_at: event.timestamp,
                from: MemberId(0),
                to: *to,
                amount: *cents,
                memo: TransactionMemo::try_from(memo.as_str()).unwrap(),
                reverses: None,
                listing: None,
                usd: true,
                quote: None,
                loan: None,
                lotto: None,
                economic: EconomicDetails::default(),
            }),
            Command::PostQuote {
                side,
                cents_per_coin,
                coins,
                expires_at,
            } => {
                if self.quotes.is_full() {
                    let i = self
                        .quotes
                        .iter()
                        .enumerate()
                        .filter(|(_, q)| !q.live(event.timestamp))
                        .min_by_key(|(_, q)| (q.updated_at, q.id))
                        .unwrap()
                        .0;
                    self.quotes.remove(i);
                }
                self.quotes
                    .push(Quote {
                        nc_scale: crate::money::scale(self.decimals),
                        id: event.sequence,
                        maker: event.actor,
                        side: *side,
                        cents_per_coin: *cents_per_coin,
                        coins: *coins,
                        expires_at: *expires_at,
                        created_at: event.timestamp,
                        updated_at: event.timestamp,
                        status: QuoteStatus::Open,
                        taker: None,
                        coin_tx: None,
                        cash_tx: None,
                    })
                    .unwrap();
            }
            Command::CancelQuote { quote } => {
                let q = self.quotes.iter_mut().find(|q| q.id == *quote).unwrap();
                q.status = QuoteStatus::Cancelled;
                q.updated_at = event.timestamp;
            }
            Command::TakeQuote { quote } => {
                let q = self.quotes.iter_mut().find(|q| q.id == *quote).unwrap();
                let (seller, buyer) = q.parties(event.actor);
                q.status = QuoteStatus::Filled;
                q.taker = Some(event.actor);
                q.updated_at = event.timestamp;
                q.coin_tx = Some(event.sequence);
                // The second leg occupies a disjoint ID range; event IDs retain
                // their existing meaning for old journals and retry receipts.
                q.cash_tx = Some(event.sequence + MAX_RECORDS as u64);
                let coin = Transaction {
                    meta: crate::ledger::TransactionMeta::default(),
                    id: event.sequence,
                    actor: event.actor,
                    created_at: event.timestamp,
                    from: seller,
                    to: buyer,
                    amount: q.coins,
                    memo: TransactionMemo::try_from("Exchange").unwrap(),
                    reverses: None,
                    listing: None,
                    usd: false,
                    quote: Some(q.id),
                    loan: None,
                    lotto: None,
                    economic: EconomicDetails::default(),
                };
                let cash = Transaction {
                    meta: crate::ledger::TransactionMeta::default(),
                    id: q.cash_tx.unwrap(),
                    from: buyer,
                    to: seller,
                    amount: q.cents(),
                    usd: true,
                    ..coin.clone()
                };
                self.record_transaction(coin);
                self.record_transaction(cash);
            }
            _ => unreachable!("forex command"),
        }
    }
}
