//! Optional Windows DLL / ESP-IDF static boundary, with no Rust fallback.
use crate::domain::Error;
use crate::domain::{Command, Event, MemberId, Side, State, Transaction};
#[cfg(target_os = "windows")]
use libloading::Library;
use std::sync::{Mutex, OnceLock};

type BankCall = unsafe extern "C" fn(i32, *mut i64) -> i32;
#[cfg(target_os = "windows")]
type Metadata = unsafe extern "C" fn() -> i32;
#[cfg(target_os = "windows")]
type Capabilities = unsafe extern "C" fn() -> u64;
struct Kernel {
    // Keeps the entry point and the GnuCOBOL runtime alive.
    #[cfg(target_os = "windows")]
    _library: Library,
    bank: BankCall,
}
static KERNEL: OnceLock<Result<Mutex<Kernel>, String>> = OnceLock::new();

#[cfg(target_os = "espidf")]
unsafe extern "C" {
    fn nc_bank_abi() -> i32;
    fn nc_bank_slots() -> i32;
    fn nc_bank_capabilities() -> u64;
    fn nc_bank_v7(operation: i32, slots: *mut i64) -> i32;
}

fn kernel() -> Result<&'static Mutex<Kernel>, &'static str> {
    KERNEL
        .get_or_init(|| {
            #[cfg(target_os = "windows")]
            {
                let path = std::env::var_os("NANACOIN_COBOL_DLL")
                    .ok_or("Set NANACOIN_COBOL_DLL to the built banking DLL")?;
                let path = std::path::PathBuf::from(path);
                if !path.is_absolute() {
                    return Err("NANACOIN_COBOL_DLL must be absolute".into());
                }
                // LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR requires a Windows-normalized
                // fully qualified path, including when the caller supplied C:/... .
                let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
                // SAFETY: explicitly selected local library built from cobol/bridge.c.
                // nc_bank_v7 uses fixed-width scalars, has no retained Rust pointers,
                // and all calls are serialized. The Library owns the symbol lifetime.
                unsafe {
                    let library: Library = libloading::os::windows::Library::load_with_flags(
                        path,
                        libloading::os::windows::LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
                            | libloading::os::windows::LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                    )
                    .map_err(|e| format!("{e:?}"))?
                    .into();
                    let abi = library
                        .get::<Metadata>(b"nc_bank_abi\0")
                        .map_err(|e| e.to_string())?;
                    let slots = library
                        .get::<Metadata>(b"nc_bank_slots\0")
                        .map_err(|e| e.to_string())?;
                    let capabilities = library
                        .get::<Capabilities>(b"nc_bank_capabilities\0")
                        .map_err(|e| e.to_string())?;
                    if abi() != 7 || slots() != 64 || capabilities() != 0xFFFFFFFFFFFFFFFE {
                        return Err("COBOL banking ABI mismatch; rebuild the module".into());
                    }
                    let bank = *library
                        .get::<BankCall>(b"nc_bank_v7\0")
                        .map_err(|e| e.to_string())?;
                    Ok(Mutex::new(Kernel {
                        _library: library,
                        bank,
                    }))
                }
            }
            #[cfg(target_os = "espidf")]
            {
                // SAFETY: the build requires an explicitly generated target
                // module with this ABI. Static symbols live for the process.
                // The same mutex serializes initialization and every entry.
                unsafe {
                    if nc_bank_abi() != 7
                        || nc_bank_slots() != 64
                        || nc_bank_capabilities() != 0xFFFFFFFFFFFFFFFE
                    {
                        return Err("COBOL banking ABI mismatch; rebuild the target module".into());
                    }
                }
                Ok(Mutex::new(Kernel { bank: nc_bank_v7 }))
            }
        })
        .as_ref()
        .map_err(String::as_str)
}

fn call(operation: i32, inputs: &[i64]) -> Result<[i64; 64], Error> {
    let mut frame = [0; 64];
    frame[..inputs.len()].copy_from_slice(inputs);
    let guard = kernel()
        .map_err(|_| Error::Unavailable)?
        .lock()
        .map_err(|_| Error::Unavailable)?;
    // SAFETY: the ABI owns exactly 64 initialized i64 slots for this call. The
    // module retains no pointers; the mutex serializes GnuCOBOL runtime state.
    let result = unsafe { (guard.bank)(operation, frame.as_mut_ptr()) };
    match result {
        0 => Ok(frame),
        1 => Err(Error::InvalidInput),
        2 => Err(Error::Overflow),
        3 => Err(Error::InsufficientFunds),
        4 => Err(Error::Conflict),
        5 => Err(Error::Forbidden),
        6 => Err(Error::Disabled),
        7 => Err(Error::NotFound),
        8 => Err(Error::Capacity),
        11 => Err(Error::ListingClosed),
        12 => Err(Error::SelfDeal),
        13 => Err(Error::BotGoodDeed),
        14 => Err(Error::OfferClosed),
        15 => Err(Error::OfferSettled),
        16 => Err(Error::MemberQuoteLimit),
        17 => Err(Error::MemberLoanLimit),
        18 => Err(Error::StaleRequest),
        19 => Err(Error::CorruptJournal),
        _ => Err(Error::Unavailable),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PostingPlan {
    pub debit: i64,
    pub credit: i64,
}

pub(crate) fn market_view(values: &[i64]) -> Result<[i64; 64], Error> {
    call(62, values)
}

pub(crate) fn newest_first<T, const N: usize>(
    rows: &[T],
    id: impl Fn(&T) -> u64,
) -> Result<heapless::Vec<&T, N>, Error> {
    if rows.len() > N {
        return Err(Error::Capacity);
    }
    let mut consumed = [false; N];
    let mut result = heapless::Vec::new();
    while result.len() < rows.len() {
        let mut best = [-1, 0];
        for (chunk, records) in rows.chunks(30).enumerate() {
            let mut frame = [0; 64];
            frame[..4].copy_from_slice(&[12, records.len() as i64, best[0], best[1]]);
            for (local, row) in records.iter().enumerate() {
                let index = chunk * 30 + local;
                frame[4 + local * 2] = index as i64;
                frame[5 + local * 2] = if consumed[index] {
                    0
                } else {
                    ledger_number(id(row))
                };
            }
            best.copy_from_slice(&call(62, &frame)?[16..18]);
        }
        let index = usize::try_from(best[0]).map_err(|_| Error::Unavailable)?;
        let row = rows.get(index).ok_or(Error::Unavailable)?;
        if consumed[index] {
            return Err(Error::Unavailable);
        }
        consumed[index] = true;
        result.push(row).map_err(|_| Error::Capacity)?;
    }
    Ok(result)
}

pub(crate) fn identity_query(values: &[i64]) -> Result<[i64; 64], Error> {
    call(9, values)
}

pub(crate) fn wire_quantity(whole: u32, fraction: u32, digits: usize) -> Result<u32, Error> {
    let out = call(49, &[6, whole.into(), fraction.into(), digits as i64])?;
    Ok(out[16] as u32)
}

pub(crate) fn household_stats(state: &State, now: u64) -> [heapless::String<256>; 2] {
    use core::fmt::Write;
    fn stats(values: &[i64]) -> [i64; 64] {
        call(63, values).expect("valid household statistics ABI")
    }
    let n = ledger_number;
    let usable = |t: &Transaction| {
        stats(&[
            0,
            t.usd.into(),
            n(t.created_at),
            n(now),
            t.reverses.is_some().into(),
            state.ledger.reversed_by(t.id).is_some().into(),
        ])[16]
            != 0
    };
    let mut employment = [0, 0];
    for m in &state.members {
        let eligible = stats(&[
            1,
            m.disabled.into(),
            (m.role == crate::domain::Role::Nana).into(),
        ])[16];
        let labor = eligible != 0
            && state.history.iter().any(|t| {
                usable(t)
                    && stats(&[
                        2,
                        t.to.0.into(),
                        m.id.0.into(),
                        t.amount,
                        economic_tag(t.economic.kind),
                    ])[16]
                        != 0
            });
        let out = stats(&[5, eligible, labor.into(), employment[0], employment[1]]);
        employment.copy_from_slice(&out[16..18]);
    }
    let mut inflation = [0, 0];
    for thing in &state.things {
        let mut sales = [-1, -1, 0];
        for (index, t) in state.history.iter().enumerate().rev() {
            let eligible = usable(t)
                && stats(&[
                    3,
                    economic_tag(t.economic.kind),
                    n(t.economic.thing),
                    n(thing.id),
                    t.economic.quantity_milli.into(),
                    t.amount,
                ])[16]
                    != 0;
            let out = stats(&[
                12,
                eligible.into(),
                index as i64,
                sales[0],
                sales[1],
                sales[2],
            ]);
            sales.copy_from_slice(&out[16..19]);
        }
        if sales[2] == 2 {
            let latest = &state.history[sales[0] as usize];
            let previous = &state.history[sales[1] as usize];
            let latest_amount = state.current_amount(latest, latest.amount);
            let prior_amount = state.current_amount(previous, previous.amount);
            let out = stats(&[
                4,
                latest_amount.is_ok().into(),
                prior_amount.is_ok().into(),
                latest_amount.unwrap_or(0),
                latest.economic.quantity_milli.into(),
                prior_amount.unwrap_or(0),
                previous.economic.quantity_milli.into(),
                inflation[0],
                inflation[1],
            ]);
            inflation.copy_from_slice(&out[16..18]);
        }
    }
    let aggregates = stats(&[6, inflation[0], inflation[1], employment[0], employment[1]]);
    let mut first = heapless::String::new();
    let _ = first.push_str("Retained year\nInflation: ");
    if aggregates[16] != 0 {
        let _ = write!(first, "{:.1}%", f64::from_bits(aggregates[17] as u64));
    } else {
        let _ = first.push_str("No data");
    }
    if aggregates[18] != 0 {
        let _ = write!(
            first,
            "\nEmployment: {:.0}% ({}/{})",
            f64::from_bits(aggregates[19] as u64),
            employment[1],
            employment[0]
        );
    } else {
        let _ = first.push_str("\nEmployment: No data");
    }

    let mut exchange: Option<(u64, i64)> = None;
    for q in &state.quotes {
        let out = stats(&[
            7,
            (q.status == crate::forex::QuoteStatus::Filled).into(),
            q.coin_tx
                .is_some_and(|id| state.ledger.reversed_by(id).is_some())
                .into(),
            q.cash_tx
                .is_some_and(|id| state.ledger.reversed_by(id).is_some())
                .into(),
            exchange.is_some().into(),
            exchange.map_or(0, |v| n(v.0)),
            n(q.updated_at),
            q.cents_per_coin,
        ]);
        if out[16] != 0 {
            exchange = Some((q.updated_at, stats(&[11, q.cents_per_coin])[17]));
        }
    }
    for coin in state.history.iter().rev() {
        let candidate = stats(&[
            8,
            coin.usd.into(),
            coin.quote.is_some().into(),
            coin.reverses.is_some().into(),
            coin.amount,
            state.ledger.reversed_by(coin.id).is_some().into(),
            exchange.is_some().into(),
            exchange.map_or(0, |v| n(v.0)),
            n(coin.created_at),
        ]);
        if candidate[17] != 0 {
            break;
        }
        if candidate[16] == 0 {
            continue;
        }
        if let Some(cash) = state.history.iter().find(|cash| {
            stats(&[
                9,
                cash.usd.into(),
                (cash.quote == coin.quote).into(),
                cash.reverses.is_some().into(),
                cash.amount,
                state.ledger.reversed_by(cash.id).is_some().into(),
            ])[16]
                != 0
        }) {
            let amount = state.current_amount(coin, coin.amount);
            let out = stats(&[
                10,
                amount.is_ok().into(),
                amount.unwrap_or(0),
                cash.amount,
                state.decimals.into(),
            ]);
            if out[16] != 0 {
                exchange = Some((coin.created_at, out[17]));
                break;
            }
        }
    }
    let mut second = heapless::String::new();
    let _ = second.push_str("Interest: ");
    if let Some(percent) = loan_summary(state).annual_percent {
        let _ = write!(second, "{percent:.2}%/yr");
    } else {
        let _ = second.push_str("No active loans");
    }
    let _ = second.push_str("\nExchange: ");
    if let Some((_, bits)) = exchange {
        let _ = write!(second, "${:.2}/NC", f64::from_bits(bits as u64));
    } else {
        let _ = second.push_str("No trades");
    }
    [first, second]
}

pub(crate) fn offer_phase(phase: crate::offers::OfferPhase) -> i64 {
    use crate::offers::OfferPhase;
    match phase {
        OfferPhase::Open => 0,
        OfferPhase::Accepted(_) => 1,
        OfferPhase::Declined => 2,
        OfferPhase::Withdrawn => 3,
        OfferPhase::Reversed(_) => 4,
    }
}

fn listing_status_tag(status: crate::domain::ListingStatus) -> i64 {
    use crate::domain::ListingStatus;
    match status {
        ListingStatus::Active => 0,
        ListingStatus::Sold => 1,
        ListingStatus::Cancelled => 2,
    }
}

pub(crate) fn offer_projection(
    offer: &crate::offers::Offer,
    now: u64,
    viewer: Option<&crate::domain::Member>,
) -> [i64; 64] {
    offer_projection_with_listing(offer, now, viewer, None)
}

fn offer_projection_with_listing(
    offer: &crate::offers::Offer,
    now: u64,
    viewer: Option<&crate::domain::Member>,
    listing_active: Option<bool>,
) -> [i64; 64] {
    call(
        21,
        &[
            offer_phase(offer.phase),
            i64::try_from(now).unwrap_or(i64::MAX),
            offer.settlement().map_or(0, |s| s.settles_at as i64),
            viewer.map_or(0, |m| m.id.0.into()),
            viewer
                .is_some_and(|m| m.role == crate::domain::Role::Nana)
                .into(),
            viewer.is_some_and(|m| m.disabled).into(),
            offer.owner.0.into(),
            offer.offerer.0.into(),
            0,
            0,
            0,
            0,
            0,
            listing_active.map_or(-1, i64::from),
        ],
    )
    .expect("valid offer projection ABI")
}

pub(crate) fn offer_view_status(
    state: &State,
    offer: &crate::offers::Offer,
    now: u64,
) -> &'static str {
    let active = state
        .listing(offer.listing)
        .is_ok_and(|l| l.status == crate::domain::ListingStatus::Active);
    match offer_projection_with_listing(offer, now, None, Some(active))[8] {
        0 => "OPEN",
        1 => "ACCEPTED",
        2 => "DECLINED",
        3 => "WITHDRAWN",
        4 => "REVERSED",
        5 => "SETTLED",
        6 => "NOT_SELECTED",
        _ => unreachable!("valid projected offer status"),
    }
}

pub(crate) fn offer_profile_selected(
    state: &State,
    offer: &crate::offers::Offer,
    subject: MemberId,
    now: u64,
) -> bool {
    let active = state
        .listing(offer.listing)
        .is_ok_and(|l| l.status == crate::domain::ListingStatus::Active);
    let status = offer_projection_with_listing(offer, now, None, Some(active))[8];
    call(62, &[4, offer.offerer.0.into(), subject.0.into(), status])
        .expect("valid offer profile ABI")[16]
        != 0
}

#[derive(Debug)]
pub(crate) struct OfferPlan {
    pub phase: i64,
    pub payer: MemberId,
    pub payee: MemberId,
    pub settles_at: u64,
    pub mutate_listing: bool,
    pub recycle: Option<usize>,
}

impl OfferPlan {
    pub(crate) fn phase(
        &self,
        settlement: Option<crate::offers::Settlement>,
    ) -> crate::offers::OfferPhase {
        use crate::offers::OfferPhase;
        match self.phase {
            0 => OfferPhase::Open,
            1 => OfferPhase::Accepted(settlement.expect("prepared accepted settlement")),
            2 => OfferPhase::Declined,
            3 => OfferPhase::Withdrawn,
            4 => OfferPhase::Reversed(settlement.expect("prepared reversed settlement")),
            _ => unreachable!("prepared offer phase"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct QuotePlan {
    pub status: crate::forex::QuoteStatus,
    pub seller: MemberId,
    pub buyer: MemberId,
    pub cents: i64,
    pub recycle: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct GiftPlan {
    pub received: i64,
    pub closed: bool,
    pub owner: MemberId,
}

pub(crate) fn loan_tag(status: crate::loans::LoanStatus) -> i64 {
    use crate::loans::LoanStatus;
    match status {
        LoanStatus::Offered => 0,
        LoanStatus::Armed => 1,
        LoanStatus::Active => 2,
        LoanStatus::Paid => 3,
        LoanStatus::Declined => 4,
        LoanStatus::Cancelled => 5,
        LoanStatus::Requested => 6,
    }
}

fn loan_status(tag: i64) -> Result<crate::loans::LoanStatus, Error> {
    use crate::loans::LoanStatus;
    Ok(match tag {
        0 => LoanStatus::Offered,
        1 => LoanStatus::Armed,
        2 => LoanStatus::Active,
        3 => LoanStatus::Paid,
        4 => LoanStatus::Declined,
        5 => LoanStatus::Cancelled,
        6 => LoanStatus::Requested,
        _ => return Err(Error::Unavailable),
    })
}

pub(crate) fn loan_projection(
    state: Option<&State>,
    loan: &crate::loans::Loan,
    viewer: Option<&crate::domain::Member>,
    now: u64,
) -> [i64; 64] {
    loan_projection_details(state, loan, viewer, now, false, None)
}

fn loan_projection_details(
    state: Option<&State>,
    loan: &crate::loans::Loan,
    viewer: Option<&crate::domain::Member>,
    now: u64,
    accrual_failed: bool,
    subject: Option<MemberId>,
) -> [i64; 64] {
    let borrower = state.and_then(|s| s.member(loan.terms.borrower).ok());
    let lender = state.and_then(|s| s.member(loan.lender).ok());
    call(
        16,
        &[
            loan_tag(loan.status),
            viewer
                .is_some_and(|m| m.role == crate::domain::Role::Nana)
                .into(),
            viewer.map_or(0, |m| m.id.0.into()),
            loan.lender.0.into(),
            loan.terms.borrower.0.into(),
            borrower.is_some().into(),
            lender.is_some().into(),
            borrower.is_some_and(|m| m.disabled).into(),
            lender.is_some_and(|m| m.disabled).into(),
            borrower.map_or(0, |m| m.balance),
            lender.map_or(0, |m| m.balance),
            loan.terms.amount,
            borrower
                .is_some_and(|b| state.unwrap().credit_blocked & (1u32 << (b.id.0 - 1)) != 0)
                .into(),
            loan.next_due_at as i64,
            now.min(i64::MAX as u64) as i64,
            loan.principal_due,
            loan.interest_due,
            loan.attempted_balance,
            accrual_failed.into(),
            subject.map_or(0, |m| m.0.into()),
        ],
    )
    .expect("initialized loan projection")
}

pub(crate) fn loan_included(
    state: &State,
    loan: &crate::loans::Loan,
    viewer: &crate::domain::Member,
    subject: Option<MemberId>,
) -> bool {
    loan_projection_details(Some(state), loan, Some(viewer), 0, false, subject)[24] == 1
}

pub(crate) fn loan_view_values(
    state: &State,
    loan: &crate::loans::Loan,
    now: u64,
) -> (i64, i64, &'static str) {
    let mut accrued = loan.clone();
    let projection = loan_projection(Some(state), loan, None, now);
    let failed = projection[27] == 1 && accrued.accrue(now).is_err();
    let projection = loan_projection_details(Some(state), loan, None, now, failed, None);
    let reason = match projection[23] {
        0 => "",
        1 => "An account is disabled",
        2 => "Clock or arithmetic limit; payment is paused",
        3 => "Waiting for a zero balance",
        4 => "Credit does not fund loan payments",
        5 => "Waiting for lender funds",
        6 => "Ready for automatic funding",
        _ => unreachable!("projected loan waiting reason"),
    };
    (accrued.interest, projection[26], reason)
}

pub(crate) struct LoanSummary {
    pub outstanding: i64,
    pub overdue: i64,
    pub active: usize,
    pub annual_percent: Option<f64>,
}

pub(crate) fn loan_summary(state: &State) -> LoanSummary {
    let mut totals = [0; 5];
    for loan in &state.loans {
        let mut frame = [0; 64];
        frame[..6].copy_from_slice(&[
            loan_tag(loan.status),
            loan.principal,
            loan.principal_due,
            loan.interest_due,
            loan.terms.rate_bps.into(),
            loan.terms.rate_days.into(),
        ]);
        frame[6..11].copy_from_slice(&totals);
        let result = call(19, &frame).expect("validated loan summary row");
        totals.copy_from_slice(&result[6..11]);
    }
    let mut frame = [0; 64];
    frame[0] = -1;
    frame[6..11].copy_from_slice(&totals);
    frame[11] = 1;
    let result = call(19, &frame).expect("validated loan summary total");
    LoanSummary {
        outstanding: result[6],
        overdue: result[7],
        active: result[8] as usize,
        annual_percent: (result[12] == 1).then(|| f64::from_bits(result[13] as u64)),
    }
}

#[derive(Debug)]
pub(crate) struct LoanPlan {
    pub status: crate::loans::LoanStatus,
    pub lender: MemberId,
    pub borrower: MemberId,
    pub draw: i64,
    pub next_due: u64,
    pub principal: i64,
    pub accrued: u64,
    pub updated: u64,
    pub recycle: Option<usize>,
}

impl LoanPlan {
    pub(crate) fn install(&self, loan: &mut crate::loans::Loan) {
        loan.status = self.status;
        loan.lender = self.lender;
        loan.terms.borrower = self.borrower;
        loan.next_due_at = self.next_due;
        loan.principal = self.principal;
        loan.accrued_at = self.accrued;
        loan.updated_at = self.updated;
    }
}

impl State {
    pub(crate) fn account_view_plan(
        &self,
        member: MemberId,
        usd: bool,
    ) -> Result<[i64; 64], Error> {
        let m = self.member(member).ok();
        call(
            62,
            &[
                7,
                member.0.into(),
                m.is_some().into(),
                usd.into(),
                self.issuance_balance,
                self.usd_issuance_balance,
                m.map_or(0, |m| m.balance),
                m.map_or(0, |m| m.usd_cents),
                m.is_some_and(|m| m.disabled).into(),
            ],
        )
    }

    pub(crate) fn active_listing_count(&self) -> Result<usize, Error> {
        let mut count = 0;
        for listing in &self.listings {
            count = call(62, &[5, count, listing_status_tag(listing.status)])?[16];
        }
        usize::try_from(count).map_err(|_| Error::Unavailable)
    }

    pub(crate) fn loan_plan(
        &self,
        actor: MemberId,
        c: &Command,
        now: u64,
    ) -> Result<LoanPlan, Error> {
        let mut frame = [0; 64];
        frame[1] = actor.0.into();
        frame[2] = now.min(i64::MAX as u64) as i64;
        let (action, terms, id) = match c {
            Command::OfferLoan { terms } => (0, Some(terms), None),
            Command::RequestLoan { terms } => (1, Some(terms), None),
            Command::RespondLoan { terms, loan } => (2, Some(terms), Some(*loan)),
            Command::AcceptLoan { loan } => (3, None, Some(*loan)),
            Command::CloseLoan { loan } => (4, None, Some(*loan)),
            Command::RepayLoan { loan, .. } => (5, None, Some(*loan)),
            Command::RunLoan {
                loan,
                expected_updated_at,
            } => {
                frame[21] = (*expected_updated_at).min(i64::MAX as u64) as i64;
                (6, None, Some(*loan))
            }
            _ => return Err(Error::InvalidInput),
        };
        frame[0] = action;
        let loan = id.and_then(|id| self.loan(id).ok());
        let terms = terms.or_else(|| loan.map(|l| &l.terms));
        if let Some(t) = terms {
            let b = self.member(t.borrower).ok();
            frame[3] = b.is_some().into();
            frame[4] = b.is_some_and(|m| m.disabled).into();
            frame[5] = t.borrower.0.into();
            frame[6] = t.amount;
            frame[7] = t.installment;
            frame[8] = t.rate_days.into();
            frame[9] = t.payment_days.into();
            frame[10] = t.memo.chars().any(char::is_control).into();
            frame[17] = t.credit.into();
        }
        if let Some(l) = loan {
            frame[11] = 1;
            frame[12] = loan_tag(l.status);
            frame[13] = l.terms.borrower.0.into();
            frame[14] = l.lender.0.into();
            let lender = self.member(l.lender).ok();
            frame[15] = lender.is_some().into();
            frame[16] = lender.is_some_and(|m| m.disabled).into();
            frame[22] = l.updated_at as i64;
            frame[23] = loan_projection(Some(self), l, None, now)[22];
            frame[24] = l.principal;
            frame[25] = l.accrued_at as i64;
            frame[26] = l.next_due_at as i64;
        }
        let mut book = [0; 64];
        book[0] = actor.0.into();
        book[1] = self.loans.len() as i64;
        for (index, loan) in self.loans.iter().enumerate() {
            book[index + 2] = i64::from(loan.lender.0) * 8 + loan_tag(loan.status);
        }
        let book = call(18, &book)?;
        frame[18] = book[34];
        frame[19] = self.loans.is_full().into();
        frame[20] = (book[35] != 0).into();
        let out = call(15, &frame)?;
        let plan = LoanPlan {
            status: loan_status(out[30])?,
            lender: MemberId(out[31] as u8),
            borrower: MemberId(out[32] as u8),
            draw: out[33],
            next_due: out[34] as u64,
            principal: out[35],
            accrued: out[36] as u64,
            updated: out[37] as u64,
            recycle: if action <= 1 && self.loans.is_full() {
                let mut rows = heapless::Vec::<(i64, i64), 32>::new();
                for (index, loan) in self.loans.iter().enumerate() {
                    rows.push((
                        if book[35] & (1i64 << index) != 0 {
                            0
                        } else {
                            -1
                        },
                        loan.id as i64,
                    ))
                    .unwrap();
                }
                Some(oldest_record(&rows)?.ok_or(Error::Capacity)?)
            } else {
                None
            },
        };
        if plan.draw > 0 {
            self.validate_posting(plan.lender, plan.borrower, plan.draw, false)?;
        }
        if out[38] != 0 {
            return Err(Error::Overflow);
        }
        Ok(plan)
    }
}

#[derive(Debug)]
pub(crate) struct ArtPlan {
    pub owner: MemberId,
    pub price: Option<i64>,
    pub equipped: bool,
    pub revision: u64,
    pub seller: MemberId,
    pub amount: i64,
    pub clear_equipment: u64,
}

impl ArtPlan {
    pub(crate) fn install(&self, art: &mut crate::commerce::Artwork) {
        art.owner = self.owner;
        art.price = self.price;
        art.equipped = self.equipped;
        art.revision = self.revision;
    }
}

pub(crate) fn art_metadata(title: &str, license: &str, hash: &str, url: &str) -> Result<(), Error> {
    let mut frame = [0; 64];
    frame[0] = title.trim().is_empty().into();
    frame[1] = license.trim().is_empty().into();
    frame[2] = hash.len() as i64;
    frame[3] = url.len() as i64;
    let mut bytes = [0; 256];
    let hash_len = hash.len().min(64);
    let url_len = url.len().min(192);
    bytes[..hash_len].copy_from_slice(&hash.as_bytes()[..hash_len]);
    bytes[64..64 + url_len].copy_from_slice(&url.as_bytes()[..url_len]);
    for (index, bytes) in bytes.chunks_exact(8).enumerate() {
        frame[4 + index] = i64::from_ne_bytes(bytes.try_into().unwrap());
    }
    call(59, &frame).map(|_| ())
}

impl State {
    pub(crate) fn art_plan(
        &self,
        actor: MemberId,
        action: &crate::commerce::Action,
        sequence: u64,
    ) -> Result<ArtPlan, Error> {
        use crate::commerce::Action;
        let mut frame = [0; 64];
        frame[1] = actor.0.into();
        frame[2] = self.member(actor)?.disabled.into();
        frame[3] = (self.commerce.artworks.len() >= crate::commerce::ARTWORKS).into();
        frame[18] = sequence as i64;
        let id = match action {
            Action::MintArt {
                title,
                license,
                sha256,
                locator,
            } => {
                frame[17] = match art_metadata(title, license, sha256, locator) {
                    Ok(()) => 1,
                    Err(Error::InvalidInput) => 0,
                    Err(error) => return Err(error),
                };
                None
            }
            Action::ListArt { art, price } => {
                frame[0] = 1;
                frame[19] = price.is_some().into();
                frame[20] = price.unwrap_or(0);
                Some(*art)
            }
            Action::BuyArt {
                art,
                expected_owner,
                expected_revision,
                expected_price,
            } => {
                frame[0] = 2;
                frame[10] = expected_owner.0.into();
                frame[11] = (*expected_revision).min(i64::MAX as u64) as i64;
                frame[12] = *expected_price;
                Some(*art)
            }
            Action::GiftArt { art, to } => {
                frame[0] = 3;
                frame[13] = to.0.into();
                let recipient = self.member(*to).ok();
                frame[14] = recipient.is_some().into();
                frame[15] = recipient.is_some_and(|m| m.disabled).into();
                Some(*art)
            }
            Action::EquipArt { art, equipped } => {
                frame[0] = 4;
                frame[16] = (*equipped).into();
                Some(*art)
            }
            _ => unreachable!("art command"),
        };
        if let Some(art) = id.and_then(|id| self.artwork(id).ok()) {
            frame[4] = 1;
            frame[5] = art.owner.0.into();
            frame[6] = art.price.is_some().into();
            frame[7] = art.price.unwrap_or(0);
            frame[8] = art.equipped.into();
            frame[9] = art.revision as i64;
        }
        let out = call(58, &frame)?;
        let mut plan = ArtPlan {
            owner: MemberId(out[30] as u8),
            price: (out[31] == 1).then_some(out[32]),
            equipped: out[33] == 1,
            revision: out[34] as u64,
            seller: MemberId(out[35] as u8),
            amount: out[36],
            clear_equipment: 0,
        };
        if matches!(action, Action::BuyArt { .. }) {
            self.validate_posting(actor, plan.seller, plan.amount, false)?;
        }
        if out[37] == 1 {
            for (chunk, rows) in self.commerce.artworks.chunks(32).enumerate() {
                let mut input = [0; 64];
                input[0] = actor.0.into();
                input[1] = rows.len() as i64;
                for (index, art) in rows.iter().enumerate() {
                    input[index + 2] = art.owner.0.into();
                }
                plan.clear_equipment |= (call(60, &input)?[63] as u64) << (chunk * 32);
            }
        }
        Ok(plan)
    }
}

impl State {
    pub(crate) fn gift_plan(
        &self,
        actor: MemberId,
        action: &crate::commerce::Action,
        now: u64,
    ) -> Result<GiftPlan, Error> {
        use crate::commerce::Action;
        let mut frame = [0; 64];
        frame[1] = actor.0.into();
        frame[2] = self.member(actor)?.disabled.into();
        frame[3] = now.min(i64::MAX as u64) as i64;
        frame[4] = (self.commerce.requests.len() >= crate::commerce::REQUESTS).into();
        match action {
            Action::CreateRequest {
                title,
                target,
                deadline,
                ..
            } => {
                frame[8] = deadline.is_some().into();
                frame[9] = deadline.map_or(0, |v| v.min(i64::MAX as u64) as i64);
                frame[10] = target.is_some().into();
                frame[11] = target.unwrap_or(0);
                frame[12] = title.trim().is_empty().into();
            }
            Action::CloseRequest { request } | Action::Contribute { request, .. } => {
                frame[0] = if matches!(action, Action::CloseRequest { .. }) {
                    1
                } else {
                    2
                };
                if let Ok(r) = self.gift_request(*request) {
                    frame[5] = 1;
                    frame[6] = r.owner.0.into();
                    frame[7] = r.closed.into();
                    frame[8] = r.deadline.is_some().into();
                    frame[9] = r.deadline.unwrap_or(0) as i64;
                    frame[13] = r.received;
                }
                if let Action::Contribute { amount, .. } = action {
                    frame[14] = *amount;
                }
            }
            _ => unreachable!("gift command"),
        }
        let out = call(55, &frame)?;
        let owner = MemberId(out[23] as u8);
        if let Action::Contribute { amount, .. } = action {
            self.validate_posting(actor, owner, *amount, false)?;
            if out[21] != 0 {
                return Err(Error::Overflow);
            }
        }
        Ok(GiftPlan {
            received: out[20],
            closed: out[22] == 1,
            owner,
        })
    }
}

fn quote_side(side: crate::forex::QuoteSide) -> i64 {
    match side {
        crate::forex::QuoteSide::ASK => 0,
        crate::forex::QuoteSide::BID => 1,
    }
}

fn quote_status(status: crate::forex::QuoteStatus) -> i64 {
    match status {
        crate::forex::QuoteStatus::Open => 0,
        crate::forex::QuoteStatus::Filled => 1,
        crate::forex::QuoteStatus::Cancelled => 2,
    }
}

pub(crate) fn quote_projection(q: &crate::forex::Quote, now: u64, taker: MemberId) -> [i64; 64] {
    call(
        50,
        &[
            quote_status(q.status),
            now.min(i64::MAX as u64) as i64,
            q.expires_at as i64,
            quote_side(q.side),
            q.maker.0.into(),
            taker.0.into(),
        ],
    )
    .expect("initialized quote projection")
}

pub(crate) fn quote_view_status(q: &crate::forex::Quote, now: u64) -> &'static str {
    match quote_projection(q, now, MemberId(0))[7] {
        0 => "OPEN",
        1 => "FILLED",
        2 => "CANCELLED",
        3 => "EXPIRED",
        _ => unreachable!("projected quote status"),
    }
}

pub(crate) fn quote_order(state: &State) -> Result<heapless::Vec<&crate::forex::Quote, 16>, Error> {
    let mut frame = [0; 64];
    for index in 0..16 {
        frame[index * 3] = -1;
    }
    for (index, q) in state.quotes.iter().enumerate() {
        frame[index * 3] = quote_side(q.side);
        frame[index * 3 + 1] = q.cents_per_coin;
        frame[index * 3 + 2] = q.id as i64;
    }
    let out = call(52, &frame)?;
    let mut rows = heapless::Vec::new();
    for index in &out[48..48 + state.quotes.len()] {
        rows.push(
            state
                .quotes
                .get(*index as usize)
                .ok_or(Error::Unavailable)?,
        )
        .map_err(|_| Error::Capacity)?;
    }
    Ok(rows)
}

impl State {
    pub(crate) fn quote_plan(
        &self,
        actor: MemberId,
        c: &Command,
        now: u64,
    ) -> Result<QuotePlan, Error> {
        let mut frame = [0; 64];
        frame[1] = actor.0.into();
        frame[2] = (self.member(actor)?.role == crate::domain::Role::Nana).into();
        frame[3] = now.min(i64::MAX as u64) as i64;
        match c {
            Command::PostQuote {
                side,
                cents_per_coin,
                coins,
                expires_at,
            } => {
                frame[6] = quote_side(*side);
                frame[9] = *cents_per_coin;
                frame[10] = *coins;
                frame[11] = crate::money::scale(self.decimals);
                frame[12] = (*expires_at).min(i64::MAX as u64) as i64;
            }
            Command::TakeQuote { quote } | Command::CancelQuote { quote } => {
                frame[0] = if matches!(c, Command::TakeQuote { .. }) {
                    1
                } else {
                    2
                };
                if let Ok(q) = self.quote(*quote) {
                    frame[4] = 1;
                    frame[5] = q.maker.0.into();
                    frame[6] = quote_side(q.side);
                    frame[7] = quote_status(q.status);
                    frame[8] = q.live(now).into();
                    frame[9] = q.cents_per_coin;
                    frame[10] = q.coins;
                    frame[11] = q.nc_scale;
                }
            }
            _ => unreachable!("quote command"),
        }
        for (index, q) in self.quotes.iter().enumerate() {
            frame[26 + index * 2] = q.maker.0.into();
            frame[27 + index * 2] = q.live(now).into();
        }
        let out = call(51, &frame)?;
        let plan = QuotePlan {
            status: match out[16] {
                0 => crate::forex::QuoteStatus::Open,
                1 => crate::forex::QuoteStatus::Filled,
                2 => crate::forex::QuoteStatus::Cancelled,
                _ => return Err(Error::Unavailable),
            },
            seller: MemberId(out[17] as u8),
            buyer: MemberId(out[18] as u8),
            cents: out[19],
            recycle: if matches!(c, Command::PostQuote { .. }) && self.quotes.is_full() {
                let mut rows = heapless::Vec::<(i64, i64), 16>::new();
                for q in &self.quotes {
                    rows.push((
                        if q.live(now) { -1 } else { q.updated_at as i64 },
                        q.id as i64,
                    ))
                    .unwrap();
                }
                Some(oldest_record(&rows)?.ok_or(Error::Capacity)?)
            } else {
                None
            },
        };
        if matches!(c, Command::TakeQuote { .. }) {
            self.validate_currency_posting(plan.seller, plan.buyer, frame[10], false, false)?;
            self.validate_currency_posting(plan.buyer, plan.seller, plan.cents, false, true)?;
        }
        Ok(plan)
    }
}

impl State {
    pub(crate) fn offer_plan(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<OfferPlan, Error> {
        use crate::domain::{EconomicKind, ListingStatus, MemberKind, Role};
        let (action, listing_id, offer_id, amount, text_control) = match command {
            Command::MakeOffer {
                listing,
                amount,
                message,
            } => (
                0,
                Some(*listing),
                None,
                *amount,
                message.chars().any(char::is_control),
            ),
            Command::AcceptOffer { offer } => (1, None, Some(*offer), 0, false),
            Command::UnacceptOffer { offer, reason } => (
                2,
                None,
                Some(*offer),
                0,
                reason.chars().any(char::is_control),
            ),
            Command::DeclineOffer { offer } => (3, None, Some(*offer), 0, false),
            Command::WithdrawOffer { offer } => (4, None, Some(*offer), 0, false),
            _ => return Err(Error::InvalidInput),
        };
        let offer = offer_id.and_then(|id| self.offer(id).ok());
        let listing = listing_id
            .or_else(|| offer.map(|o| o.listing))
            .and_then(|id| self.listing(id).ok());
        let settlement = offer.and_then(|o| o.settlement());
        let parties =
            offer.and_then(|o| self.member(o.owner).ok().zip(self.member(o.offerer).ok()));
        let disabled = match parties {
            Some((a, b)) => (a.disabled || b.disabled).into(),
            None => -1,
        };
        let full = self.offers.is_full();
        let recyclable = action == 0 && full && self.offers.iter().any(|o| o.recyclable(now));
        // Slots 1-29 are scalar facts and optional-record tags. COBOL owns
        // permission/status/deadline decisions; Rust resolves records and UTF-8.
        let out = call(
            22,
            &[
                action,
                i64::try_from(now).unwrap_or(i64::MAX),
                actor.0.into(),
                self.admin(actor).is_ok().into(),
                amount,
                text_control.into(),
                listing.is_some().into(),
                listing
                    .is_some_and(|l| l.status == ListingStatus::Active)
                    .into(),
                listing
                    .is_some_and(|l| l.status == ListingStatus::Sold)
                    .into(),
                listing.map_or(0, |l| l.owner.0.into()),
                listing.is_some_and(|l| l.side == Side::Buy).into(),
                listing.is_some_and(|l| l.is_good_deed()).into(),
                listing
                    .is_some_and(|l| l.economic.kind == EconomicKind::Labor)
                    .into(),
                offer
                    .map(|o| o.owner)
                    .and_then(|id| self.member(id).ok())
                    .is_some_and(|m| m.role == Role::Nana)
                    .into(),
                self.member(actor)
                    .is_ok_and(|m| m.kind == MemberKind::Bot)
                    .into(),
                full.into(),
                recyclable.into(),
                offer.is_some().into(),
                offer.map_or(-1, |o| offer_phase(o.phase)),
                offer.map_or(0, |o| o.owner.0.into()),
                offer.map_or(0, |o| o.offerer.0.into()),
                disabled,
                settlement.map_or(0, |s| s.settles_at as i64),
                settlement.map_or(0, |s| s.payer.0.into()),
                settlement.map_or(0, |s| s.payee.0.into()),
                i64::try_from(self.offer_settles_after).unwrap_or(i64::MAX),
                settlement
                    .is_none_or(|s| self.correction_room(s.transaction).is_ok())
                    .into(),
                settlement.map_or(0, |s| self.ledger.refunded(s.transaction)),
                settlement
                    .is_some_and(|s| self.ledger.reversed_by(s.transaction).is_some())
                    .into(),
                offer
                    .and_then(|o| self.member(o.offerer).ok())
                    .is_some_and(|m| m.role == Role::Nana)
                    .into(),
            ],
        )?;
        let payer = MemberId(out[31] as u8);
        let payee = MemberId(out[32] as u8);
        if action == 1 || action == 2 {
            self.validate_posting(
                payer,
                payee,
                offer.expect("validated offer exists").amount,
                action == 2,
            )?;
            if out[34] != 0 {
                return Err(Error::Overflow);
            }
        }
        let recycle = if action == 0 && full {
            let mut candidates = [0; 64];
            for index in 0..32 {
                candidates[index * 2] = -1;
            }
            for (index, o) in self.offers.iter().enumerate() {
                if o.recyclable(now) {
                    candidates[index * 2] = o.updated_at as i64;
                    candidates[index * 2 + 1] = o.id.0 as i64;
                }
            }
            let index = call(24, &candidates)?[0];
            Some(usize::try_from(index).map_err(|_| Error::Capacity)?)
        } else {
            None
        };
        Ok(OfferPlan {
            phase: out[30],
            payer,
            payee,
            settles_at: out[33] as u64,
            mutate_listing: out[35] != 0,
            recycle,
        })
    }
}

pub(crate) fn rescale(value: i64, exponent: i16, limit: i64) -> Result<i64, Error> {
    call(2, &[value, exponent.into(), limit]).map(|out| out[3])
}

pub(crate) fn exchange_cents(coins: i64, rate: i64, scale: i64) -> Result<i64, Error> {
    call(3, &[coins, rate, scale]).map(|out| out[3])
}

pub(crate) fn annual_rate(rate: u32, days: u16) -> u32 {
    call(4, &[rate.into(), days.into()]).expect("valid annual rate ABI")[2] as u32
}

pub(crate) fn policy(operation: i32, inputs: &[i64]) -> Result<(), Error> {
    call(operation, inputs).map(|_| ())
}

pub(crate) fn member_wire(
    blank: bool,
    bot: bool,
    nana: bool,
    grant: bool,
    initial: i64,
) -> Result<(bool, i64), Error> {
    let out = call(
        49,
        &[
            2,
            blank.into(),
            bot.into(),
            nana.into(),
            grant.into(),
            initial,
        ],
    )?;
    Ok((out[7] != 0, out[8]))
}

pub(crate) fn ledger(inputs: &[i64]) -> Result<[i64; 64], Error> {
    call(53, inputs)
}

pub(crate) fn activity(inputs: &[i64]) -> Result<[i64; 64], Error> {
    call(54, inputs)
}

pub(crate) fn ledger_number(value: u64) -> i64 {
    // Values above the signed ABI range necessarily exceed current ordinals/pages.
    // Saturation preserves comparisons and clamped limits for hostile cursors.
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(crate) fn ledger_transaction(tx: &Transaction) -> [i64; 64] {
    ledger(&[
        0,
        tx.amount,
        tx.reverses.is_some().into(),
        tx.listing.is_some().into(),
        tx.loan.is_some().into(),
        tx.lotto.is_some().into(),
        tx.quote.is_some().into(),
        tx.from.0.into(),
        tx.to.0.into(),
    ])
    .expect("valid ledger row ABI")
}

pub(crate) fn ledger_debit(amount: i64) -> i64 {
    ledger(&[7, amount]).expect("valid ledger amount ABI")[16]
}

pub(crate) fn ledger_minimum(rows: &[Transaction]) -> Option<usize> {
    let mut best: Option<(usize, i64)> = None;
    for (chunk, rows) in rows.chunks(60).enumerate() {
        let mut frame = [0; 64];
        frame[0] = 8;
        frame[1] = rows.len() as i64;
        for (i, t) in rows.iter().enumerate() {
            frame[i + 2] = ledger_number(t.meta.ordinal);
        }
        let out = ledger(&frame).expect("valid ledger selection ABI");
        let candidate = (chunk * 60 + out[62] as usize, out[63]);
        if best.is_none_or(|(_, value)| {
            ledger(&[8, 2, candidate.1, value]).expect("valid ledger merge ABI")[62] == 0
        }) {
            best = Some(candidate);
        }
    }
    best.map(|(index, _)| index)
}

pub(crate) fn recyclable(kind: i64, protected: bool, dependencies: bool) -> bool {
    call(28, &[kind, protected.into(), dependencies.into()]).expect("valid recycling policy ABI")[3]
        != 0
}

pub(crate) fn listing_status(status: i64) -> crate::domain::ListingStatus {
    use crate::domain::ListingStatus;
    match status {
        0 => ListingStatus::Active,
        1 => ListingStatus::Sold,
        2 => ListingStatus::Cancelled,
        _ => unreachable!("prepared listing status"),
    }
}

#[derive(Debug)]
pub(crate) struct ListingPlan {
    pub status: i64,
    pub payer: MemberId,
    pub payee: MemberId,
    pub recycle: Option<usize>,
    pub thing_index: Option<usize>,
    pub thing_recycle: Option<usize>,
    pub standard: bool,
}

// Merge bounded batches through the same COBOL comparison policy. Index
// translation is storage bookkeeping; no Rust timestamp ordering is used.
fn oldest_record(records: &[(i64, i64)]) -> Result<Option<usize>, Error> {
    let mut winner = None;
    for (chunk, rows) in records.chunks(32).enumerate() {
        let mut frame = [0; 64];
        for index in 0..32 {
            frame[index * 2] = -1;
        }
        for (index, (timestamp, id)) in rows.iter().enumerate() {
            frame[index * 2] = *timestamp;
            frame[index * 2 + 1] = *id;
        }
        let selected = call(24, &frame)?[0];
        if selected < 0 {
            continue;
        }
        let candidate = chunk * 32 + selected as usize;
        winner = match winner {
            None => Some(candidate),
            Some(previous) => {
                for index in 0..32 {
                    frame[index * 2] = -1;
                }
                (frame[0], frame[1]) = records[previous];
                (frame[2], frame[3]) = records[candidate];
                Some(if call(24, &frame)?[0] == 0 {
                    previous
                } else {
                    candidate
                })
            }
        };
    }
    Ok(winner)
}

impl State {
    pub(crate) fn listing_plan(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<ListingPlan, Error> {
        use crate::domain::{EconomicKind, ListingStatus, Role};
        let (action, id, title, price, details, side, economic, standard) = match command {
            Command::List {
                title,
                price,
                details,
                side,
                ..
            } => (
                0,
                None,
                Some(title),
                Some(*price),
                details.as_ref(),
                Some(*side),
                None,
                false,
            ),
            Command::ClassifiedList {
                title,
                price,
                side,
                economic,
                standard,
                ..
            } => (
                4,
                None,
                Some(title),
                Some(*price),
                None,
                Some(*side),
                Some(economic),
                *standard,
            ),
            Command::Cancel { listing } => (1, Some(*listing), None, None, None, None, None, false),
            Command::UpdateListing {
                listing,
                title,
                price,
                ..
            } => (
                2,
                Some(*listing),
                title.as_ref(),
                *price,
                None,
                None,
                None,
                false,
            ),
            Command::Buy { listing } => (3, Some(*listing), None, None, None, None, None, false),
            _ => return Err(Error::InvalidInput),
        };
        let listing = id.and_then(|id| self.listing(id).ok());
        let side = side.or_else(|| listing.map(|l| l.side));
        let economic = economic.or_else(|| listing.map(|l| &l.economic));
        let by_id = economic
            .filter(|e| e.thing != 0)
            .and_then(|e| self.things.iter().position(|t| t.id == e.thing));
        let by_name = title.and_then(|name| {
            self.things
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(name))
        });
        let same = |index: Option<usize>| {
            index.zip(economic).is_some_and(|(i, e)| {
                self.things[i].kind == e.kind && self.things[i].unit == e.unit
            })
        };
        let thing_index = by_id.or(by_name);
        let mut recycling_flags = [0; 64];
        if action == 0 || action == 4 {
            for (index, l) in self.listings.iter().enumerate() {
                recycling_flags[index] = self.listing_recyclable(l, now).into();
            }
        }
        let recycle = if self.listings.is_full() && (action == 0 || action == 4) {
            let index = call(27, &recycling_flags)?[0];
            usize::try_from(index).ok()
        } else {
            None
        };
        let mut thing_rows = [(-1, 0); 64];
        if action == 4 && self.things.is_full() {
            for (index, t) in self.things.iter().enumerate() {
                let dependencies = self
                    .listings
                    .iter()
                    .any(|l| l.economic.thing == t.id && l.status == ListingStatus::Active);
                if recyclable(1, t.standard, dependencies) {
                    thing_rows[index] = (t.updated_at as i64, t.id as i64);
                }
            }
        }
        let thing_recycle = if action == 4 && self.things.is_full() {
            oldest_record(&thing_rows)?
        } else {
            None
        };
        let kind = details.map_or(0, |d| match d.kind.as_str() {
            "" => 0,
            "item" => 1,
            "service" => 2,
            "currency" => 3,
            crate::domain::GOOD_DEED => 4,
            _ => -1,
        });
        let mut facts = [0; 39];
        facts[..30].copy_from_slice(&[
            action,
            actor.0.into(),
            self.admin(actor).is_ok().into(),
            self.member(actor)
                .is_ok_and(|m| m.role == Role::Nana)
                .into(),
            listing.is_some().into(),
            listing.map_or(0, |l| l.owner.0.into()),
            listing
                .is_some_and(|l| l.status == ListingStatus::Active)
                .into(),
            side.is_some_and(|s| s == Side::Buy).into(),
            listing.is_some_and(|l| l.is_good_deed()).into(),
            economic
                .is_some_and(|e| e.kind == EconomicKind::Labor)
                .into(),
            listing
                .map(|l| l.owner)
                .and_then(|id| self.member(id).ok())
                .is_some_and(|m| m.role == Role::Nana)
                .into(),
            price.or_else(|| listing.map(|l| l.price)).unwrap_or(0),
            title.is_some_and(|t| t.trim().is_empty()).into(),
            title.is_some().into(),
            price.is_some().into(),
            details.is_some().into(),
            kind,
            details.is_none_or(|d| d.currency.is_empty()).into(),
            details
                .is_some_and(|d| d.currency.chars().any(char::is_control))
                .into(),
            details.map_or(0, |d| d.minor_units),
            self.listings.is_full().into(),
            recycle.is_some().into(),
            economic.map_or(0, |e| e.quantity_milli.into()),
            economic.map_or(0, |e| e.thing as i64),
            by_id.is_some().into(),
            same(by_id).into(),
            by_name.is_some().into(),
            same(by_name).into(),
            self.things.is_full().into(),
            thing_recycle.is_some().into(),
        ]);
        facts[36] = thing_index.is_some_and(|i| self.things[i].standard).into();
        facts[37] = standard.into();
        facts[38] = self
            .member(actor)
            .is_ok_and(|m| m.role == Role::Nana)
            .into();
        let out = call(26, &facts)?;
        let payer = MemberId(out[31] as u8);
        let payee = MemberId(out[32] as u8);
        if action == 3 {
            self.validate_posting(
                payer,
                payee,
                listing.expect("validated listing").price,
                false,
            )?;
        }
        let thing_index = match out[33] {
            0 => by_id,
            1 => by_name,
            _ => None,
        };
        Ok(ListingPlan {
            status: out[30],
            payer,
            payee,
            recycle,
            thing_index,
            thing_recycle,
            standard: out[34] != 0,
        })
    }
}

pub(crate) fn settle(
    mut loan: crate::loans::Loan,
    amount: Option<i64>,
    now: u64,
    balance: i64,
    blocked: bool,
    borrower_disabled: bool,
    lender_disabled: bool,
) -> Result<crate::loans::Settlement, Error> {
    use crate::loans::LoanStatus;
    let status = match loan.status {
        LoanStatus::Active => 1,
        LoanStatus::Armed => 2,
        _ => 0,
    };
    let mut frame = [0; 34];
    frame[..20].copy_from_slice(&[
        status,
        amount.is_some().into(),
        amount.unwrap_or(0),
        now.min(i64::MAX as u64) as i64,
        balance,
        blocked.into(),
        loan.principal,
        loan.interest,
        loan.remainder as i64,
        loan.accrued_at as i64,
        loan.next_due_at as i64,
        loan.principal_due,
        loan.interest_due,
        loan.attempted_balance,
        loan.terms.amount,
        loan.terms.rate_bps.into(),
        loan.terms.rate_days.into(),
        loan.terms.payment_days.into(),
        loan.terms.installment,
        borrower_disabled.into(),
    ]);
    frame[33] = lender_disabled.into();
    let out = call(6, &frame)?;
    loan.status = if out[20] == 3 {
        LoanStatus::Paid
    } else {
        LoanStatus::Active
    };
    loan.principal = out[21];
    loan.interest = out[22];
    loan.remainder = out[23] as u64;
    loan.accrued_at = out[24] as u64;
    loan.next_due_at = out[25] as u64;
    loan.principal_due = out[26];
    loan.interest_due = out[27];
    loan.attempted_balance = out[28];
    loan.updated_at = out[29] as u64;
    Ok(crate::loans::Settlement {
        loan,
        principal_paid: out[30],
        interest_paid: out[31],
        draw: out[32] == 1,
    })
}

pub(crate) fn loan_validation(
    result: &crate::loans::Settlement,
) -> Result<Option<(MemberId, MemberId, i64)>, Error> {
    let out = call(
        8,
        &[
            result.draw.into(),
            result.loan.terms.amount,
            result.principal_paid,
            result.interest_paid,
            result.loan.lender.0.into(),
            result.loan.terms.borrower.0.into(),
        ],
    )?;
    Ok((out[30] != 0).then_some((MemberId(out[28] as u8), MemberId(out[29] as u8), out[27])))
}

pub(crate) fn accrue(loan: &mut crate::loans::Loan, now: u64) -> Result<(), Error> {
    let out = call(
        5,
        &[
            loan.principal,
            loan.terms.rate_bps.into(),
            loan.terms.rate_days.into(),
            i64::try_from(now).map_err(|_| Error::Overflow)?,
            i64::try_from(loan.accrued_at).map_err(|_| Error::Overflow)?,
            loan.interest,
            i64::try_from(loan.remainder).map_err(|_| Error::Overflow)?,
        ],
    )?;
    loan.interest = out[7];
    loan.remainder = out[8] as u64;
    loan.accrued_at = out[9] as u64;
    Ok(())
}

pub(crate) fn loan_denominator(days: u16) -> u64 {
    call(5, &[0, 0, days.into(), 0, 0, 0, 0]).expect("validated loan interest denominator")[10]
        as u64
}

pub fn initialize() -> Result<(), &'static str> {
    kernel()?;
    // Initialize libcob on the shallow startup stack, before journal replay
    // and invariant checks add their banking frames. Rescaling zero is a
    // stateless query and also warms the generated module.
    call(2, &[0])
        .map(|_| ())
        .map_err(|_| "COBOL runtime initialization failed")
}

// Transient publication plan: never serialized and never independently durable.
// Every event has at most two cash legs (FX or principal plus interest).
#[derive(Debug)]
pub(crate) struct IdentityPlan {
    pub id: MemberId,
    pub role: crate::domain::Role,
    pub kind: crate::domain::MemberKind,
    pub disabled: bool,
    pub revoke_keys: bool,
    pub key_created: u64,
    pub full_key: bool,
    pub default_household: bool,
    pub settlement: u64,
    pub grant: bool,
}

impl State {
    pub(crate) fn listing_view_indices(
        &self,
        wanted: Option<&str>,
    ) -> Result<heapless::Vec<usize, { crate::domain::LISTINGS }>, Error> {
        let wanted = match wanted {
            None => -1,
            Some("ACTIVE") => 0,
            Some("SOLD") => 1,
            Some("CANCELLED") => 2,
            Some(_) => 3,
        };
        // Query validation also runs for an empty book.
        policy(62, &[0, wanted, 0])?;
        let mut eligible = [false; crate::domain::LISTINGS];
        for (index, l) in self.listings.iter().enumerate() {
            let status = listing_status_tag(l.status);
            eligible[index] = call(62, &[0, wanted, status])?[16] != 0;
        }
        let mut indices = heapless::Vec::new();
        loop {
            let mut best = [-1, 0, 0];
            for (chunk, rows) in self.listings.chunks(20).enumerate() {
                let mut frame = [0; 64];
                frame[0] = 1;
                frame[1] = rows.len() as i64;
                for (local, row) in rows.iter().enumerate() {
                    frame[2 + local * 3] = ledger_number(row.created_at);
                    frame[3 + local * 3] = ledger_number(row.id);
                    frame[4 + local * 3] = eligible[chunk * 20 + local].into();
                }
                let candidate = call(62, &frame)?;
                let candidate_index = if candidate[61] < 0 {
                    -1
                } else {
                    (chunk * 20) as i64 + candidate[61]
                };
                let merged = call(
                    62,
                    &[
                        2,
                        best[0],
                        best[1],
                        best[2],
                        candidate_index,
                        candidate[62],
                        candidate[63],
                    ],
                )?;
                best.copy_from_slice(&merged[16..19]);
            }
            if best[0] < 0 {
                break;
            }
            let index = usize::try_from(best[0]).map_err(|_| Error::Unavailable)?;
            if !eligible.get(index).copied().unwrap_or(false) {
                return Err(Error::Unavailable);
            }
            eligible[index] = false;
            indices.push(index).map_err(|_| Error::Unavailable)?;
        }
        Ok(indices)
    }

    pub(crate) fn bank_commerce_invariants(&self) -> Result<(), Error> {
        let n = ledger_number;
        policy(
            61,
            &[
                13,
                self.commerce.requests.len() as i64,
                crate::commerce::REQUESTS as i64,
            ],
        )?;
        policy(
            61,
            &[
                13,
                self.commerce.artworks.len() as i64,
                crate::commerce::ARTWORKS as i64,
            ],
        )?;
        for (index, r) in self.commerce.requests.iter().enumerate() {
            policy(
                61,
                &[
                    14,
                    self.member(r.owner).is_ok().into(),
                    n(r.id),
                    n(self.sequence),
                    r.received,
                    r.target.is_some().into(),
                    r.target.unwrap_or(0),
                    r.title.trim().is_empty().into(),
                    self.commerce.requests[..index]
                        .iter()
                        .any(|p| p.id == r.id)
                        .into(),
                ],
            )?;
        }
        for (index, a) in self.commerce.artworks.iter().enumerate() {
            policy(
                61,
                &[
                    15,
                    self.member(a.owner).is_ok().into(),
                    self.member(a.creator).is_ok().into(),
                    n(a.id),
                    n(self.sequence),
                    n(a.revision),
                    a.price.is_some().into(),
                    a.price.unwrap_or(0),
                    art_metadata(&a.title, &a.license, &a.sha256, &a.locator)
                        .is_ok()
                        .into(),
                    self.commerce.artworks[..index]
                        .iter()
                        .any(|p| p.id == a.id)
                        .into(),
                    a.equipped.into(),
                    self.commerce.artworks[..index]
                        .iter()
                        .filter(|p| p.owner == a.owner && p.equipped)
                        .count() as i64,
                ],
            )?;
        }
        Ok(())
    }

    pub(crate) fn bank_fulfillment_invariants(&self) -> Result<(), Error> {
        use crate::fulfillment::Status;
        fn tag(status: Status) -> i64 {
            match status {
                Status::Todo => 0,
                Status::Done => 1,
                Status::Disputed => 2,
                Status::Reversed => 3,
            }
        }
        let n = ledger_number;
        policy(
            61,
            &[
                13,
                self.fulfillments.len() as i64,
                crate::fulfillment::CAPACITY as i64,
            ],
        )?;
        for (index, f) in self.fulfillments.iter().enumerate() {
            policy(
                61,
                &[
                    16,
                    n(f.payment.id),
                    n(f.transaction),
                    f.payment.to.0.into(),
                    f.provider.0.into(),
                    f.payment.from.0.into(),
                    f.recipient.0.into(),
                    f.payment.amount,
                    f.payment.usd.into(),
                    f.payment.reverses.is_some().into(),
                    self.ledger.reversed_by(f.transaction).is_some().into(),
                    (f.status == Status::Reversed).into(),
                    n(self.sequence),
                    self.member(f.provider).is_ok().into(),
                    self.member(f.recipient).is_ok().into(),
                    self.fulfillments[..index]
                        .iter()
                        .any(|p| p.transaction == f.transaction)
                        .into(),
                    f.updates.last().is_some().into(),
                    f.updates.last().map_or(-1, |u| tag(u.status)),
                    tag(f.status),
                ],
            )?;
            for u in &f.updates {
                policy(
                    61,
                    &[
                        17,
                        n(u.sequence),
                        n(self.sequence),
                        self.member(u.actor).is_ok().into(),
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn bank_base_invariants(&self) -> Result<(), Error> {
        let n = ledger_number;
        policy(
            61,
            &[
                0,
                self.decimals.into(),
                n(self.money_epoch),
                self.ledger.epochs.len() as i64,
                self.ledger.corrections.len() as i64,
                crate::ledger::CORRECTIONS as i64,
            ],
        )?;
        for (index, l) in self.loans.iter().enumerate() {
            policy(
                61,
                &[
                    1,
                    n(l.id),
                    n(self.sequence),
                    self.loans[..index].iter().any(|p| p.id == l.id).into(),
                    l.lender.0.into(),
                    l.terms.borrower.0.into(),
                    loan_tag(l.status),
                    self.member(l.lender).is_ok().into(),
                    self.member(l.terms.borrower).is_ok().into(),
                    l.terms.amount,
                    l.terms.installment,
                    l.terms.rate_days.into(),
                    l.terms.payment_days.into(),
                    l.principal,
                    l.interest,
                    l.principal_due,
                    l.interest_due,
                    n(l.remainder),
                    n(l.accrued_at),
                    n(l.updated_at),
                    n(self.last_timestamp),
                    n(l.next_due_at),
                ],
            )?;
        }
        for q in &self.quotes {
            policy(
                61,
                &[
                    2,
                    q.nc_scale,
                    q.coins,
                    q.cents_per_coin,
                    self.decimals.into(),
                ],
            )?;
        }
        let mut sums = call(
            61,
            &[
                3,
                self.issuance_balance,
                self.lotto_escrow,
                self.usd_issuance_balance,
            ],
        )?;
        for m in &self.members {
            sums = call(61, &[4, sums[32], sums[33], m.balance, m.usd_cents])?;
        }
        policy(61, &[5, sums[32], sums[33]])
    }

    pub(crate) fn bank_ledger_invariants(&self) -> Result<(), Error> {
        use crate::ledger::{AUDIT_CACHE, CORRECTIONS};
        const BASE: i128 = 1_000_000_000_000_000;
        let n = ledger_number;
        policy(
            61,
            &[
                6,
                n(self.transactions),
                n(self.sequence),
                n(self.money_epoch),
                self.ledger.epochs.len() as i64,
                self.ledger.corrections.len() as i64,
                CORRECTIONS as i64,
                self.ledger.audit.len() as i64,
                AUDIT_CACHE as i64,
                self.history.len() as i64,
                crate::domain::HISTORY as i64,
                n(self.archive.transactions),
                n(self.archive.first_transaction),
                n(self.archive.audit_sequence),
                n(self.archive.first_page),
            ],
        )?;
        for (index, e) in self.ledger.epochs.iter().enumerate() {
            policy(
                61,
                &[
                    7,
                    index as i64,
                    e.decimals.into(),
                    e.exponent.into(),
                    self.ledger
                        .epochs
                        .get(index.wrapping_sub(1))
                        .map_or(0, |p| p.decimals.into()),
                ],
            )?;
            for (flow, value) in e.flows.iter().enumerate() {
                let high = i64::try_from(*value / BASE);
                policy(
                    61,
                    &[
                        19,
                        high.is_ok().into(),
                        high.unwrap_or(0),
                        (*value % BASE) as i64,
                        n(self.transactions),
                        flow as i64,
                    ],
                )?;
            }
        }
        policy(
            61,
            &[
                20,
                self.ledger.epochs.last().is_some().into(),
                self.ledger.epochs.last().map_or(0, |e| e.decimals.into()),
                self.decimals.into(),
            ],
        )?;
        for (index, c) in self.ledger.corrections.iter().enumerate() {
            let original = self.original_payment(c.original).ok();
            policy(
                61,
                &[
                    8,
                    n(c.original),
                    c.original_amount,
                    c.refunded,
                    c.full.is_some().into(),
                    c.full.map_or(0, n),
                    self.ledger.corrections[..index]
                        .iter()
                        .any(|p| p.original == c.original)
                        .into(),
                    original.is_some().into(),
                    original.is_some_and(|t| t.reverses.is_some()).into(),
                    original.map_or(0, |t| t.amount),
                ],
            )?;
        }
        let mut ordinal = None;
        for t in &self.history {
            policy(
                61,
                &[
                    9,
                    n(t.meta.ordinal),
                    n(self.transactions),
                    n(self.archive.first_transaction),
                    ordinal.is_some().into(),
                    ordinal.map_or(0, n),
                    n(t.meta.epoch),
                    n(self.money_epoch),
                    n(t.meta.group),
                    n(self.sequence),
                    t.meta.decimals.into(),
                    t.usd.into(),
                    self.ledger
                        .epochs
                        .get(t.meta.epoch as usize)
                        .map_or(-1, |e| e.decimals.into()),
                    t.meta.refund_units,
                    t.meta.original_amount,
                    t.reverses.is_some().into(),
                ],
            )?;
            ordinal = Some(t.meta.ordinal);
        }
        policy(
            61,
            &[
                10,
                ordinal.is_some().into(),
                ordinal.map_or(0, n),
                n(self.transactions),
                n(self.archive.first_page),
            ],
        )?;
        let mut sequence = None;
        for a in &self.ledger.audit {
            a.validate_public()?;
            policy(
                61,
                &[
                    11,
                    n(a.sequence),
                    n(self.sequence),
                    n(a.at),
                    n(self.last_timestamp),
                    sequence.is_some().into(),
                    sequence.map_or(0, n),
                ],
            )?;
            sequence = Some(a.sequence);
        }
        policy(61, &[12, n(self.sequence), sequence.map_or(0, n)])
    }

    fn audit_plan(
        &self,
        event: &Event,
        identity: Option<&IdentityPlan>,
    ) -> Result<(crate::ledger::Audit, bool), Error> {
        use crate::domain::Role;
        use crate::ledger::{Audit, AuditAction, AUDIT_CACHE};
        let mut f = [0; 12];
        f[1] = 8;
        f[3] = identity.map_or(0, |p| p.id.0.into());
        f[10] = self.ledger.audit.len() as i64;
        f[11] = AUDIT_CACHE as i64;
        match &event.command {
            Command::Provision { .. } => f[1] = 0,
            Command::CreateMember { role, .. } => {
                f[1] = 1;
                f[4] = (*role == Role::Nana).into();
            }
            Command::CreateBot { .. } => f[1] = 2,
            Command::AddMember { .. } => f[1] = 3,
            Command::UpdateMember {
                member,
                display_name,
                password,
                role,
                disabled,
                ..
            } => {
                f[1] = 4;
                f[2] = member.0.into();
                f[4] = (*role == Some(Role::Nana)).into();
                f[5] = role.is_some().into();
                f[6] = disabled.is_some().into();
                f[7] = (*disabled == Some(true)).into();
                f[8] = password.is_some().into();
                f[9] = display_name.is_some().into();
            }
            Command::MigrateMember { member, .. } => {
                f[1] = 5;
                f[2] = member.0.into();
            }
            Command::SetApiKey { member, .. } => {
                f[1] = 6;
                f[2] = member.0.into();
            }
            Command::SetReadKey { member, .. } => {
                f[1] = 7;
                f[2] = member.0.into();
            }
            _ => {}
        }
        let out = call(57, &f)?;
        let action = if out[16] == 0 {
            AuditAction::Business(event.command.clone())
        } else {
            let name = match (out[18], &event.command) {
                (0, _) => crate::domain::Name::new(),
                (
                    1,
                    Command::Provision { display_name, .. }
                    | Command::CreateMember { display_name, .. }
                    | Command::CreateBot { display_name, .. },
                ) => display_name.clone(),
                (2, Command::MigrateMember { username, .. }) => username.clone(),
                (3, Command::AddMember { name, .. }) => name.clone(),
                (4, Command::UpdateMember { display_name, .. }) => display_name
                    .as_ref()
                    .expect("prepared audit display")
                    .clone(),
                _ => return Err(Error::Unavailable),
            };
            AuditAction::Identity {
                member: MemberId(out[17] as u8),
                name,
                role: (out[19] != 0).then_some(if out[20] == 1 { Role::Nana } else { Role::User }),
                disabled: (out[21] != 0).then_some(out[22] != 0),
                credentials_changed: out[23] != 0,
                created: out[24] != 0,
            }
        };
        Ok((
            Audit {
                sequence: event.sequence,
                actor: event.actor,
                at: event.timestamp,
                action,
            },
            out[25] != 0,
        ))
    }

    pub(crate) fn identity_policy(&self, actor: MemberId, command: &Command) -> Result<(), Error> {
        use crate::domain::Role;
        match command {
            Command::UpdateMember {
                member,
                display_name,
                password,
                role,
                disabled,
                mastodon_id,
                bio,
            } => {
                let target = self.members.iter().find(|m| m.id == *member);
                let text = mastodon_id.as_ref().map_or("", |m| m.as_str());
                let empty = text.is_empty();
                let text = text.strip_prefix('@').unwrap_or(text);
                let mut parts = text.split('@');
                let user = parts.next().unwrap_or_default();
                let host = parts.next().unwrap_or_default();
                policy(
                    17,
                    &[
                        target.is_some().into(),
                        bio.as_ref()
                            .is_some_and(|b| b.chars().any(char::is_control))
                            .into(),
                        target.is_some_and(|m| m.password.is_some()).into(),
                        password.is_some().into(),
                        (actor == *member).into(),
                        role.is_some().into(),
                        disabled.is_some().into(),
                        self.admin(actor).is_ok().into(),
                        display_name
                            .as_ref()
                            .is_some_and(|n| n.trim().is_empty())
                            .into(),
                        display_name
                            .as_ref()
                            .is_some_and(|n| n.chars().any(char::is_control))
                            .into(),
                        empty.into(),
                        user.is_empty().into(),
                        host.is_empty().into(),
                        parts.next().is_some().into(),
                        host.contains('.').into(),
                        text.chars()
                            .any(|c| c.is_control() || c.is_whitespace() || c == '/')
                            .into(),
                        password.as_ref().is_none_or(|p| p.valid()).into(),
                        target.is_some_and(|m| m.role == Role::Nana).into(),
                        target.is_some_and(|m| m.disabled).into(),
                        (*role == Some(Role::User) || *disabled == Some(true)).into(),
                        self.active_nana_count()?,
                    ],
                )
            }
            Command::MigrateMember {
                member,
                username,
                password,
            } => {
                let target = self.members.iter().find(|m| m.id == *member);
                policy(
                    23,
                    &[
                        0,
                        self.admin(actor).is_ok().into(),
                        (actor == *member).into(),
                        target.is_some().into(),
                        target.is_some_and(|m| m.password.is_some()).into(),
                        username.trim().is_empty().into(),
                        username.chars().any(char::is_control).into(),
                        password.valid().into(),
                        self.members.len() as i64,
                        self.members.capacity() as i64,
                        self.members.iter().any(|m| m.username == *username).into(),
                        0,
                    ],
                )
            }
            Command::AddMember { name, token_hash } => policy(
                23,
                &[
                    1,
                    self.admin(actor).is_ok().into(),
                    0,
                    0,
                    0,
                    name.trim().is_empty().into(),
                    name.chars().any(char::is_control).into(),
                    0,
                    self.members.len() as i64,
                    self.members.capacity() as i64,
                    self.members
                        .iter()
                        .any(|m| m.name == *name || m.token_hash == *token_hash)
                        .into(),
                    (*token_hash == [0; 32]).into(),
                ],
            ),
            _ => Err(Error::InvalidInput),
        }
    }

    fn active_nana_count(&self) -> Result<i64, Error> {
        let mut count = 0;
        for rows in self.members.chunks(30) {
            let mut frame = [0; 64];
            frame[0] = 6;
            frame[1] = count;
            frame[2] = rows.len() as i64;
            for (index, m) in rows.iter().enumerate() {
                frame[3 + index * 2] = (m.role == crate::domain::Role::Nana).into();
                frame[4 + index * 2] = m.disabled.into();
            }
            count = identity_query(&frame)?[16];
        }
        Ok(count)
    }

    fn identity_plan(&self, event: &Event) -> Result<Option<IdentityPlan>, Error> {
        use crate::domain::{MemberKind, Role};
        use crate::offers::DEFAULT_SETTLEMENT;
        let mut f = [0; 16];
        f[1] = self.members.len() as i64;
        f[10] = event.timestamp as i64;
        f[11] = self.household_name.is_empty().into();
        f[12] = self.offer_settles_after as i64;
        f[13] = -1;
        f[14] = DEFAULT_SETTLEMENT as i64;
        match &event.command {
            Command::Provision { .. } => f[0] = 0,
            Command::CreateMember { role, grant, .. } => {
                f[0] = 1;
                f[2] = (*role == Role::Nana).into();
                f[15] = *grant;
            }
            Command::CreateBot { grant, .. } => {
                f[0] = 2;
                f[15] = *grant;
            }
            Command::AddMember { .. } => f[0] = 3,
            Command::UpdateMember {
                member,
                password,
                role,
                disabled,
                ..
            } => {
                f[0] = 4;
                let target = self.member(*member)?;
                f[2] = (*role == Some(Role::Nana)).into();
                f[3] = (target.role == Role::Nana).into();
                f[4] = target.disabled.into();
                f[5] = role.is_some().into();
                f[6] = disabled.is_some().into();
                f[7] = (*disabled == Some(true)).into();
                f[8] = password.is_some().into();
            }
            Command::SetApiKey { key_hash, .. } => {
                f[0] = 5;
                f[9] = (*key_hash == [0; 32]).into();
            }
            Command::SetReadKey { key_hash, .. } => {
                f[0] = 6;
                f[9] = (*key_hash == [0; 32]).into();
            }
            Command::MigrateMember { .. } => f[0] = 7,
            Command::Configure {
                offer_settles_after,
                ..
            } => {
                f[0] = 8;
                f[13] = offer_settles_after.map_or(-1, |s| s as i64);
            }
            _ => return Ok(None),
        }
        let out = call(29, &f)?;
        Ok(Some(IdentityPlan {
            id: MemberId(out[32] as u8),
            role: if out[33] == 1 { Role::Nana } else { Role::User },
            kind: if out[34] == 1 {
                MemberKind::Bot
            } else {
                MemberKind::Human
            },
            disabled: out[35] != 0,
            revoke_keys: out[36] != 0,
            key_created: out[37] as u64,
            full_key: out[38] != 0,
            default_household: out[39] != 0,
            settlement: out[40] as u64,
            grant: out[41] != 0,
        }))
    }
}

#[derive(Debug, Default)]
pub(crate) struct Prepared {
    loan_reference: bool,
    pub audit: Option<(crate::ledger::Audit, bool)>,
    pub identity: Option<IdentityPlan>,
    legs: heapless::Vec<Leg, 2>,
    loan_postings: heapless::Vec<LoanPosting, 2>,
    consumed: usize,
    pub loan: Option<crate::loans::Settlement>,
    pub loan_policy: Option<LoanPlan>,
    pub lotto: Option<LottoPlan>,
    pub reform: Option<ReformPlan>,
    pub correction_policy: Option<CorrectionPolicy>,
    pub conversions: heapless::Vec<i64, 1024>,
    pub fulfillment: Option<crate::fulfillment::Status>,
    pub correction: Option<crate::ledger::Correction>,
    pub offer: Option<OfferPlan>,
    pub listing: Option<ListingPlan>,
    pub quote: Option<QuotePlan>,
    pub gift: Option<GiftPlan>,
    pub art: Option<ArtPlan>,
    pub gift_refund: Option<(u64, i64)>,
    pub reversed_offers: u32,
    pub new_fulfillment: Option<(
        crate::fulfillment::Kind,
        crate::fulfillment::Status,
        Option<usize>,
    )>,
    pub reversed_fulfillment: Option<(usize, crate::fulfillment::Status)>,
}

#[derive(Debug)]
struct Leg {
    from: MemberId,
    to: MemberId,
    amount: i64,
    usd: bool,
    balances: PostingPlan,
    credit_blocked: u32,
    flows: [i128; crate::ledger::FLOW_COUNT],
}

#[derive(Debug)]
pub(crate) struct LoanPosting {
    pub from: MemberId,
    pub to: MemberId,
    pub amount: i64,
    pub kind: crate::domain::EconomicKind,
    pub offset: u64,
}

impl State {
    fn prepare_loan_postings(&self, plan: &mut Prepared, values: &[i64; 6]) -> Result<(), Error> {
        let out = call(8, values)?;
        if !(0..=2).contains(&out[16]) {
            return Err(Error::Unavailable);
        }
        for index in 0..out[16] as usize {
            let base = 17 + index * 5;
            let kind = match out[base + 3] {
                3 => crate::domain::EconomicKind::LoanPrincipal,
                4 => crate::domain::EconomicKind::Interest,
                _ => return Err(Error::Unavailable),
            };
            let (from, to, amount) = (
                MemberId(out[base] as u8),
                MemberId(out[base + 1] as u8),
                out[base + 2],
            );
            self.prepare_leg(plan, from, to, amount, false)?;
            plan.loan_postings
                .push(LoanPosting {
                    from,
                    to,
                    amount,
                    kind,
                    offset: out[base + 4] as u64,
                })
                .map_err(|_| Error::Unavailable)?;
        }
        Ok(())
    }

    pub(crate) fn take_loan_postings(&mut self) -> heapless::Vec<LoanPosting, 2> {
        std::mem::take(&mut self.prepared.loan_postings)
    }

    fn fulfillment_recycle_index(&self) -> Result<Option<usize>, Error> {
        use crate::fulfillment::Status;
        for (chunk, rows) in self.fulfillments.chunks(31).enumerate() {
            let mut frame = [0; 64];
            frame[0] = rows.len() as i64;
            for (index, f) in rows.iter().enumerate() {
                frame[1 + index * 2] = match f.status {
                    Status::Todo => 0,
                    Status::Done => 1,
                    Status::Disputed => 2,
                    Status::Reversed => 3,
                };
                frame[2 + index * 2] = self.history.iter().any(|t| t.id == f.transaction).into();
            }
            let index = call(47, &frame)?[63];
            if index >= 0 {
                return Ok(Some(chunk * 31 + index as usize));
            }
        }
        Ok(None)
    }

    pub(crate) fn validate_fulfillment_capacity(&self, c: &Command) -> Result<(), Error> {
        let (category, amount, kind) = match c {
            Command::Buy { .. } | Command::AcceptOffer { .. } => (1, 0, 0),
            Command::ClassifiedTransfer {
                amount, economic, ..
            } => (2, *amount, physical_kind(economic.kind)),
            _ => (0, 0, 0),
        };
        let full = self.fulfillments.len() == crate::fulfillment::CAPACITY;
        let recyclable = if full {
            self.fulfillment_recycle_index()?.is_some()
        } else {
            false
        };
        call(
            45,
            &[category, amount, kind, full.into(), recyclable.into()],
        )
        .map(|_| ())
    }

    fn bank_balance(&self, id: MemberId, usd: bool) -> i64 {
        if id == MemberId(0) {
            if usd {
                self.usd_issuance_balance
            } else {
                self.issuance_balance
            }
        } else if id == crate::lotto::ESCROW {
            self.lotto_escrow
        } else {
            // A member creation's grant targets the member about to be inserted.
            self.member(id)
                .map_or(0, |m| if usd { m.usd_cents } else { m.balance })
        }
    }

    fn prepare_leg(
        &self,
        plan: &mut Prepared,
        from: MemberId,
        to: MemberId,
        amount: i64,
        usd: bool,
    ) -> Result<(), Error> {
        let balance = |id| {
            let mut value = self.bank_balance(id, usd);
            for leg in &plan.legs {
                if leg.usd == usd {
                    if leg.from == id {
                        value = leg.balances.debit;
                    }
                    if leg.to == id {
                        value = leg.balances.credit;
                    }
                }
            }
            value
        };
        // Business permission/funds checks already ran. The publication plan
        // chooses empty or same-wallet legs as well as the resulting balances.
        let out = call(
            1,
            &[
                balance(from),
                balance(to),
                amount,
                (from == MemberId(0)).into(),
                1,
                usd.into(),
                0,
                0,
                1,
                (from == to).into(),
                from.0.into(),
                to.0.into(),
                plan.loan_reference.into(),
                plan.legs
                    .last()
                    .map_or(self.credit_blocked, |leg| leg.credit_blocked)
                    .into(),
            ],
        )?;
        if out[16] == 0 {
            return Ok(());
        }
        let balances = PostingPlan {
            debit: out[6],
            credit: out[7],
        };
        plan.legs
            .push(Leg {
                from,
                to,
                amount,
                usd,
                balances,
                credit_blocked: out[17] as u32,
                flows: [0; crate::ledger::FLOW_COUNT],
            })
            .map_err(|_| Error::Capacity)
    }

    pub(crate) fn prepare_bank(&mut self, event: &Event) -> Result<(), Error> {
        let identity = self.identity_plan(event)?;
        let audit = self.audit_plan(event, identity.as_ref())?;
        let loan_reference = match &event.command {
            Command::AcceptLoan { .. } | Command::RepayLoan { .. } | Command::RunLoan { .. } => {
                true
            }
            Command::Reverse { transaction, .. } | Command::Refund { transaction, .. } => {
                self.original_payment(*transaction)?.loan.is_some()
            }
            _ => false,
        };
        let mut plan = Prepared {
            loan_reference,
            identity,
            audit: Some(audit),
            ..Prepared::default()
        };
        let actor = event.actor;
        match &event.command {
            Command::Reverse { transaction, .. } => {
                plan.correction_policy = Some(self.correction_policy(actor, *transaction, None)?)
            }
            Command::Refund {
                transaction,
                amount,
                ..
            } => {
                plan.correction_policy =
                    Some(self.correction_policy(actor, *transaction, Some(*amount))?)
            }
            _ => {}
        }
        if matches!(
            event.command,
            Command::CreateLotto { .. } | Command::BuyTickets { .. } | Command::RunLotto { .. }
        ) {
            plan.lotto = Some(self.lotto_plan(actor, &event.command, event.timestamp)?);
        }
        if matches!(
            event.command,
            Command::OfferLoan { .. }
                | Command::RequestLoan { .. }
                | Command::RespondLoan { .. }
                | Command::AcceptLoan { .. }
                | Command::CloseLoan { .. }
                | Command::RepayLoan { .. }
                | Command::RunLoan { .. }
        ) {
            plan.loan_policy = Some(self.loan_plan(actor, &event.command, event.timestamp)?);
        }
        if let Command::Commerce { action } = &event.command {
            if matches!(
                action,
                crate::commerce::Action::MintArt { .. }
                    | crate::commerce::Action::ListArt { .. }
                    | crate::commerce::Action::BuyArt { .. }
                    | crate::commerce::Action::GiftArt { .. }
                    | crate::commerce::Action::EquipArt { .. }
            ) {
                plan.art = Some(self.art_plan(actor, action, event.sequence)?);
            }
            if matches!(
                action,
                crate::commerce::Action::CreateRequest { .. }
                    | crate::commerce::Action::CloseRequest { .. }
                    | crate::commerce::Action::Contribute { .. }
            ) {
                plan.gift = Some(self.gift_plan(actor, action, event.timestamp)?);
            }
        }
        if matches!(
            event.command,
            Command::PostQuote { .. } | Command::TakeQuote { .. } | Command::CancelQuote { .. }
        ) {
            plan.quote = Some(self.quote_plan(actor, &event.command, event.timestamp)?);
        }
        if matches!(
            event.command,
            Command::MakeOffer { .. }
                | Command::AcceptOffer { .. }
                | Command::UnacceptOffer { .. }
                | Command::DeclineOffer { .. }
                | Command::WithdrawOffer { .. }
        ) {
            plan.offer = Some(self.offer_plan(actor, &event.command, event.timestamp)?);
        }
        if matches!(
            event.command,
            Command::List { .. }
                | Command::ClassifiedList { .. }
                | Command::Cancel { .. }
                | Command::UpdateListing { .. }
                | Command::Buy { .. }
        ) {
            plan.listing = Some(self.listing_plan(actor, &event.command, event.timestamp)?);
        }
        match &event.command {
            Command::SetFulfillment {
                transaction,
                action,
                reason,
            } => {
                let f = self
                    .fulfillments
                    .iter()
                    .find(|f| f.transaction == *transaction)
                    .ok_or(Error::NotFound)?;
                plan.fulfillment = Some(fulfillment(f, actor, *action, reason)?);
            }
            Command::ReformCurrency {
                decimals, power, ..
            } => self.prepare_reform_numbers(&mut plan, *decimals, *power)?,
            Command::Issue { to, amount, .. } => {
                self.prepare_leg(&mut plan, MemberId(0), *to, *amount, false)?
            }
            Command::Retire { from, amount, .. } => {
                self.prepare_leg(&mut plan, *from, MemberId(0), *amount, false)?
            }
            Command::Transfer { to, amount, .. }
            | Command::ClassifiedTransfer { to, amount, .. } => {
                self.prepare_leg(&mut plan, actor, *to, *amount, false)?
            }
            Command::CreateMember { grant, .. } | Command::CreateBot { grant, .. } => {
                let id = plan.identity.as_ref().expect("prepared member identity").id;
                self.prepare_leg(&mut plan, MemberId(0), id, *grant, false)?;
            }
            Command::Buy { listing } => {
                let l = self.listing(*listing)?;
                let p = plan.listing.as_ref().expect("prepared listing");
                let (from, to) = (p.payer, p.payee);
                self.prepare_leg(&mut plan, from, to, l.price, false)?;
            }
            Command::Reverse { transaction, .. } | Command::Refund { transaction, .. } => {
                let tx = self.original_payment(*transaction)?;
                let p = plan.correction_policy.expect("prepared correction policy");
                let units = p.units;
                self.prepare_leg(
                    &mut plan,
                    p.payer,
                    p.payee,
                    self.current_amount(tx, units)?,
                    tx.usd,
                )?;
            }
            Command::IssueUsd { to, cents, .. } => {
                self.prepare_leg(&mut plan, MemberId(0), *to, *cents, true)?
            }
            Command::TakeQuote { quote } => {
                let q = self.quote(*quote)?;
                let p = plan.quote.as_ref().expect("prepared quote");
                let (seller, buyer, cents) = (p.seller, p.buyer, p.cents);
                self.prepare_leg(&mut plan, seller, buyer, q.coins, false)?;
                self.prepare_leg(&mut plan, buyer, seller, cents, true)?;
            }
            Command::AcceptOffer { offer } => {
                let o = self.offer(*offer)?;
                let p = plan.offer.as_ref().expect("prepared offer");
                let (from, to) = (p.payer, p.payee);
                self.prepare_leg(&mut plan, from, to, o.amount, false)?;
            }
            Command::UnacceptOffer { offer, .. } => {
                let o = self.offer(*offer)?;
                let s = o.settlement().ok_or(Error::Conflict)?;
                self.prepare_leg(&mut plan, s.payee, s.payer, o.amount, false)?;
            }
            Command::Commerce { action } => match action {
                crate::commerce::Action::Contribute { amount, .. } => {
                    let to = plan.gift.as_ref().expect("prepared gift").owner;
                    self.prepare_leg(&mut plan, actor, to, *amount, false)?;
                }
                crate::commerce::Action::BuyArt { .. } => {
                    let p = plan.art.as_ref().expect("prepared art sale");
                    let (seller, amount) = (p.seller, p.amount);
                    self.prepare_leg(&mut plan, actor, seller, amount, false)?;
                }
                _ => {}
            },
            Command::AcceptLoan { .. } => {
                let p = plan.loan_policy.as_ref().expect("prepared loan acceptance");
                let (from, to, amount) = (p.lender, p.borrower, p.draw);
                self.prepare_loan_postings(
                    &mut plan,
                    &[1, amount, 0, 0, from.0.into(), to.0.into()],
                )?;
            }
            Command::RepayLoan { loan, .. } | Command::RunLoan { loan, .. } => {
                let amount = match event.command {
                    Command::RepayLoan { amount, .. } => Some(amount),
                    _ => None,
                };
                let result = self.loan_settlement(*loan, amount, event.timestamp)?;
                let l = &result.loan;
                self.prepare_loan_postings(
                    &mut plan,
                    &[
                        result.draw.into(),
                        l.terms.amount,
                        result.principal_paid,
                        result.interest_paid,
                        l.lender.0.into(),
                        l.terms.borrower.0.into(),
                    ],
                )?;
                plan.loan = Some(result);
            }
            Command::BuyTickets { .. } | Command::RunLotto { .. } => {
                let p = plan.lotto.as_ref().expect("prepared lotto");
                let (from, to, amount) = (p.from, p.to, p.amount);
                self.prepare_leg(&mut plan, from, to, amount, false)?;
            }
            _ => {}
        }
        let correction = match &event.command {
            Command::Reverse { transaction, .. } => {
                let original = self.original_payment(*transaction)?;
                Some((*transaction, original.amount, original.amount))
            }
            Command::Refund {
                transaction,
                amount,
                ..
            } => Some((
                *transaction,
                self.original_payment(*transaction)?.amount,
                *amount,
            )),
            Command::UnacceptOffer { offer, .. } => {
                let s = self.offer(*offer)?.settlement().ok_or(Error::Conflict)?;
                Some((s.transaction, s.original_amount, s.original_amount))
            }
            _ => None,
        };
        if let Some((original, original_amount, units)) = correction {
            if let Some(id) = self.original_payment(original)?.meta.gift_request {
                let amount = plan.legs.first().map_or(0, |leg| leg.amount);
                let out = call(56, &[self.gift_request(id)?.received, amount])?;
                plan.gift_refund = Some((id, out[2]));
            }
            let out = call(
                40,
                &[
                    original_amount,
                    self.ledger.refunded(original),
                    units,
                    event.sequence as i64,
                ],
            )?;
            plan.correction = Some(crate::ledger::Correction {
                original,
                original_amount,
                refunded: out[4],
                full: (out[5] != 0).then_some(out[5] as u64),
            });
            if out[5] != 0 {
                for (chunk, offers) in self.offers.chunks(16).enumerate() {
                    let mut rows = [0; 64];
                    rows[0] = original as i64;
                    rows[1] = offers.len() as i64;
                    for (index, offer) in offers.iter().enumerate() {
                        rows[2 + index * 2] = offer_phase(offer.phase);
                        rows[3 + index * 2] =
                            offer.settlement().map_or(0, |s| s.transaction as i64);
                    }
                    plan.reversed_offers |= (call(25, &rows)?[34] as u32) << (chunk * 16);
                }
            }
        }
        let mut totals = self.ledger.epochs[self.money_epoch as usize].flows;
        for index in 0..plan.legs.len() {
            use crate::domain::EconomicKind;
            let (kind, quote, lotto, reversal) = match &event.command {
                Command::ClassifiedTransfer { economic, .. } => {
                    (economic.kind, false, false, false)
                }
                Command::Buy { listing } => {
                    (self.listing(*listing)?.economic.kind, false, false, false)
                }
                Command::AcceptOffer { offer } => (
                    self.listing(self.offer(*offer)?.listing)?.economic.kind,
                    false,
                    false,
                    false,
                ),
                Command::UnacceptOffer { offer, .. } => (
                    self.offer(*offer)?
                        .settlement()
                        .ok_or(Error::Conflict)?
                        .economic
                        .kind,
                    false,
                    false,
                    true,
                ),
                Command::Reverse { transaction, .. } | Command::Refund { transaction, .. } => {
                    let tx = self.original_payment(*transaction)?;
                    (
                        tx.economic.kind,
                        tx.quote.is_some(),
                        tx.lotto.is_some(),
                        true,
                    )
                }
                Command::TakeQuote { .. } => (EconomicKind::Other, true, false, false),
                Command::AcceptLoan { .. } => (EconomicKind::LoanPrincipal, false, false, false),
                Command::RepayLoan { .. } | Command::RunLoan { .. } => {
                    let kind = plan.loan_postings[index].kind;
                    (kind, false, false, false)
                }
                Command::RunLotto { .. } => (
                    plan.lotto.as_ref().expect("prepared lotto").kind,
                    false,
                    true,
                    false,
                ),
                Command::BuyTickets { .. } => (EconomicKind::Other, false, true, false),
                Command::Commerce {
                    action: crate::commerce::Action::BuyArt { .. },
                } => (EconomicKind::Good, false, false, false),
                Command::Commerce {
                    action: crate::commerce::Action::Contribute { .. },
                } => (EconomicKind::Gift, false, false, false),
                _ => (EconomicKind::Other, false, false, false),
            };
            let leg = &mut plan.legs[index];
            totals = flow_totals(
                totals, leg.amount, leg.from, leg.to, leg.usd, kind, quote, lotto, reversal,
            )?;
            leg.flows = totals;
        }
        let listing = match &event.command {
            Command::Buy { listing } => Some(self.listing(*listing)?),
            Command::AcceptOffer { offer } => Some(self.listing(self.offer(*offer)?.listing)?),
            _ => None,
        };
        let kind = match &event.command {
            Command::ClassifiedTransfer { economic, .. } => economic.kind,
            _ => listing.map_or(crate::domain::EconomicKind::Other, |l| l.economic.kind),
        };
        let leg = plan.legs.first();
        let out = call(
            46,
            &[
                correction.is_some().into(),
                plan.correction
                    .as_ref()
                    .is_some_and(|c| c.full.is_some())
                    .into(),
                leg.is_some_and(|l| l.usd).into(),
                leg.map_or(0, |l| l.amount),
                matches!(
                    event.command,
                    Command::AcceptLoan { .. }
                        | Command::RepayLoan { .. }
                        | Command::RunLoan { .. }
                )
                .into(),
                matches!(
                    event.command,
                    Command::BuyTickets { .. } | Command::RunLotto { .. }
                )
                .into(),
                matches!(event.command, Command::TakeQuote { .. }).into(),
                matches!(
                    event.command,
                    Command::Commerce {
                        action: crate::commerce::Action::BuyArt { .. }
                    }
                )
                .into(),
                leg.map_or(0, |l| l.from.0.into()),
                listing.is_some().into(),
                physical_kind(kind),
                listing.is_some_and(|l| l.details.kind == "currency").into(),
                listing.is_some_and(|l| l.details.kind == "service").into(),
            ],
        )?;
        match out[13] {
            1 => {
                let kind = match out[14] {
                    0 => crate::fulfillment::Kind::Work,
                    1 => crate::fulfillment::Kind::Goods,
                    2 => crate::fulfillment::Kind::Cash,
                    _ => return Err(Error::Unavailable),
                };
                let recycle = if self.fulfillments.len() == crate::fulfillment::CAPACITY {
                    Some(self.fulfillment_recycle_index()?.ok_or(Error::Capacity)?)
                } else {
                    None
                };
                plan.new_fulfillment = Some((kind, fulfillment_status(out[15])?, recycle));
            }
            2 => {
                let original = correction.expect("prepared correction").0;
                plan.reversed_fulfillment = self
                    .fulfillments
                    .iter()
                    .position(|f| f.transaction == original)
                    .map(|index| fulfillment_status(out[15]).map(|status| (index, status)))
                    .transpose()?;
            }
            _ => {}
        }
        *self.prepared = plan;
        Ok(())
    }

    pub(crate) fn prepared_cash(&self) -> bool {
        self.prepared.consumed < self.prepared.legs.len()
    }

    pub(crate) fn apply_balances(&mut self, tx: &Transaction) {
        if !self.prepared_cash() {
            assert_eq!(tx.amount, 0, "empty prepared cash plan");
            return;
        }
        let leg = &self.prepared.legs[self.prepared.consumed];
        assert_eq!(
            (leg.from, leg.to, leg.amount, leg.usd),
            (tx.from, tx.to, tx.amount, tx.usd),
            "prepared posting mismatch"
        );
        let balances = leg.balances;
        self.credit_blocked = leg.credit_blocked;
        self.prepared.consumed += 1;
        for (id, balance) in [(tx.from, balances.debit), (tx.to, balances.credit)] {
            if id == MemberId(0) {
                if tx.usd {
                    self.usd_issuance_balance = balance;
                } else {
                    self.issuance_balance = balance;
                }
            } else if id == crate::lotto::ESCROW {
                self.lotto_escrow = balance;
            } else {
                let m = self
                    .members
                    .iter_mut()
                    .find(|m| m.id == id)
                    .expect("prepared member");
                if tx.usd {
                    m.usd_cents = balance;
                } else {
                    m.balance = balance;
                }
            }
        }
    }

    pub(crate) fn prepared_amount(&self, offset: usize) -> i64 {
        self.prepared.legs[self.prepared.consumed + offset].amount
    }

    pub(crate) fn prepared_flows(&self) -> [i128; crate::ledger::FLOW_COUNT] {
        self.prepared.legs[self.prepared.consumed].flows
    }

    pub(crate) fn finish_bank_plan(&self) {
        assert!(self.prepared.audit.is_none(), "prepared audit applied");
        assert!(
            self.prepared.identity.is_none(),
            "prepared identity applied"
        );
        assert_eq!(
            self.prepared.consumed,
            self.prepared.legs.len(),
            "all prepared cash legs applied"
        );
        assert!(self.prepared.loan.is_none(), "prepared loan applied");
        assert!(
            self.prepared.loan_postings.is_empty(),
            "prepared loan postings applied"
        );
        assert!(
            self.prepared.correction_policy.is_none(),
            "prepared correction policy applied"
        );
        assert!(self.prepared.lotto.is_none(), "prepared lotto applied");
        assert!(self.prepared.reform.is_none(), "prepared reform applied");
        assert!(
            self.prepared.loan_policy.is_none(),
            "prepared loan policy applied"
        );
        assert!(self.prepared.offer.is_none(), "prepared offer applied");
        assert!(self.prepared.listing.is_none(), "prepared listing applied");
        assert!(self.prepared.quote.is_none(), "prepared quote applied");
        assert!(self.prepared.gift.is_none(), "prepared gift applied");
        assert!(self.prepared.art.is_none(), "prepared art applied");
        assert!(
            self.prepared.gift_refund.is_none(),
            "prepared gift refund applied"
        );
        assert!(
            self.prepared.new_fulfillment.is_none(),
            "prepared obligation applied"
        );
        assert!(
            self.prepared.reversed_fulfillment.is_none(),
            "prepared obligation reversal applied"
        );
        assert!(
            self.prepared.correction.is_none(),
            "prepared correction applied"
        );
    }
}

fn physical_kind(kind: crate::domain::EconomicKind) -> i64 {
    match kind {
        crate::domain::EconomicKind::Labor => 1,
        crate::domain::EconomicKind::Good => 2,
        _ => 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn flow_totals(
    totals: [i128; 20],
    amount: i64,
    from: MemberId,
    to: MemberId,
    usd: bool,
    kind: crate::domain::EconomicKind,
    quote: bool,
    lotto: bool,
    reversal: bool,
) -> Result<[i128; 20], Error> {
    use crate::domain::EconomicKind;
    let kind = match kind {
        EconomicKind::Labor => 1,
        EconomicKind::Good => 2,
        EconomicKind::LoanPrincipal => 3,
        EconomicKind::Interest => 4,
        _ => 0,
    };
    let delta = call(
        30,
        &[
            amount,
            from.0.into(),
            to.0.into(),
            usd.into(),
            quote.into(),
            lotto.into(),
            reversal.into(),
            kind,
        ],
    )?;
    let mut frame = [0; 64];
    frame[..20].copy_from_slice(&delta[8..28]);
    const BASE: i128 = 1_000_000_000_000_000;
    for (index, total) in totals.iter().enumerate() {
        frame[20 + index * 2] = i64::try_from(total / BASE).map_err(|_| Error::Overflow)?;
        frame[21 + index * 2] = (total % BASE) as i64;
    }
    let out = call(31, &frame)?;
    let mut result = [0; 20];
    for (index, total) in result.iter_mut().enumerate() {
        *total = i128::from(out[20 + index * 2]) * BASE + i128::from(out[21 + index * 2]);
    }
    Ok(result)
}

pub(crate) fn reform_fraction(
    interest: i64,
    remainder: u64,
    denominator: u64,
    exponent: i16,
    principal: i64,
) -> Result<(i64, u64), Error> {
    let out = call(
        7,
        &[
            interest,
            remainder as i64,
            denominator as i64,
            exponent.into(),
            principal,
        ],
    )?;
    Ok((out[5], out[6] as u64))
}

fn fulfillment_status(status: i64) -> Result<crate::fulfillment::Status, Error> {
    use crate::fulfillment::Status;
    match status {
        0 => Ok(Status::Todo),
        1 => Ok(Status::Done),
        2 => Ok(Status::Disputed),
        3 => Ok(Status::Reversed),
        _ => Err(Error::Unavailable),
    }
}

pub(crate) fn fulfillment(
    f: &crate::fulfillment::Fulfillment,
    actor: MemberId,
    action: crate::fulfillment::Action,
    reason: &str,
) -> Result<crate::fulfillment::Status, Error> {
    use crate::fulfillment::{Action, Status};
    let status = match f.status {
        Status::Todo => 0,
        Status::Done => 1,
        Status::Disputed => 2,
        Status::Reversed => 3,
    };
    let action = match action {
        Action::Complete => 0,
        Action::Dispute => 1,
        Action::WithdrawDispute => 2,
    };
    let out = call(
        20,
        &[
            status,
            action,
            actor.0.into(),
            f.provider.0.into(),
            f.recipient.0.into(),
            reason.trim().is_empty().into(),
            reason.chars().any(char::is_control).into(),
        ],
    )?;
    fulfillment_status(out[7])
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LottoPlan {
    pub amount: i64,
    pub pool: i64,
    pub escrow: i64,
    pub interest: i64,
    pub interest_remaining: i64,
    pub winner: Option<MemberId>,
    pub step: u8,
    pub from: MemberId,
    pub to: MemberId,
    pub kind: crate::domain::EconomicKind,
    pub tickets: u32,
    pub index: Option<usize>,
    pub recycle: Option<usize>,
}

fn lotto_kind(kind: crate::lotto::LottoKind) -> i64 {
    match kind {
        crate::lotto::LottoKind::Simple => 0,
        crate::lotto::LottoKind::Delayed => 1,
        crate::lotto::LottoKind::Savings => 2,
    }
}

pub(crate) fn lotto_projection(l: &crate::lotto::Lotto, now: u64) -> [i64; 64] {
    let mut frame = [0; 36];
    frame[..4].copy_from_slice(&[
        lotto_kind(l.terms.kind),
        l.terms.closes_at as i64,
        l.step.into(),
        i64::try_from(now).unwrap_or(i64::MAX),
    ]);
    for (out, count) in frame[4..].iter_mut().zip(l.tickets) {
        *out = count.into();
    }
    call(33, &frame).expect("bounded lotto projection")
}

impl State {
    pub(crate) fn lotto_plan(
        &self,
        actor: MemberId,
        command: &Command,
        now: u64,
    ) -> Result<LottoPlan, Error> {
        let (action, terms, row_id, input, expected) = match command {
            Command::CreateLotto { terms } => (1, Some(terms), None, 0, 0),
            Command::BuyTickets { lotto, count } => (2, None, Some(*lotto), i64::from(*count), 0),
            Command::RunLotto {
                lotto,
                ticket,
                step,
            } => (
                3,
                None,
                Some(*lotto),
                i64::try_from(*ticket).unwrap_or(i64::MAX),
                i64::from(*step),
            ),
            _ => return Err(Error::InvalidInput),
        };
        let index = row_id.and_then(|id| self.lottos.iter().position(|l| l.id == id));
        let row = index.map(|i| &self.lottos[i]);
        let terms = terms.or_else(|| row.map(|l| &l.terms));
        let mut book = [0; 17];
        book[0] = self.lottos.len() as i64;
        for (out, l) in book[1..].iter_mut().zip(&self.lottos) {
            *out = l.step.into();
        }
        let first = call(34, &book)?[17];
        let recycle = (first > 0).then(|| first as usize - 1);
        let member = self.member(actor).ok();
        let mut frame = [0; 56];
        frame[..23].copy_from_slice(&[
            action,
            i64::try_from(now).unwrap_or(i64::MAX),
            actor.0.into(),
            self.admin(actor).is_ok().into(),
            row.is_some().into(),
            terms.map_or(0, |t| lotto_kind(t.kind)),
            terms
                .is_some_and(|t| {
                    !t.title.trim().is_empty() && !t.title.chars().any(char::is_control)
                })
                .into(),
            terms.map_or(0, |t| t.ticket_price),
            terms.map_or(0, |t| i64::try_from(t.closes_at).unwrap_or(i64::MAX)),
            terms.map_or(0, |t| t.rate_bps.into()),
            row.map_or(0, |l| l.house.0.into()),
            row.map_or(0, |l| l.step.into()),
            row.map_or(0, |l| l.pool),
            row.map_or(0, |l| l.escrow),
            row.map_or(0, |l| l.interest),
            row.map_or(0, |l| l.interest_remaining),
            row.and_then(|l| l.winner).map_or(0, |id| id.0.into()),
            input,
            self.lotto_escrow,
            if action == 3 {
                row.and_then(|l| self.member(l.house).ok())
                    .map_or(0, |m| m.balance)
            } else {
                member.map_or(0, |m| m.balance)
            },
            expected,
            self.lottos.len() as i64,
            first,
        ]);
        if let Some(l) = row {
            for (out, n) in frame[24..].iter_mut().zip(l.tickets) {
                *out = n.into();
            }
        }
        let out = call(32, &frame)?;
        let parties = out[63] & 65535;
        let p = LottoPlan {
            amount: out[56],
            pool: out[57],
            escrow: out[58],
            interest: out[59],
            interest_remaining: out[60],
            winner: (out[61] != 0).then(|| MemberId(out[61] as u8)),
            step: out[62] as u8,
            from: MemberId((parties / 256) as u8),
            to: MemberId((parties % 256) as u8),
            kind: if out[63] & 65536 != 0 {
                crate::domain::EconomicKind::Interest
            } else {
                crate::domain::EconomicKind::Other
            },
            tickets: out[23] as u32,
            index,
            recycle,
        };
        if p.amount > 0 && action == 3 {
            let target = self.member(p.to)?.balance;
            call(
                1,
                &[
                    self.bank_balance(p.from, false),
                    target,
                    p.amount,
                    (p.from == MemberId(0)).into(),
                    1,
                    0,
                    0,
                    0,
                    1,
                    (p.from == p.to).into(),
                    p.from.0.into(),
                    p.to.0.into(),
                    0,
                    self.credit_blocked.into(),
                ],
            )?;
        }
        Ok(p)
    }
}

pub(crate) fn lotto_winner(l: &crate::lotto::Lotto, ticket: u64) -> Option<MemberId> {
    let mut frame = lotto_projection(l, 0);
    frame[41] = i64::try_from(ticket).unwrap_or(i64::MAX);
    let winner = call(33, &frame).expect("bounded ticket selection")[42];
    (winner != 0).then_some(MemberId(winner as u8))
}

impl State {
    pub(crate) fn lotto_invariants(&self) -> Result<(), Error> {
        let mut escrow = 0;
        let mut present = 0i64;
        for index in 0..crate::domain::MEMBERS {
            if self.member(MemberId(index as u8 + 1)).is_ok() {
                present |= 1 << index;
            }
        }
        for (index, l) in self.lottos.iter().enumerate() {
            let mut frame = [0; 48];
            frame[..16].copy_from_slice(&[
                l.id as i64,
                self.sequence as i64,
                self.lottos[..index]
                    .iter()
                    .any(|other| other.id == l.id)
                    .into(),
                self.member(l.house).is_ok().into(),
                lotto_kind(l.terms.kind),
                l.terms.ticket_price,
                l.terms.rate_bps.into(),
                l.terms.closes_at as i64,
                l.step.into(),
                l.pool,
                l.escrow,
                l.interest,
                l.interest_remaining,
                l.winner
                    .map_or(0, |w| if w.0 == 0 { -1 } else { w.0.into() }),
                present,
                escrow,
            ]);
            for (out, count) in frame[16..].iter_mut().zip(l.tickets) {
                *out = count.into();
            }
            escrow = call(35, &frame).map_err(|_| Error::CorruptJournal)?[48];
        }
        let mut final_frame = [0; 51];
        final_frame[15] = escrow;
        final_frame[49] = self.lotto_escrow;
        final_frame[50] = 1;
        call(35, &final_frame).map_err(|_| Error::CorruptJournal)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReformPlan {
    pub exponent: i16,
    pub next_epoch: u64,
    pub nc_scale: i64,
}
impl State {
    pub(crate) fn reform_policy(
        &self,
        decimals: u8,
        power: i8,
        expected_epoch: u64,
        expected_sequence: u64,
    ) -> Result<ReformPlan, Error> {
        let out = call(
            36,
            &[
                decimals.into(),
                power.into(),
                self.decimals.into(),
                self.ledger.epochs.len() as i64,
                self.money_epoch as i64,
                self.sequence as i64,
                expected_epoch as i64,
                expected_sequence as i64,
                crate::ledger::EPOCHS as i64,
            ],
        )?;
        Ok(ReformPlan {
            exponent: out[9] as i16,
            next_epoch: out[10] as u64,
            nc_scale: out[11],
        })
    }
    pub(crate) fn reform_field(
        &self,
        value: i64,
        decimals: u8,
        power: i8,
        kind: i64,
    ) -> Result<i64, Error> {
        Ok(call(
            38,
            &[
                value,
                self.decimals.into(),
                decimals.into(),
                power.into(),
                kind,
            ],
        )?[5])
    }
}
pub(crate) fn reform_lotto_consistency(pool: i64, interest: i64, rate: u32) -> Result<(), Error> {
    policy(37, &[pool, interest, rate.into()])
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CorrectionPolicy {
    pub payer: MemberId,
    pub payee: MemberId,
    pub units: i64,
    pub overdraft: bool,
}
impl State {
    pub(crate) fn account_posting(
        &self,
        from: MemberId,
        to: MemberId,
        amount: i64,
        correction: bool,
        usd: bool,
    ) -> Result<(), Error> {
        let source = self.member(from).ok();
        let target = self.member(to).ok();
        call(
            39,
            &[
                from.0.into(),
                to.0.into(),
                amount,
                correction.into(),
                usd.into(),
                source.is_some().into(),
                source.is_some_and(|m| m.disabled).into(),
                target.is_some().into(),
                target.is_some_and(|m| m.disabled).into(),
                self.bank_balance(from, usd),
                self.bank_balance(to, usd),
            ],
        )
        .map(|_| ())
    }
    pub(crate) fn correction_policy(
        &self,
        actor: MemberId,
        id: u64,
        units: Option<i64>,
    ) -> Result<CorrectionPolicy, Error> {
        let tx = self.original_payment(id).ok();
        let out = call(
            41,
            &[
                if units.is_some() { 0 } else { 1 },
                tx.is_some().into(),
                actor.0.into(),
                self.member(actor)
                    .is_ok_and(|m| m.role == crate::domain::Role::Nana && !m.disabled)
                    .into(),
                tx.map_or(0, |t| t.from.0.into()),
                tx.map_or(0, |t| t.to.0.into()),
                tx.map_or(0, |t| t.amount),
                tx.is_some_and(|t| t.reverses.is_some()).into(),
                tx.is_some_and(|t| t.usd).into(),
                tx.is_some_and(|t| t.loan.is_some()).into(),
                tx.is_some_and(|t| t.lotto.is_some()).into(),
                tx.is_some_and(|t| t.quote.is_some()).into(),
                tx.is_some_and(|t| t.meta.art.is_some()).into(),
                self.ledger.refunded(id),
                self.ledger.reversed_by(id).is_some().into(),
                units.unwrap_or(0),
                self.ledger.corrections.len() as i64,
                crate::ledger::CORRECTIONS as i64,
                self.ledger
                    .corrections
                    .iter()
                    .any(|c| c.original == id)
                    .into(),
            ],
        )?;
        Ok(CorrectionPolicy {
            payer: MemberId(out[19] as u8),
            payee: MemberId(out[20] as u8),
            units: out[21],
            overdraft: out[22] != 0,
        })
    }
    pub(crate) fn correction_capacity(&self, id: u64) -> Result<(), Error> {
        let mut frame = [0; 19];
        frame[0] = 2;
        frame[16] = self.ledger.corrections.len() as i64;
        frame[17] = crate::ledger::CORRECTIONS as i64;
        frame[18] = self
            .ledger
            .corrections
            .iter()
            .any(|c| c.original == id)
            .into();
        policy(41, &frame)
    }
}

fn economic_tag(kind: crate::domain::EconomicKind) -> i64 {
    use crate::domain::EconomicKind::*;
    match kind {
        Other => 0,
        Labor => 1,
        Good => 2,
        LoanPrincipal => 3,
        Interest => 4,
        Gift => 5,
    }
}
impl State {
    pub(crate) fn transfer_policy(&self, actor: MemberId, command: &Command) -> Result<(), Error> {
        let (to, amount, memo, economic) = match command {
            Command::Transfer { to, amount, memo } => (*to, *amount, memo, None),
            Command::ClassifiedTransfer {
                to,
                amount,
                memo,
                economic,
            } => (*to, *amount, memo, Some(economic)),
            _ => return Err(Error::InvalidInput),
        };
        let source = self.member(actor).ok();
        let target = self.member(to).ok();
        let thing = economic.and_then(|e| self.thing(e.thing).ok());
        policy(
            43,
            &[
                actor.0.into(),
                to.0.into(),
                amount,
                0,
                0,
                source.is_some().into(),
                source.is_some_and(|m| m.disabled).into(),
                target.is_some().into(),
                target.is_some_and(|m| m.disabled).into(),
                self.bank_balance(actor, false),
                self.bank_balance(to, false),
                0,
                0,
                economic.is_some().into(),
                target
                    .is_some_and(|m| m.role == crate::domain::Role::Nana)
                    .into(),
                memo.trim().is_empty().into(),
                memo.chars().any(char::is_control).into(),
                economic.map_or(0, |e| e.quantity_milli.into()),
                economic.map_or(0, |e| economic_tag(e.kind)),
                economic.map_or(0, |e| e.unit as i64),
                economic.is_some_and(|e| e.thing != 0).into(),
                thing.is_some().into(),
                thing.map_or(0, |t| economic_tag(t.kind)),
                thing.map_or(0, |t| t.unit as i64),
            ],
        )
    }
    pub(crate) fn historical_amount(&self, tx: &Transaction, units: i64) -> Result<i64, Error> {
        let mut frame = [0; 37];
        frame[..5].copy_from_slice(&[
            units,
            tx.usd.into(),
            tx.meta.epoch as i64,
            self.ledger.epochs.len() as i64,
            crate::domain::MAX_AMOUNT,
        ]);
        for (out, epoch) in frame[5..].iter_mut().zip(&self.ledger.epochs) {
            *out = epoch.exponent.into();
        }
        Ok(call(42, &frame)?[37])
    }
}
