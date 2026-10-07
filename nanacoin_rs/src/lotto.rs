//! Ticket pools with durable draws and one journaled cash leg per settlement step.
use crate::domain::*;
use serde::{Deserialize, Serialize};
pub const LOTTOS: usize = 16;
pub const ESCROW: MemberId = MemberId(255);
pub const HOUSE_INTEREST: u8 = MEMBERS as u8 + 1;
pub const ISSUED_INTEREST: u8 = HOUSE_INTEREST + 1;
pub const DONE: u8 = ISSUED_INTEREST + 1;
pub const MONTH: u64 = 30 * crate::loans::DAY;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LottoKind {
    Simple,
    Delayed,
    Savings,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LottoTerms {
    pub kind: LottoKind,
    pub title: Title,
    pub ticket_price: i64,
    pub closes_at: u64,
    /// Simple rate for the whole 30-day holding period, in basis points.
    pub rate_bps: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lotto {
    pub id: u64,
    pub house: MemberId,
    pub terms: LottoTerms,
    #[serde(with = "first_sixteen")]
    pub tickets: [u32; MEMBERS],
    pub pool: i64,
    pub escrow: i64,
    pub interest: i64,
    pub interest_remaining: i64,
    pub winner: Option<MemberId>,
    /// 0 = draw; 1..=MEMBERS = principal; then house interest, issuance, DONE.
    pub step: u8,
}
/// A lotto row stores the tickets of members 1-16, exactly as it did when a
/// household had at most 16 members; checkpoints keep members 17-32 beside
/// it (`journal/checkpoint.rs`, `LottoTicketsRow`).
mod first_sixteen {
    use super::MEMBERS;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(tickets: &[u32; MEMBERS], s: S) -> Result<S::Ok, S::Error> {
        let low: &[u32; 16] = tickets[..16].try_into().unwrap();
        low.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u32; MEMBERS], D::Error> {
        let low = <[u32; 16]>::deserialize(d)?;
        let mut tickets = [0; MEMBERS];
        tickets[..16].copy_from_slice(&low);
        Ok(tickets)
    }
}
impl Lotto {
    pub fn due_at(&self) -> u64 {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::lotto_projection(self, 0)[37] as u64
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            self.terms.closes_at
                + if self.terms.kind == LottoKind::Simple {
                    0
                } else {
                    MONTH
                }
        }
    }
    pub fn total_tickets(&self) -> u64 {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::lotto_projection(self, 0)[36] as u64
        }
        #[cfg(not(feature = "cobol-core"))]
        self.tickets.iter().map(|n| *n as u64).sum()
    }
    pub fn winner_for(&self, ticket: u64) -> Option<MemberId> {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::lotto_winner(self, ticket)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            let mut ticket = ticket;
            for (i, n) in self.tickets.iter().enumerate() {
                if ticket < *n as u64 {
                    return Some(MemberId(i as u8 + 1));
                }
                ticket -= *n as u64;
            }
            None
        }
    }
    pub(crate) fn ready(&self, now: u64) -> bool {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::lotto_projection(self, now)[39] != 0
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            self.step < DONE && now >= self.due_at()
        }
    }
    pub(crate) fn status(&self, now: u64) -> &'static str {
        #[cfg(feature = "cobol-core")]
        {
            match crate::cobol::lotto_projection(self, now)[38] {
                3 => "SETTLED",
                2 => "PAYING",
                1 => "WAITING",
                _ => "OPEN",
            }
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if self.step == DONE {
                "SETTLED"
            } else if self.step > 0 {
                "PAYING"
            } else if now >= self.terms.closes_at {
                "WAITING"
            } else {
                "OPEN"
            }
        }
    }
}
impl State {
    pub fn lotto(&self, id: u64) -> Result<&Lotto, Error> {
        self.lottos
            .iter()
            .find(|l| l.id == id)
            .ok_or(Error::NotFound)
    }
    #[cfg(not(feature = "cobol-core"))]
    pub(crate) fn lotto_leg(&self, l: &Lotto) -> Option<(MemberId, MemberId, i64, EconomicKind)> {
        let winner = l.winner?;
        if (1..=MEMBERS as u8).contains(&l.step) {
            let to = MemberId(l.step);
            let amount = if l.terms.kind == LottoKind::Savings {
                l.tickets[l.step as usize - 1] as i64 * l.terms.ticket_price
            } else if to == winner {
                l.pool
            } else {
                0
            };
            return (amount > 0).then_some((ESCROW, to, amount, EconomicKind::Other));
        }
        let amount = if l.step == HOUSE_INTEREST {
            l.interest_remaining
                .min(self.member(l.house).ok()?.balance.max(0))
        } else if l.step == ISSUED_INTEREST {
            l.interest_remaining
        } else {
            0
        };
        (amount > 0).then_some((
            if l.step == HOUSE_INTEREST {
                l.house
            } else {
                MemberId(0)
            },
            winner,
            amount,
            EconomicKind::Interest,
        ))
    }
    pub(crate) fn validate_lotto(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.lotto_plan(actor, command, now).map(|_| ())
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if !(crate::offers::MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
                return Err(Error::Unavailable);
            }
            match command {
                Command::CreateLotto { terms } => {
                    self.admin(actor)?;
                    if self.lottos.is_full() && !self.lottos.iter().any(|l| l.step == DONE) {
                        return Err(Error::Capacity);
                    }
                    if terms.title.trim().is_empty()
                        || terms.title.chars().any(char::is_control)
                        || !(1..=MAX_AMOUNT).contains(&terms.ticket_price)
                        || terms.closes_at <= now
                        || terms.closes_at > MAX_SEQUENCE - MONTH
                        || terms.rate_bps > 10_000
                        || (terms.kind == LottoKind::Simple && terms.rate_bps != 0)
                    {
                        return Err(Error::InvalidInput);
                    }
                }
                Command::BuyTickets { lotto, count } => {
                    let l = self.lotto(*lotto)?;
                    if actor == l.house {
                        return Err(Error::Forbidden);
                    }
                    if now >= l.terms.closes_at || l.step != 0 {
                        return Err(Error::Conflict);
                    }
                    if *count == 0 {
                        return Err(Error::InvalidInput);
                    }
                    l.tickets[actor.0 as usize - 1]
                        .checked_add(*count)
                        .ok_or(Error::Overflow)?;
                    let amount = l
                        .terms
                        .ticket_price
                        .checked_mul(*count as i64)
                        .filter(|n| *n <= MAX_AMOUNT)
                        .ok_or(Error::Overflow)?;
                    let pool = l
                        .pool
                        .checked_add(amount)
                        .filter(|n| *n <= MAX_AMOUNT)
                        .ok_or(Error::Overflow)?;
                    let interest = (pool as i128 * l.terms.rate_bps as i128 / 10_000) as i64;
                    if pool + interest > MAX_AMOUNT
                        || self
                            .lotto_escrow
                            .checked_add(amount)
                            .is_none_or(|n| n > MAX_SEQUENCE as i64)
                    {
                        return Err(Error::Overflow);
                    }
                    if self.member(actor)?.balance < amount {
                        return Err(Error::InsufficientFunds);
                    }
                }
                Command::RunLotto {
                    lotto,
                    step,
                    ticket,
                } => {
                    if actor != MemberId(0) {
                        return Err(Error::Forbidden);
                    }
                    let l = self.lotto(*lotto)?;
                    if *step != l.step || l.step == DONE || now < l.due_at() {
                        return Err(Error::Conflict);
                    }
                    if (l.step == 0 && *ticket >= l.total_tickets().max(1))
                        || (l.step != 0 && *ticket != 0)
                    {
                        return Err(Error::InvalidInput);
                    }
                    if let Some((from, to, amount, _)) = self.lotto_leg(l) {
                        let balance = self.member(to)?.balance;
                        if from != to
                            && balance
                                .checked_add(amount)
                                .is_none_or(|n| n > MAX_SEQUENCE as i64)
                        {
                            return Err(Error::Overflow);
                        }
                        if from == MemberId(0)
                            && self
                                .issuance_balance
                                .checked_sub(amount)
                                .is_none_or(|n| n < -(MAX_SEQUENCE as i64))
                        {
                            return Err(Error::Overflow);
                        }
                    }
                }
                _ => return Err(Error::InvalidInput),
            }
            Ok(())
        }
    }
    pub(crate) fn apply_lotto(&mut self, event: &Event) {
        #[cfg(feature = "cobol-core")]
        {
            let p = self.prepared.lotto.take().expect("prepared lotto");
            if let Command::CreateLotto { terms } = &event.command {
                if self.lottos.is_full() {
                    self.lottos
                        .remove(p.recycle.expect("prepared lotto recycling"));
                }
                self.lottos
                    .push(Lotto {
                        id: event.sequence,
                        house: event.actor,
                        terms: terms.clone(),
                        tickets: [0; MEMBERS],
                        pool: p.pool,
                        escrow: p.escrow,
                        interest: p.interest,
                        interest_remaining: p.interest_remaining,
                        winner: p.winner,
                        step: p.step,
                    })
                    .unwrap();
            } else {
                let l = &mut self.lottos[p.index.expect("prepared lotto row")];
                l.pool = p.pool;
                l.escrow = p.escrow;
                l.interest = p.interest;
                l.interest_remaining = p.interest_remaining;
                l.winner = p.winner;
                l.step = p.step;
                if matches!(event.command, Command::BuyTickets { .. }) {
                    l.tickets[event.actor.0 as usize - 1] = p.tickets;
                }
                let id = l.id;
                if p.amount > 0 {
                    self.lotto_post(event, id, p.from, p.to, p.amount, p.kind);
                }
            }
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            match &event.command {
                Command::CreateLotto { terms } => {
                    if self.lottos.is_full() {
                        let i = self.lottos.iter().position(|l| l.step == DONE).unwrap();
                        self.lottos.remove(i);
                    }
                    self.lottos
                        .push(Lotto {
                            id: event.sequence,
                            house: event.actor,
                            terms: terms.clone(),
                            tickets: [0; MEMBERS],
                            pool: 0,
                            escrow: 0,
                            interest: 0,
                            interest_remaining: 0,
                            winner: None,
                            step: 0,
                        })
                        .unwrap();
                }
                Command::BuyTickets { lotto, count } => {
                    let l = self.lottos.iter_mut().find(|l| l.id == *lotto).unwrap();
                    let amount = l.terms.ticket_price * *count as i64;
                    l.tickets[event.actor.0 as usize - 1] += count;
                    l.pool += amount;
                    l.escrow += amount;
                    l.interest = (l.pool as i128 * l.terms.rate_bps as i128 / 10_000) as i64;
                    l.interest_remaining = l.interest;
                    self.lotto_post(
                        event,
                        *lotto,
                        event.actor,
                        ESCROW,
                        amount,
                        EconomicKind::Other,
                    );
                }
                Command::RunLotto { lotto, ticket, .. } => {
                    let mut l = self.lotto(*lotto).unwrap().clone();
                    if l.step == 0 {
                        l.winner = l.winner_for(*ticket);
                        l.step = if l.winner.is_some() { 1 } else { DONE };
                    } else {
                        if let Some((from, to, amount, kind)) = self.lotto_leg(&l) {
                            self.lotto_post(event, *lotto, from, to, amount, kind);
                            if from == ESCROW {
                                l.escrow -= amount;
                            } else {
                                l.interest_remaining -= amount;
                            }
                        }
                        l.step += 1;
                    }
                    *self.lottos.iter_mut().find(|v| v.id == *lotto).unwrap() = l;
                }
                _ => unreachable!(),
            }
        }
    }
    fn lotto_post(
        &mut self,
        event: &Event,
        lotto: u64,
        from: MemberId,
        to: MemberId,
        amount: i64,
        kind: EconomicKind,
    ) {
        self.record_transaction(Transaction {
            meta: crate::ledger::TransactionMeta::default(),
            id: event.sequence,
            actor: event.actor,
            created_at: event.timestamp,
            from,
            to,
            amount,
            memo: TransactionMemo::try_from(if kind == EconomicKind::Interest {
                "Lotto interest"
            } else if to == ESCROW {
                "Lotto tickets"
            } else {
                "Lotto principal"
            })
            .unwrap(),
            reverses: None,
            listing: None,
            usd: false,
            quote: None,
            loan: None,
            lotto: Some(lotto),
            economic: EconomicDetails {
                kind,
                ..EconomicDetails::default()
            },
        });
    }
    pub(crate) fn check_lottos(&self) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.lotto_invariants()
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            let mut escrow = 0i64;
            for (i, l) in self.lottos.iter().enumerate() {
                if l.id == 0
                    || l.id > self.sequence
                    || self.lottos[..i].iter().any(|v| v.id == l.id)
                    || self.member(l.house).is_err()
                    || l.step > DONE
                    || !(1..=MAX_AMOUNT).contains(&l.terms.ticket_price)
                    || l.terms.rate_bps > 10_000
                    || l.terms.closes_at > MAX_SEQUENCE - MONTH
                    || (l.terms.kind == LottoKind::Simple && l.terms.rate_bps != 0)
                    || l.pool as i128 != l.total_tickets() as i128 * l.terms.ticket_price as i128
                    || !(0..=MAX_AMOUNT).contains(&l.pool)
                    || !(0..=l.pool).contains(&l.escrow)
                    || l.interest != (l.pool as i128 * l.terms.rate_bps as i128 / 10_000) as i64
                    || !(0..=l.interest).contains(&l.interest_remaining)
                    || (l.step == DONE && (l.escrow != 0 || l.interest_remaining != 0))
                    || l.winner.is_some_and(|w| {
                        w.0 == 0 || w.0 as usize > MEMBERS || l.tickets[w.0 as usize - 1] == 0
                    })
                    || l.tickets
                        .iter()
                        .enumerate()
                        .any(|(i, n)| *n > 0 && self.member(MemberId(i as u8 + 1)).is_err())
                {
                    return Err(Error::CorruptJournal);
                }
                let paid_principal: i64 = if l.terms.kind == LottoKind::Savings {
                    l.tickets
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| l.step > (*i as u8 + 1))
                        .map(|(_, n)| *n as i64 * l.terms.ticket_price)
                        .sum()
                } else if l.winner.is_some_and(|w| l.step > w.0) {
                    l.pool
                } else {
                    0
                };
                if l.escrow != l.pool - paid_principal
                    || (l.step == 0 && l.winner.is_some())
                    || (l.step > 0 && l.total_tickets() > 0 && l.winner.is_none())
                    || (l.step <= HOUSE_INTEREST && l.interest_remaining != l.interest)
                {
                    return Err(Error::CorruptJournal);
                }
                escrow = escrow.checked_add(l.escrow).ok_or(Error::CorruptJournal)?;
            }
            if escrow != self.lotto_escrow || escrow < 0 {
                return Err(Error::CorruptJournal);
            }
            Ok(())
        }
    }
}
/// Rejection sampling avoids modulo bias. Persist the selected ticket before paying.
pub(crate) fn random_ticket(total: u64) -> Result<u64, Error> {
    if total == 0 {
        return Ok(0);
    }
    let threshold = total.wrapping_neg() % total;
    loop {
        let mut bytes = [0; 8];
        getrandom::getrandom(&mut bytes).map_err(|_| Error::Unavailable)?;
        let n = u64::from_le_bytes(bytes);
        if n >= threshold {
            return Ok(n % total);
        }
    }
}
