//! Bounded, cash-funded household lending. No borrowing from issuance.
use crate::domain::*;
#[cfg(not(feature = "cobol-core"))]
use crate::offers::MIN_CLOCK;
use serde::{Deserialize, Serialize};

pub const LOANS: usize = 32;
/// Open loan offers (not yet accepted or withdrawn) one lender may have.
pub const OFFERS_PER_LENDER: usize = 4;
pub const DAY: u64 = 86_400;
pub const RATE_SCALE: u64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoanStatus {
    Offered,
    Armed,
    Active,
    Paid,
    Declined,
    Cancelled,
    Requested,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoanTerms {
    pub borrower: MemberId,
    pub amount: i64,
    /// Hundredths of a percent per rate period. Unsigned: negative rates rejected.
    pub rate_bps: u32,
    pub rate_days: u16,
    pub payment_days: u16,
    /// Principal due per installment; interest is collected in addition.
    pub installment: i64,
    pub credit: bool,
    pub memo: Memo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Loan {
    pub id: u64,
    pub lender: MemberId,
    pub terms: LoanTerms,
    pub status: LoanStatus,
    pub principal: i64,
    pub interest: i64,
    pub remainder: u64,
    pub principal_due: i64,
    pub interest_due: i64,
    pub accrued_at: u64,
    pub next_due_at: u64,
    pub created_at: u64,
    pub updated_at: u64,
    /// Prevent repeated collection attempts without any new borrower funds.
    pub attempted_balance: i64,
}

impl LoanTerms {
    /// The yearly rate in hundredths of a percent, rounded down: what
    /// clients compare (`rate_bps` is per `rate_days`).
    pub fn apr_bps(&self) -> u32 {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::annual_rate(self.rate_bps, self.rate_days)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            (u64::from(self.rate_bps) * 365 / u64::from(self.rate_days.max(1)))
                .min(u64::from(u32::MAX)) as u32
        }
    }
}

impl Loan {
    pub fn visible_to(&self, member: &Member) -> bool {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::loan_projection(None, self, Some(member), 0)[20] == 1
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            self.status == LoanStatus::Requested
                || member.role == Role::Nana
                || member.id == self.lender
                || member.id == self.terms.borrower
        }
    }
    pub fn terminal(&self) -> bool {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::loan_projection(None, self, None, 0)[21] == 1
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            matches!(
                self.status,
                LoanStatus::Paid | LoanStatus::Declined | LoanStatus::Cancelled
            )
        }
    }
    pub fn denominator(&self) -> u64 {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::loan_denominator(self.terms.rate_days)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            RATE_SCALE * self.terms.rate_days as u64 * DAY
        }
    }
    pub fn accrue(&mut self, now: u64) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::accrue(self, now)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if now < self.accrued_at {
                return Err(Error::Unavailable);
            }
            let numerator = (self.principal as u128)
                .checked_mul(self.terms.rate_bps as u128)
                .and_then(|n| n.checked_mul((now - self.accrued_at) as u128))
                .and_then(|n| n.checked_add(self.remainder as u128))
                .ok_or(Error::Overflow)?;
            let denominator = self.denominator() as u128;
            let interest = self.interest as u128 + numerator / denominator;
            if interest + self.principal as u128 > MAX_SEQUENCE as u128 {
                return Err(Error::Overflow);
            }
            self.interest = interest as i64;
            self.remainder = (numerator % denominator) as u64;
            self.accrued_at = now;
            Ok(())
        }
    }
    #[cfg(not(feature = "cobol-core"))]
    fn start(&mut self, now: u64) -> Result<(), Error> {
        self.next_due_at = now
            .checked_add(self.terms.payment_days as u64 * DAY)
            .filter(|v| *v <= MAX_SEQUENCE)
            .ok_or(Error::Overflow)?;
        self.principal = self.terms.amount;
        self.accrued_at = now;
        self.updated_at = now;
        self.status = LoanStatus::Active;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Settlement {
    pub loan: Loan,
    pub principal_paid: i64,
    pub interest_paid: i64,
    pub draw: bool,
}

impl State {
    pub fn loan(&self, id: u64) -> Result<&Loan, Error> {
        self.loans
            .iter()
            .find(|l| l.id == id)
            .ok_or(Error::NotFound)
    }

    pub(crate) fn loan_settlement(
        &self,
        id: u64,
        amount: Option<i64>,
        now: u64,
    ) -> Result<Settlement, Error> {
        #[cfg(not(feature = "cobol-core"))]
        if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
            return Err(Error::Unavailable);
        }
        #[cfg(feature = "cobol-core")]
        {
            let loan = self.loan(id)?.clone();
            let borrower = self.member(loan.terms.borrower)?;
            let lender = self.member(loan.lender)?;
            let result = crate::cobol::settle(
                loan,
                amount,
                now,
                borrower.balance,
                self.credit_blocked & (1u32 << (borrower.id.0 - 1)) != 0,
                borrower.disabled,
                lender.disabled,
            )?;
            if let Some((from, to, amount)) = crate::cobol::loan_validation(&result)? {
                self.validate_posting(from, to, amount, false)?;
            }
            Ok(result)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            let mut loan = self.loan(id)?.clone();
            let borrower = self.member(loan.terms.borrower)?;
            let lender = self.member(loan.lender)?;
            if borrower.disabled || lender.disabled {
                return Err(Error::Disabled);
            }
            if loan.status == LoanStatus::Armed && amount.is_none() {
                if borrower.balance != 0 || self.credit_blocked & (1u32 << (borrower.id.0 - 1)) != 0
                {
                    return Err(Error::Conflict);
                }
                self.validate_posting(lender.id, borrower.id, loan.terms.amount, false)?;
                loan.start(now)?;
                return Ok(Settlement {
                    loan,
                    principal_paid: 0,
                    interest_paid: 0,
                    draw: true,
                });
            }
            if loan.status != LoanStatus::Active {
                return Err(Error::Conflict);
            }
            loan.accrue(now)?;
            if now >= loan.next_due_at {
                let interval = loan.terms.payment_days as u64 * DAY;
                let periods = (now - loan.next_due_at) / interval + 1;
                // Constant-time catch-up. Money is paid now, never backdated.
                loan.principal_due = (loan.principal_due as u128
                    + periods as u128 * loan.terms.installment as u128)
                    .min(loan.principal as u128) as i64;
                loan.interest_due = loan.interest;
                loan.next_due_at = loan
                    .next_due_at
                    .checked_add(periods * interval)
                    .filter(|v| *v <= MAX_SEQUENCE)
                    .ok_or(Error::Overflow)?;
            }
            let requested = match amount {
                Some(amount) => {
                    if !(1..=MAX_AMOUNT).contains(&amount) {
                        return Err(Error::InvalidInput);
                    }
                    amount.min(loan.principal + loan.interest)
                }
                None => (loan.principal_due + loan.interest_due).min(MAX_AMOUNT),
            };
            if amount.is_some() && borrower.balance < requested {
                return Err(Error::InsufficientFunds);
            }
            let paid = requested.min(borrower.balance.max(0));
            if paid > 0 {
                self.validate_posting(borrower.id, lender.id, paid, false)?;
            }
            let interest_paid = paid.min(loan.interest);
            let principal_paid = paid - interest_paid;
            loan.interest -= interest_paid;
            loan.principal -= principal_paid;
            loan.interest_due = (loan.interest_due - interest_paid).max(0);
            loan.principal_due = (loan.principal_due - principal_paid).max(0);
            loan.attempted_balance = borrower.balance - paid;
            loan.updated_at = now;
            if loan.principal == 0 && loan.interest == 0 {
                loan.status = LoanStatus::Paid;
                // Terms explicitly waive the final fraction below a spendable unit.
                loan.remainder = 0;
                loan.next_due_at = 0;
            }
            Ok(Settlement {
                loan,
                principal_paid,
                interest_paid,
                draw: false,
            })
        }
    }

    pub(crate) fn validate_loan(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.loan_plan(actor, command, now)?;
            match command {
                Command::RepayLoan { loan, amount } => {
                    self.loan_settlement(*loan, Some(*amount), now)?;
                }
                Command::RunLoan { loan, .. } => {
                    self.loan_settlement(*loan, None, now)?;
                }
                _ => {}
            }
            Ok(())
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if !(MIN_CLOCK..=MAX_SEQUENCE).contains(&now) {
                return Err(Error::Unavailable);
            }
            match command {
                Command::OfferLoan { terms }
                | Command::RequestLoan { terms }
                | Command::RespondLoan { terms, .. } => {
                    if matches!(command, Command::RequestLoan { .. }) {
                        if terms.borrower != actor {
                            return Err(Error::Forbidden);
                        }
                    } else if terms.borrower == actor {
                        return Err(Error::SelfDeal);
                    }
                    if self.member(terms.borrower)?.disabled {
                        return Err(Error::Disabled);
                    }
                    if !(1..=MAX_AMOUNT).contains(&terms.amount)
                        || !(1..=terms.amount).contains(&terms.installment)
                        || ![1, 7, 30, 365].contains(&terms.rate_days)
                        || ![1, 7, 30].contains(&terms.payment_days)
                        || terms.memo.chars().any(char::is_control)
                    {
                        return Err(Error::InvalidInput);
                    }
                    if let Command::RespondLoan { loan, .. } = command {
                        let request = self.loan(*loan)?;
                        if request.status != LoanStatus::Requested {
                            return Err(Error::Conflict);
                        }
                        if request.terms.borrower != terms.borrower {
                            return Err(Error::InvalidInput);
                        }
                    }
                    if !matches!(command, Command::RequestLoan { .. })
                        && self
                            .loans
                            .iter()
                            .filter(|l| l.lender == actor && l.status == LoanStatus::Offered)
                            .count()
                            >= OFFERS_PER_LENDER
                    {
                        return Err(Error::MemberLoanLimit);
                    }
                    if !matches!(command, Command::RespondLoan { .. })
                        && self.loans.is_full()
                        && !self.loans.iter().any(Loan::terminal)
                    {
                        return Err(Error::Capacity);
                    }
                    now.checked_add(terms.payment_days as u64 * DAY)
                        .filter(|v| *v <= MAX_SEQUENCE)
                        .ok_or(Error::Overflow)?;
                }
                Command::AcceptLoan { loan } => {
                    let l = self.loan(*loan)?;
                    if actor != l.terms.borrower {
                        return Err(Error::Forbidden);
                    }
                    if l.status != LoanStatus::Offered {
                        return Err(Error::Conflict);
                    }
                    if self.member(l.lender)?.disabled {
                        return Err(Error::Disabled);
                    }
                    if !l.terms.credit {
                        self.validate_posting(l.lender, actor, l.terms.amount, false)?;
                    }
                    let mut copy = l.clone();
                    copy.start(now)?;
                }
                Command::CloseLoan { loan } => {
                    let l = self.loan(*loan)?;
                    if actor != l.lender && actor != l.terms.borrower {
                        return Err(Error::Forbidden);
                    }
                    if !matches!(
                        l.status,
                        LoanStatus::Offered | LoanStatus::Armed | LoanStatus::Requested
                    ) {
                        return Err(Error::Conflict);
                    }
                }
                Command::RepayLoan { loan, amount } => {
                    if actor != self.loan(*loan)?.terms.borrower {
                        return Err(Error::Forbidden);
                    }
                    self.loan_settlement(*loan, Some(*amount), now)?;
                }
                Command::RunLoan {
                    loan,
                    expected_updated_at,
                } => {
                    if actor != MemberId(0) {
                        return Err(Error::Forbidden);
                    }
                    if self.loan(*loan)?.updated_at != *expected_updated_at
                        || !self.loan_ready(self.loan(*loan)?, now)
                    {
                        return Err(Error::Conflict);
                    }
                    self.loan_settlement(*loan, None, now)?;
                }
                _ => return Err(Error::InvalidInput),
            }
            Ok(())
        }
    }

    pub(crate) fn loan_ready(&self, l: &Loan, now: u64) -> bool {
        #[cfg(feature = "cobol-core")]
        {
            crate::cobol::loan_projection(Some(self), l, None, now)[22] == 1
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            let Ok(b) = self.member(l.terms.borrower) else {
                return false;
            };
            let Ok(a) = self.member(l.lender) else {
                return false;
            };
            if b.disabled || a.disabled {
                return false;
            }
            match l.status {
                LoanStatus::Armed => {
                    b.balance == 0
                        && a.balance >= l.terms.amount
                        && self.credit_blocked & (1u32 << (b.id.0 - 1)) == 0
                }
                LoanStatus::Active => {
                    now >= l.next_due_at
                        || (l.principal_due + l.interest_due > 0
                            && b.balance > 0
                            && b.balance != l.attempted_balance)
                }
                _ => false,
            }
        }
    }

    pub(crate) fn apply_loan(&mut self, event: &Event) {
        #[cfg(feature = "cobol-core")]
        let policy = self.prepared.loan_policy.take();
        match &event.command {
            Command::RespondLoan { loan, terms } => {
                let l = self.loans.iter_mut().find(|l| l.id == *loan).unwrap();
                l.terms = terms.clone();
                #[cfg(feature = "cobol-core")]
                policy.as_ref().expect("prepared loan response").install(l);
                #[cfg(not(feature = "cobol-core"))]
                {
                    l.lender = event.actor;
                    l.status = LoanStatus::Offered;
                    l.updated_at = event.timestamp;
                }
            }
            Command::OfferLoan { terms } | Command::RequestLoan { terms } => {
                if self.loans.is_full() {
                    #[cfg(feature = "cobol-core")]
                    let index = policy
                        .as_ref()
                        .expect("prepared loan")
                        .recycle
                        .expect("prepared loan recycling");
                    #[cfg(not(feature = "cobol-core"))]
                    let index = self
                        .loans
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.terminal())
                        .min_by_key(|(_, l)| l.id)
                        .unwrap()
                        .0;
                    self.loans.remove(index);
                }
                let loan = Loan {
                    id: event.sequence,
                    lender: {
                        #[cfg(feature = "cobol-core")]
                        {
                            policy.as_ref().expect("prepared loan creation").lender
                        }
                        #[cfg(not(feature = "cobol-core"))]
                        {
                            if matches!(event.command, Command::RequestLoan { .. }) {
                                MemberId(0)
                            } else {
                                event.actor
                            }
                        }
                    },
                    terms: terms.clone(),
                    status: {
                        #[cfg(feature = "cobol-core")]
                        {
                            policy.as_ref().expect("prepared loan creation").status
                        }
                        #[cfg(not(feature = "cobol-core"))]
                        {
                            if matches!(event.command, Command::RequestLoan { .. }) {
                                LoanStatus::Requested
                            } else {
                                LoanStatus::Offered
                            }
                        }
                    },
                    principal: 0,
                    interest: 0,
                    remainder: 0,
                    principal_due: 0,
                    interest_due: 0,
                    accrued_at: 0,
                    next_due_at: 0,
                    created_at: event.timestamp,
                    updated_at: event.timestamp,
                    attempted_balance: 0,
                };
                #[cfg(feature = "cobol-core")]
                let loan = {
                    let mut loan = loan;
                    policy
                        .as_ref()
                        .expect("prepared loan creation")
                        .install(&mut loan);
                    loan
                };
                self.loans.push(loan).unwrap();
            }
            Command::AcceptLoan { loan } => {
                let mut l = self.loan(*loan).unwrap().clone();
                #[cfg(feature = "cobol-core")]
                {
                    let p = policy.as_ref().expect("prepared loan acceptance");
                    p.install(&mut l);
                    for leg in self.take_loan_postings() {
                        self.loan_posting(
                            event, &l, leg.from, leg.to, leg.amount, leg.kind, leg.offset,
                        );
                    }
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    if l.terms.credit {
                        l.status = LoanStatus::Armed;
                        l.updated_at = event.timestamp;
                    } else {
                        l.start(event.timestamp).unwrap();
                        self.loan_posting(
                            event,
                            &l,
                            l.lender,
                            l.terms.borrower,
                            l.terms.amount,
                            EconomicKind::LoanPrincipal,
                            0,
                        );
                    }
                }
                *self.loans.iter_mut().find(|v| v.id == *loan).unwrap() = l;
            }
            Command::CloseLoan { loan } => {
                let l = self.loans.iter_mut().find(|v| v.id == *loan).unwrap();
                #[cfg(feature = "cobol-core")]
                policy.as_ref().expect("prepared loan closure").install(l);
                #[cfg(not(feature = "cobol-core"))]
                {
                    l.status = if event.actor == l.terms.borrower && l.status == LoanStatus::Offered
                    {
                        LoanStatus::Declined
                    } else {
                        LoanStatus::Cancelled
                    };
                    l.updated_at = event.timestamp;
                }
            }
            Command::RepayLoan { loan, .. } | Command::RunLoan { loan, .. } => {
                #[cfg(not(feature = "cobol-core"))]
                let amount = match event.command {
                    Command::RepayLoan { amount, .. } => Some(amount),
                    _ => None,
                };
                #[cfg(not(feature = "cobol-core"))]
                let result = self
                    .loan_settlement(*loan, amount, event.timestamp)
                    .unwrap();
                #[cfg(feature = "cobol-core")]
                let result = self.prepared.loan.take().expect("prepared loan settlement");
                let l = &result.loan;
                #[cfg(feature = "cobol-core")]
                for leg in self.take_loan_postings() {
                    self.loan_posting(event, l, leg.from, leg.to, leg.amount, leg.kind, leg.offset);
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    if result.draw {
                        self.loan_posting(
                            event,
                            l,
                            l.lender,
                            l.terms.borrower,
                            l.terms.amount,
                            EconomicKind::LoanPrincipal,
                            0,
                        );
                    } else {
                        if result.principal_paid > 0 {
                            self.loan_posting(
                                event,
                                l,
                                l.terms.borrower,
                                l.lender,
                                result.principal_paid,
                                EconomicKind::LoanPrincipal,
                                0,
                            );
                        }
                        if result.interest_paid > 0 {
                            self.loan_posting(
                                event,
                                l,
                                l.terms.borrower,
                                l.lender,
                                result.interest_paid,
                                EconomicKind::Interest,
                                if result.principal_paid > 0 { 4096 } else { 0 },
                            );
                        }
                    }
                }
                *self.loans.iter_mut().find(|v| v.id == *loan).unwrap() = result.loan;
            }
            _ => unreachable!(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn loan_posting(
        &mut self,
        event: &Event,
        loan: &Loan,
        from: MemberId,
        to: MemberId,
        amount: i64,
        kind: EconomicKind,
        offset: u64,
    ) {
        self.record_transaction(Transaction {
            meta: crate::ledger::TransactionMeta::default(),
            id: event.sequence + offset,
            actor: event.actor,
            created_at: event.timestamp,
            from,
            to,
            amount,
            memo: TransactionMemo::try_from(if kind == EconomicKind::Interest {
                "Loan interest"
            } else {
                "Loan principal"
            })
            .unwrap(),
            reverses: None,
            listing: None,
            usd: false,
            quote: None,
            loan: Some(loan.id),
            lotto: None,
            economic: EconomicDetails {
                kind,
                ..EconomicDetails::default()
            },
        });
    }
}
