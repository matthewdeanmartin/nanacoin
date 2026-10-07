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

#[cfg(not(feature = "cobol-core"))]
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

#[cfg(not(feature = "cobol-core"))]
fn side(side: Side) -> &'static str {
    match side {
        Side::Sell => "SELL",
        Side::Buy => "BUY",
    }
}

/// The public event an audit row stands for, if any.
fn loan_payment_sequence(s: &State, loan: u64) -> Option<u64> {
    let rows = s.ledger.audit.iter().filter_map(|a| match &a.action {
        AuditAction::Business(
            Command::RepayLoan { loan: id, .. } | Command::RunLoan { loan: id, .. },
        ) if *id == loan => Some(a.sequence),
        _ => None,
    });
    #[cfg(not(feature = "cobol-core"))]
    {
        let mut rows = rows;
        rows.next_back()
    }
    #[cfg(feature = "cobol-core")]
    {
        let mut f = [0; 64];
        f[0] = 4;
        let mut best = 0;
        for sequence in rows {
            let index = f[1] as usize + 2;
            f[index] = crate::cobol::ledger_number(sequence);
            f[1] += 1;
            if f[1] == 60 {
                let next = crate::cobol::activity(&f).expect("valid latest-payment ABI")[63];
                best = crate::cobol::activity(&[4, 2, best, next])
                    .expect("valid latest-payment merge ABI")[63];
                f[1] = 0;
            }
        }
        if f[1] != 0 {
            let next = crate::cobol::activity(&f).expect("valid latest-payment ABI")[63];
            best = crate::cobol::activity(&[4, 2, best, next])
                .expect("valid latest-payment merge ABI")[63];
        }
        (best != 0).then_some(best as u64)
    }
}

#[cfg(feature = "cobol-core")]
fn event<'a>(s: &'a State, a: &'a Audit) -> Option<Event<'a>> {
    let n = crate::cobol::ledger_number;
    let mut f = [0; 29];
    f[1] = 14;
    f[2] = 1;
    f[12] = a.actor.0.into();
    f[13] = n(a.sequence);
    f[25] = n(a.at);
    f[26] = n(a.sequence);
    let mut title = "";
    let listing_side = |side: Side| match side {
        Side::Sell => 1,
        Side::Buy => 2,
    };
    let quote_side = |side: crate::forex::QuoteSide| match side {
        crate::forex::QuoteSide::BID => 3,
        crate::forex::QuoteSide::ASK => 4,
    };
    match &a.action {
        AuditAction::Identity {
            member,
            name,
            role,
            disabled,
            created,
            ..
        } => {
            f[1] = 0;
            let m = s.members.iter().find(|m| m.id == *member);
            f[2] = m.is_some().into();
            f[3] = (!name.is_empty()).into();
            f[4] = role.is_some().into();
            f[5] = disabled.is_some().into();
            f[28] = (*created).into();
            f[6] = m.map_or(0, |m| n(m.created_at));
            f[14] = member.0.into();
        }
        AuditAction::Business(command) => match command {
            Command::List {
                title: text,
                price,
                side,
                details,
                ..
            } => {
                f[1] = 1;
                title = text;
                f[7] = details.as_ref().is_some_and(|d| d.kind == GOOD_DEED).into();
                f[17] = *price;
                f[22] = listing_side(*side);
            }
            Command::ClassifiedList {
                title: text,
                price,
                side,
                ..
            } => {
                f[1] = 2;
                title = text;
                f[17] = *price;
                f[22] = listing_side(*side);
            }
            Command::Buy { listing } => {
                f[1] = 3;
                let l = s.listings.iter().find(|l| l.id == *listing);
                f[2] = l.is_some().into();
                f[13] = n(*listing);
                f[15] = a.actor.0.into();
                if let Some(l) = l {
                    title = &l.title;
                    f[17] = l.price;
                    f[14] = l.owner.0.into();
                }
            }
            Command::AcceptOffer { offer } => {
                f[1] = 4;
                let o = s.offers.iter().find(|o| o.id.0 == offer.0);
                f[2] = o.is_some().into();
                if let Some(o) = o {
                    title = &o.listing_title;
                    f[13] = n(o.listing);
                    f[17] = o.amount;
                    f[15] = o.offerer.0.into();
                    f[7] = s.listing(o.listing).is_ok_and(|l| l.is_good_deed()).into();
                }
            }
            Command::CreateLotto { terms } => {
                f[1] = 5;
                title = &terms.title;
                f[17] = terms.ticket_price;
                f[21] = n(terms.closes_at);
                f[23] = match terms.kind {
                    crate::lotto::LottoKind::Simple => 0,
                    crate::lotto::LottoKind::Delayed => 1,
                    crate::lotto::LottoKind::Savings => 2,
                };
            }
            Command::RunLotto { lotto, step, .. } => {
                f[1] = 6;
                f[8] = (*step).into();
                f[13] = n(*lotto);
                let l = s.lottos.iter().find(|l| l.id == *lotto);
                f[2] = l.is_some().into();
                if let Some(l) = l {
                    title = &l.terms.title;
                    f[9] = l.winner.is_some().into();
                    f[17] = l.pool;
                    f[14] = l.house.0.into();
                    f[15] = l.winner.map_or(0, |m| m.0.into());
                }
            }
            Command::RequestLoan { terms } => {
                f[1] = 7;
                f[17] = terms.amount;
                f[20] = terms.apr_bps().into();
            }
            Command::AcceptLoan { loan } => {
                f[1] = 8;
                f[13] = n(*loan);
                let l = s.loans.iter().find(|l| l.id == *loan);
                f[2] = l.is_some().into();
                if let Some(l) = l {
                    f[17] = l.terms.amount;
                    f[20] = l.terms.apr_bps().into();
                    f[14] = l.lender.0.into();
                    f[15] = l.terms.borrower.0.into();
                }
            }
            Command::RepayLoan { loan, .. } | Command::RunLoan { loan, .. } => {
                f[1] = 9;
                f[13] = n(*loan);
                f[27] = loan_payment_sequence(s, *loan).map_or(0, n);
                let l = s.loans.iter().find(|l| l.id == *loan);
                f[2] = l.is_some().into();
                if let Some(l) = l {
                    f[10] = crate::cobol::loan_tag(l.status);
                    f[11] = n(l.updated_at);
                    f[17] = l.terms.amount;
                    f[14] = l.terms.borrower.0.into();
                    f[15] = l.lender.0.into();
                }
            }
            Command::PostQuote {
                side,
                cents_per_coin,
                coins,
                ..
            } => {
                f[1] = 10;
                f[18] = *cents_per_coin;
                f[19] = *coins;
                f[22] = quote_side(*side);
            }
            Command::TakeQuote { quote } => {
                f[1] = 11;
                f[13] = n(*quote);
                let q = s.quotes.iter().find(|q| q.id == *quote);
                f[2] = q.is_some().into();
                if let Some(q) = q {
                    f[18] = q.cents_per_coin;
                    f[19] = q.coins;
                    f[22] = quote_side(q.side);
                    f[15] = q.maker.0.into();
                }
            }
            Command::Commerce { action } => match action {
                crate::commerce::Action::CreateRequest {
                    title: text,
                    target,
                    ..
                } => {
                    f[1] = 12;
                    title = text;
                    f[16] = target.is_some().into();
                    f[17] = target.unwrap_or(0);
                }
                crate::commerce::Action::MintArt { title: text, .. } => {
                    f[1] = 13;
                    title = text;
                }
                _ => {}
            },
            _ => {}
        },
    }
    let out = crate::cobol::activity(&f).expect("valid activity event ABI");
    if out[32] == 0 {
        return None;
    }
    let actor = MemberId(out[34] as u8);
    let member = s.member(actor).ok();
    let other = (out[35] >= 0).then(|| MemberId(out[35] as u8));
    Some(Event {
        seq: a.sequence,
        at: a.at,
        kind: [
            "",
            "member_joined",
            "listing_opened",
            "good_deed_posted",
            "listing_sold",
            "good_deed_claimed",
            "lotto_opened",
            "lotto_drawn",
            "loan_requested",
            "loan_funded",
            "loan_paid",
            "quote_posted",
            "quote_taken",
            "gift_request_opened",
            "art_minted",
        ][out[32] as usize],
        actor: account(actor),
        actor_name: member.map_or("", |m| m.name.as_str()),
        actor_bot: member.is_some_and(|m| m.kind == MemberKind::Bot),
        subject: if out[33] == 0 {
            Id::new()
        } else {
            id(
                [
                    "", "listing-", "lotto-", "loan-", "quote-", "request-", "art-",
                ][out[33] as usize],
                out[36] as u64,
            )
        },
        title,
        side: ["", "SELL", "BUY", "BID", "ASK"][out[43] as usize],
        amount: (out[37] != 0).then_some(out[38]),
        rate: (out[39] != 0).then_some(out[46]),
        coins: (out[40] != 0).then_some(out[47]),
        apr_bps: (out[41] != 0).then_some(out[48] as u32),
        closes_at: (out[42] != 0).then_some(out[49] as u64),
        lotto_kind: if out[44] != 0 {
            ["SIMPLE", "DELAYED", "SAVINGS"][out[45] as usize]
        } else {
            ""
        },
        other: other.map_or_else(Id::new, account),
        other_name: other
            .and_then(|m| s.member(m).ok())
            .map_or("", |m| m.name.as_str()),
    })
}

#[cfg(not(feature = "cobol-core"))]
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
            created,
            ..
        } => {
            // Joining is the identity row made when the member was created:
            // a name and a role, at the member's creation time. (A key or
            // password change in the same second has neither.)
            if !created || name.is_empty() || role.is_none() || disabled.is_some() {
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
            let l = s.loan(*loan).ok().filter(|l| {
                l.status == crate::loans::LoanStatus::Paid
                    && l.updated_at == a.at
                    && loan_payment_sequence(s, *loan) == Some(a.sequence)
            })?;
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
        #[cfg(feature = "cobol-core")]
        crate::cobol::policy(54, &[3, 0, s.state.member(actor)?.disabled.into()])?;
        #[cfg(not(feature = "cobol-core"))]
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
        let requested_limit = number("limit")?;
        #[cfg(not(feature = "cobol-core"))]
        let limit = match requested_limit {
            None => LIMIT,
            Some(n @ 1..=50) => n as usize,
            Some(_) => return Err(Error::InvalidInput),
        };
        let audit = &s.state.ledger.audit;
        #[cfg(feature = "cobol-core")]
        let (limit, truncated) = {
            let n = crate::cobol::ledger_number;
            let out = crate::cobol::activity(&[
                1,
                s.state.member(actor)?.disabled.into(),
                n(requested_limit.unwrap_or(0)),
                n(after),
                audit.len() as i64,
                crate::ledger::AUDIT_CACHE as i64,
                audit.front().is_some().into(),
                audit.front().map_or(0, |a| n(a.sequence)),
                requested_limit.is_some().into(),
            ])?;
            (out[32] as usize, out[33] != 0)
        };
        // Rows after `after` were evicted if the oldest kept row is later
        // than the event that followed `after`.
        #[cfg(not(feature = "cobol-core"))]
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
            .filter(|a| {
                #[cfg(feature = "cobol-core")]
                {
                    crate::cobol::activity(&[
                        2,
                        crate::cobol::ledger_number(a.sequence),
                        crate::cobol::ledger_number(after),
                    ])
                    .expect("valid activity cursor ABI")[32]
                        != 0
                }
                #[cfg(not(feature = "cobol-core"))]
                {
                    a.sequence > after
                }
            })
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
