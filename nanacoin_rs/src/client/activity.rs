//! `GET /api/v1/activity?after=<sequence>&limit=<n>`: what happened in the
//! economy, oldest first, for any active member (or read key). Built for the
//! NanaCoin news bot and trading bots: lottos, listings, loans, forex.
//!
//! It is a sanitized projection of the in-memory business audit, never the
//! audit itself: no memos, offer messages, zero-value messages, loan notes,
//! dispute or reversal reasons, credentials or identity changes. Only facts
//! every member can already read elsewhere (listings, lottos, the quote book,
//! loan requests, funded loans on profiles, public money movements).
use super::*;
use crate::ledger::{Audit, AuditAction};

const LIMIT: usize = 50;

#[derive(Serialize, Default)]
struct Event<'a> {
    seq: u64,
    at: u64,
    kind: &'static str,
    actor: Id,
    actor_name: &'a str,
    actor_bot: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    subject: Id,
    #[serde(skip_serializing_if = "str::is_empty")]
    title: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    side: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coins: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apr_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    closes_at: Option<u64>,
    #[serde(skip_serializing_if = "str::is_empty")]
    lotto_kind: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    other: Id,
    #[serde(skip_serializing_if = "str::is_empty")]
    other_name: &'a str,
}

fn side(side: Side) -> &'static str {
    match side {
        Side::Sell => "SELL",
        Side::Buy => "BUY",
    }
}

/// The public event an audit row stands for, if any.
fn event<'a>(s: &'a State, a: &'a Audit) -> Option<Event<'a>> {
    let who = |m: MemberId| s.member(m).ok();
    let actor = who(a.actor);
    let mut e = Event {
        seq: a.sequence,
        at: a.at,
        actor: account(a.actor),
        actor_name: actor.map_or("", |m| m.name.as_str()),
        actor_bot: actor.is_some_and(|m| m.kind == MemberKind::Bot),
        ..Event::default()
    };
    let other = |e: &mut Event<'a>, m: MemberId| {
        e.other = account(m);
        e.other_name = who(m).map_or("", |m| m.name.as_str());
    };
    // The service itself (account 0) acts for scheduled steps: name the
    // member the step is about instead.
    let as_actor = |e: &mut Event<'a>, m: MemberId| {
        let member = who(m);
        e.actor = account(m);
        e.actor_name = member.map_or("", |m| m.name.as_str());
        e.actor_bot = member.is_some_and(|m| m.kind == MemberKind::Bot);
    };
    let command = match &a.action {
        AuditAction::Identity {
            member,
            name,
            role,
            disabled,
            ..
        } => {
            // Joining is the identity row made when the member was created:
            // a name and a role, at the member's creation time. (A key or
            // password change in the same second has neither.)
            if name.is_empty() || role.is_none() || disabled.is_some() {
                return None;
            }
            let m = who(*member).filter(|m| m.created_at == a.at)?;
            e.kind = "member_joined";
            as_actor(&mut e, m.id);
            return Some(e);
        }
        AuditAction::Business(command) => command,
    };
    match command {
        Command::List {
            title,
            price,
            side: listing_side,
            details,
            ..
        } => {
            e.kind = if details.as_ref().is_some_and(|d| d.kind == GOOD_DEED) {
                "good_deed_posted"
            } else {
                "listing_opened"
            };
            e.subject = id("listing-", a.sequence);
            e.title = title;
            e.side = side(*listing_side);
            e.amount = Some(*price);
        }
        Command::ClassifiedList {
            title,
            price,
            side: listing_side,
            ..
        } => {
            e.kind = "listing_opened";
            e.subject = id("listing-", a.sequence);
            e.title = title;
            e.side = side(*listing_side);
            e.amount = Some(*price);
        }
        Command::Buy { listing } => {
            let l = s.listing(*listing).ok()?;
            e.kind = "listing_sold";
            e.subject = id("listing-", l.id);
            e.title = &l.title;
            e.amount = Some(l.price);
            other(&mut e, a.actor);
            as_actor(&mut e, l.owner);
        }
        Command::AcceptOffer { offer } => {
            let o = s.offer(*offer).ok()?;
            let good_deed = s.listing(o.listing).is_ok_and(|l| l.is_good_deed());
            e.kind = if good_deed {
                "good_deed_claimed"
            } else {
                "listing_sold"
            };
            e.subject = id("listing-", o.listing);
            e.title = &o.listing_title;
            e.amount = Some(o.amount);
            other(&mut e, o.offerer);
        }
        Command::CreateLotto { terms } => {
            e.kind = "lotto_opened";
            e.subject = id("lotto-", a.sequence);
            e.title = &terms.title;
            e.amount = Some(terms.ticket_price);
            e.closes_at = Some(terms.closes_at);
            e.lotto_kind = match terms.kind {
                crate::lotto::LottoKind::Simple => "SIMPLE",
                crate::lotto::LottoKind::Delayed => "DELAYED",
                crate::lotto::LottoKind::Savings => "SAVINGS",
            };
        }
        // Step 0 is the draw.
        Command::RunLotto { lotto, step: 0, .. } => {
            let l = s.lotto(*lotto).ok()?;
            e.kind = "lotto_drawn";
            e.subject = id("lotto-", l.id);
            e.title = &l.terms.title;
            e.amount = Some(l.pool);
            as_actor(&mut e, l.house);
            other(&mut e, l.winner?);
        }
        Command::RequestLoan { terms } => {
            e.kind = "loan_requested";
            e.subject = id("loan-", a.sequence);
            e.amount = Some(terms.amount);
            e.apr_bps = Some(terms.apr_bps());
        }
        Command::AcceptLoan { loan } => {
            let l = s.loan(*loan).ok()?;
            e.kind = "loan_funded";
            e.subject = id("loan-", l.id);
            e.amount = Some(l.terms.amount);
            e.apr_bps = Some(l.terms.apr_bps());
            as_actor(&mut e, l.lender);
            other(&mut e, l.terms.borrower);
        }
        // The payment that paid a loan off (its last change).
        Command::RepayLoan { loan, .. } | Command::RunLoan { loan, .. } => {
            let l = s
                .loan(*loan)
                .ok()
                .filter(|l| l.status == crate::loans::LoanStatus::Paid && l.updated_at == a.at)?;
            e.kind = "loan_paid";
            e.subject = id("loan-", l.id);
            e.amount = Some(l.terms.amount);
            as_actor(&mut e, l.terms.borrower);
            other(&mut e, l.lender);
        }
        Command::PostQuote {
            side: quote_side,
            cents_per_coin,
            coins,
            ..
        } => {
            e.kind = "quote_posted";
            e.subject = id("quote-", a.sequence);
            e.side = match quote_side {
                crate::forex::QuoteSide::BID => "BID",
                crate::forex::QuoteSide::ASK => "ASK",
            };
            e.rate = Some(*cents_per_coin);
            e.coins = Some(*coins);
        }
        Command::TakeQuote { quote } => {
            let q = s.quote(*quote).ok()?;
            e.kind = "quote_taken";
            e.subject = id("quote-", q.id);
            e.side = match q.side {
                crate::forex::QuoteSide::BID => "BID",
                crate::forex::QuoteSide::ASK => "ASK",
            };
            e.rate = Some(q.cents_per_coin);
            e.coins = Some(q.coins);
            other(&mut e, q.maker);
        }
        Command::Commerce { action } => match action {
            crate::commerce::Action::CreateRequest { title, target, .. } => {
                e.kind = "gift_request_opened";
                e.subject = id("request-", a.sequence);
                e.title = title;
                e.amount = *target;
            }
            crate::commerce::Action::MintArt { title, .. } => {
                e.kind = "art_minted";
                e.subject = id("art-", a.sequence);
                e.title = title;
            }
            _ => return None,
        },
        _ => return None,
    }
    Some(e)
}

pub(super) fn route<J: Journal>(
    s: &mut Service<J>,
    actor: MemberId,
    method: &str,
    path: &str,
    query: &str,
    output: &mut [u8],
) -> Option<Result<usize, Error>> {
    if path != "/api/v1/activity" {
        return None;
    }
    Some((|| {
        if method != "GET" {
            return Err(Error::NotFound);
        }
        if s.state.member(actor)?.disabled {
            return Err(Error::Forbidden);
        }
        let number = |name: &str| -> Result<Option<u64>, Error> {
            query
                .split('&')
                .find_map(|p| p.strip_prefix(name)?.strip_prefix('='))
                .map(|v| v.parse::<u64>().map_err(|_| Error::InvalidInput))
                .transpose()
        };
        let after = number("after")?.unwrap_or(0);
        let limit = match number("limit")? {
            None => LIMIT,
            Some(n @ 1..=50) => n as usize,
            Some(_) => return Err(Error::InvalidInput),
        };
        let audit = &s.state.ledger.audit;
        // Rows after `after` were evicted if the oldest kept row is later
        // than the event that followed `after`.
        let truncated = audit.len() == crate::ledger::AUDIT_CACHE
            && audit
                .front()
                .is_some_and(|first| next_sequence(after).is_some_and(|n| first.sequence > n));
        #[derive(Serialize)]
        struct Feed<I> {
            incarnation: u64,
            generation: u64,
            sequence: u64,
            decimals: u8,
            truncated: bool,
            events: I,
        }
        let state = &s.state;
        let events = audit
            .iter()
            .filter(|a| a.sequence > after)
            .filter_map(|a| event(state, a))
            .take(limit);
        serialize(
            &Feed {
                incarnation: state.archive.incarnation,
                generation: s.generation(),
                sequence: state.sequence,
                decimals: state.decimals,
                truncated,
                events: Rows(events),
            },
            output,
        )
    })())
}
