use super::*;
use crate::loans::{Loan, LoanStatus, LoanTerms};

#[derive(Serialize)]
struct LoanView<'a> {
    id: u64,
    lender: Id,
    lender_name: &'a str,
    borrower: Id,
    borrower_name: &'a str,
    amount: i64,
    rate_bps: u32,
    rate_days: u16,
    payment_days: u16,
    installment: i64,
    credit: bool,
    memo: &'a str,
    status: LoanStatus,
    principal: i64,
    interest: i64,
    overdue: i64,
    next_due_at: u64,
    created_at: u64,
    updated_at: u64,
    waiting_reason: &'static str,
}
fn view<'a>(s: &'a State, l: &'a Loan, now: u64) -> LoanView<'a> {
    let mut accrued = l.clone();
    let waiting_reason =
        if s.member(l.lender).unwrap().disabled || s.member(l.terms.borrower).unwrap().disabled {
            "An account is disabled"
        } else if l.status == LoanStatus::Active && accrued.accrue(now).is_err() {
            "Clock or arithmetic limit; payment is paused"
        } else if l.status == LoanStatus::Armed {
            if s.member(l.terms.borrower).unwrap().balance != 0 {
                "Waiting for a zero balance"
            } else if s.credit_blocked & (1u16 << (l.terms.borrower.0 - 1)) != 0 {
                "Credit does not fund loan payments"
            } else if s.member(l.lender).unwrap().balance < l.terms.amount {
                "Waiting for lender funds"
            } else {
                "Ready for automatic funding"
            }
        } else {
            ""
        };
    LoanView {
        id: l.id,
        lender: account(l.lender),
        lender_name: s.member(l.lender).unwrap().name.as_str(),
        borrower: account(l.terms.borrower),
        borrower_name: s.member(l.terms.borrower).unwrap().name.as_str(),
        amount: l.terms.amount,
        rate_bps: l.terms.rate_bps,
        rate_days: l.terms.rate_days,
        payment_days: l.terms.payment_days,
        installment: l.terms.installment,
        credit: l.terms.credit,
        memo: l.terms.memo.as_str(),
        status: l.status,
        principal: l.principal,
        interest: accrued.interest,
        overdue: l.principal_due + l.interest_due,
        next_due_at: l.next_due_at,
        created_at: l.created_at,
        updated_at: l.updated_at,
        waiting_reason,
    }
}
#[derive(Serialize)]
struct Summary {
    outstanding: heapless::String<32>,
    overdue: heapless::String<32>,
    weighted_annual_percent: Option<f64>,
    active: usize,
}
fn summary(s: &State) -> Summary {
    use core::fmt::Write;
    let mut principal: i128 = 0;
    let mut overdue: i128 = 0;
    let mut weighted = 0.0;
    let mut active = 0;
    for l in &s.loans {
        if l.status != LoanStatus::Active {
            continue;
        }
        active += 1;
        principal += l.principal as i128;
        overdue += (l.principal_due + l.interest_due) as i128;
        weighted +=
            l.principal as f64 * l.terms.rate_bps as f64 / 100.0 * 365.0 / l.terms.rate_days as f64;
    }
    let mut outstanding = heapless::String::new();
    write!(outstanding, "{principal}").unwrap();
    let mut debt = heapless::String::new();
    write!(debt, "{overdue}").unwrap();
    Summary {
        outstanding,
        overdue: debt,
        weighted_annual_percent: if principal > 0 {
            Some(weighted / principal as f64)
        } else {
            None
        },
        active,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn route<J: Journal>(
    s: &mut Service<J>,
    actor: MemberId,
    method: &str,
    path: &str,
    key: &str,
    body: &[u8],
    output: &mut [u8],
) -> Option<Result<usize, Error>> {
    if path != "/api/v1/loans"
        && !path.starts_with("/api/v1/loans/")
        && path != "/api/v1/admin/reform"
    {
        return None;
    }
    Some((|| {
        if path == "/api/v1/admin/reform" {
            s.state.admin(actor)?;
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Reform {
                decimals: u8,
                power: i8,
                expected_epoch: u64,
                expected_sequence: u64,
                preview: bool,
            }
            let r: Reform = parse(body)?;
            if method != "POST" {
                return Err(Error::NotFound);
            }
            if r.preview {
                if r.expected_epoch != s.state.money_epoch
                    || r.expected_sequence != s.state.sequence
                {
                    return Err(Error::Conflict);
                }
                s.state.validate_reform(r.decimals, r.power)?;
            } else {
                s.execute_keyed(
                    actor,
                    key,
                    Command::ReformCurrency {
                        decimals: r.decimals,
                        power: r.power,
                        expected_epoch: r.expected_epoch,
                        expected_sequence: r.expected_sequence,
                    },
                )?;
            }
            #[derive(Serialize)]
            struct ReformView {
                decimals: u8,
                money_epoch: u64,
                sequence: u64,
                circulation: i64,
                preview: bool,
            }
            let circulation = if r.preview {
                crate::money::rescale(
                    -s.state.issuance_balance,
                    r.decimals as i16 - s.state.decimals as i16 - r.power as i16,
                    MAX_SEQUENCE as i64,
                )?
            } else {
                -s.state.issuance_balance
            };
            return serialize(
                &ReformView {
                    decimals: r.decimals,
                    money_epoch: s.state.money_epoch,
                    sequence: s.state.sequence,
                    circulation,
                    preview: r.preview,
                },
                output,
            );
        }
        if path == "/api/v1/loans" && method == "GET" {
            #[derive(Serialize)]
            struct Response<T> {
                loans: T,
                summary: Summary,
                decimals: u8,
                money_epoch: u64,
                sequence: u64,
            }
            let member = s.state.member(actor)?;
            return serialize(
                &Response {
                    loans: Rows(
                        s.state
                            .loans
                            .iter()
                            .rev()
                            .filter(|l| l.visible_to(member))
                            .map(|l| view(&s.state, l, s.now())),
                    ),
                    summary: summary(&s.state),
                    decimals: s.state.decimals,
                    money_epoch: s.state.money_epoch,
                    sequence: s.state.sequence,
                },
                output,
            );
        }
        if method != "POST" {
            return Err(Error::NotFound);
        }
        let (command, existing) = if path == "/api/v1/loans" {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Offer {
                borrower: Id,
                amount: i64,
                rate_bps: u32,
                rate_days: u16,
                payment_days: u16,
                installment: i64,
                credit: bool,
                memo: Memo,
            }
            let r: Offer = parse(body)?;
            (
                Command::OfferLoan {
                    terms: LoanTerms {
                        borrower: member_id(&r.borrower, "account-")?,
                        amount: r.amount,
                        rate_bps: r.rate_bps,
                        rate_days: r.rate_days,
                        payment_days: r.payment_days,
                        installment: r.installment,
                        credit: r.credit,
                        memo: r.memo,
                    },
                },
                None,
            )
        } else {
            let tail = path.strip_prefix("/api/v1/loans/").unwrap();
            let (id, action) = tail.split_once('/').ok_or(Error::NotFound)?;
            let loan = id.parse::<u64>().map_err(|_| Error::InvalidInput)?;
            let command = match action {
                "accept" => Command::AcceptLoan { loan },
                "close" => Command::CloseLoan { loan },
                "repay" => {
                    #[derive(Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Repay {
                        amount: i64,
                    }
                    Command::RepayLoan {
                        loan,
                        amount: parse::<Repay>(body)?.amount,
                    }
                }
                _ => return Err(Error::NotFound),
            };
            (command, Some(loan))
        };
        let receipt = s.execute_keyed(actor, key, command)?;
        let loan = s.state.loan(existing.unwrap_or(receipt.sequence))?;
        serialize(&view(&s.state, loan, s.now()), output)
    })())
}
