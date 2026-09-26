//! Wire adapter for the existing nanacoin_web/Angular client. Domain IDs and
//! commands stay strongly typed; the adapter owns HTTP strings and views.
mod commerce;
mod history;
use crate::{
    api::{parse, serialize},
    domain::*,
    journal::{Journal, Service, FRAME_SIZE, MAX_RECORDS},
};
use core::fmt::Write;
use heapless::String;
use serde::{ser::SerializeSeq, Deserialize, Serialize};

type Id = String<32>;
mod forex;
mod fulfillment;
mod loans;
mod lotto;
mod offers;
fn id(prefix: &str, number: u64) -> Id {
    let mut out = Id::new();
    write!(out, "{prefix}{number}").unwrap();
    out
}
fn account(member: MemberId) -> Id {
    if member == crate::lotto::ESCROW {
        Id::try_from("account:lotto-escrow").unwrap()
    } else if member.0 == 0 {
        Id::try_from("account:system-issuance").unwrap()
    } else {
        id("account-", member.0 as u64)
    }
}
fn member_id(value: &str, prefix: &str) -> Result<MemberId, Error> {
    value
        .strip_prefix(prefix)
        .and_then(|v| v.parse::<u8>().ok())
        .filter(|n| *n > 0)
        .map(MemberId)
        .ok_or(Error::InvalidInput)
}
fn number(value: &str, prefix: &str) -> Result<u64, Error> {
    value
        .strip_prefix(prefix)
        .and_then(|v| v.parse().ok())
        .ok_or(Error::InvalidInput)
}

fn quantity_milli(value: &str) -> Result<u32, Error> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    let fraction_len = fraction.len();
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 3
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(Error::InvalidInput);
    }
    let whole: u32 = whole.parse().map_err(|_| Error::InvalidInput)?;
    let fraction: u32 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| Error::InvalidInput)?
    };
    let scale = match fraction_len {
        0 => 1000,
        1 => 100,
        2 => 10,
        3 => 1,
        _ => return Err(Error::InvalidInput),
    };
    whole
        .checked_mul(1000)
        .and_then(|n| n.checked_add(fraction * scale))
        .filter(|n| (1..=1_000_000_000).contains(n))
        .ok_or(Error::InvalidInput)
}

#[derive(Serialize)]
pub(crate) struct User<'a> {
    id: Id,
    username: &'a str,
    display_name: &'a str,
    role: Role,
    status: &'static str,
    account: Id,
    created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    mastodon_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    balance: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usd_cents: Option<i64>,
}
pub(crate) fn user(member: &Member) -> User<'_> {
    User {
        id: id("user-", member.id.0 as u64),
        username: &member.username,
        display_name: &member.name,
        role: member.role,
        status: if member.disabled {
            "DISABLED"
        } else {
            "ACTIVE"
        },
        account: account(member.id),
        created_at: member.created_at,
        mastodon_id: (!member.mastodon_id.is_empty()).then_some(member.mastodon_id.as_str()),
        balance: Some(member.balance),
        usd_cents: Some(member.usd_cents),
    }
}

struct Rows<I>(I);
impl<I: Iterator + Clone> Serialize for Rows<I>
where
    I::Item: Serialize,
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        for row in self.0.clone() {
            seq.serialize_element(&row)?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct Posting<'a> {
    account: Id,
    name: &'a str,
    amount: i64,
}
#[derive(Serialize)]
struct TransactionView<'a> {
    metadata: crate::ledger::TransactionMeta,
    current_money_epoch: u64,
    current_postings: Option<[Posting<'a>; 2]>,
    refunded: i64,
    fulfillment: Option<fulfillment::View<'a>>,
    id: Id,
    kind: &'static str,
    created_at: u64,
    actor: Id,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reverses: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reversed_by: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<Id>,
    economic_kind: EconomicKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    thing: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thing_name: Option<&'a str>,
    quantity_milli: u32,
    unit: Unit,
    postings: [Posting<'a>; 2],
}
fn transaction<'a>(state: &'a State, tx: &'a Transaction) -> TransactionView<'a> {
    let name = |member| {
        if member == crate::lotto::ESCROW {
            "Lotto escrow"
        } else if member == MemberId(0) {
            "Issuance"
        } else {
            state.member(member).map(|m| m.name.as_str()).unwrap_or("")
        }
    };
    TransactionView {
        metadata: tx.meta,
        current_money_epoch: state.money_epoch,
        current_postings: state.current_amount(tx, tx.amount).ok().map(|amount| {
            [
                Posting {
                    account: currency_account(tx.from, tx.usd),
                    name: name(tx.from),
                    amount: -amount,
                },
                Posting {
                    account: currency_account(tx.to, tx.usd),
                    name: name(tx.to),
                    amount,
                },
            ]
        }),
        refunded: state.ledger.refunded(tx.id),
        fulfillment: state
            .fulfillments
            .iter()
            .find(|f| f.transaction == tx.id)
            .map(|f| fulfillment::view(state, f)),
        id: id("tx-", tx.id),
        kind: if tx.amount == 0 {
            "MESSAGE"
        } else if tx.reverses.is_some() {
            "REVERSAL"
        } else if tx.listing.is_some() {
            "PURCHASE"
        } else if tx.from == MemberId(0) {
            "ISSUE"
        } else if tx.to == MemberId(0) {
            "RETIRE"
        } else {
            "TRANSFER"
        },
        created_at: tx.created_at,
        actor: id("user-", tx.actor.0 as u64),
        description: &tx.memo,
        reverses: tx.reverses.map(|n| id("tx-", n)),
        reversed_by: state.ledger.reversed_by(tx.id).map(|n| id("tx-", n)),
        reference: tx
            .loan
            .map(|l| id("loan-", l))
            .or_else(|| tx.lotto.map(|l| id("lotto-", l)))
            .or_else(|| tx.quote.map(|q| id("quote-", q))),
        economic_kind: tx.economic.kind,
        thing: (tx.economic.thing != 0).then(|| id("thing-", tx.economic.thing)),
        thing_name: state
            .things
            .iter()
            .find(|t| t.id == tx.economic.thing)
            .map(|t| t.name.as_str()),
        quantity_milli: tx.economic.quantity_milli,
        unit: tx.economic.unit,
        postings: [
            Posting {
                account: currency_account(tx.from, tx.usd),
                name: name(tx.from),
                amount: -tx.amount,
            },
            Posting {
                account: currency_account(tx.to, tx.usd),
                name: name(tx.to),
                amount: tx.amount,
            },
        ],
    }
}
#[derive(Serialize)]
struct ListingView<'a> {
    id: Id,
    seller: Id,
    seller_name: &'a str,
    title: &'a str,
    description: &'a str,
    price: i64,
    side: &'static str,
    status: &'static str,
    created_at: u64,
    updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    buyer: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sold_tx: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    buyer_name: Option<&'a str>,
    kind: &'a str,
    currency: &'a str,
    minor_units: i64,
    economic_kind: EconomicKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    thing: Option<Id>,
    quantity_milli: u32,
    unit: Unit,
    standard: bool,
}
fn listing<'a>(state: &'a State, l: &'a Listing) -> ListingView<'a> {
    ListingView {
        id: id("listing-", l.id),
        seller: account(l.owner),
        seller_name: state.member(l.owner).map(|m| m.name.as_str()).unwrap_or(""),
        title: &l.title,
        description: &l.description,
        price: l.price,
        side: if l.side == Side::Sell { "SELL" } else { "BUY" },
        status: match l.status {
            ListingStatus::Active => "ACTIVE",
            ListingStatus::Sold => "SOLD",
            ListingStatus::Cancelled => "CANCELLED",
        },
        created_at: l.created_at,
        updated_at: l.updated_at,
        buyer: l.buyer.map(account),
        sold_tx: l.sold_tx.map(|n| id("tx-", n)),
        buyer_name: l
            .buyer
            .and_then(|m| state.member(m).ok())
            .map(|m| m.name.as_str()),
        kind: &l.details.kind,
        currency: &l.details.currency,
        minor_units: l.details.minor_units,
        economic_kind: l.economic.kind,
        thing: (l.economic.thing != 0).then(|| id("thing-", l.economic.thing)),
        quantity_milli: l.economic.quantity_milli,
        unit: l.economic.unit,
        standard: state
            .things
            .iter()
            .find(|t| t.id == l.economic.thing)
            .is_some_and(|t| t.standard),
    }
}

#[derive(Serialize)]
struct ThingView<'a> {
    id: Id,
    name: &'a str,
    economic_kind: EconomicKind,
    unit: Unit,
    standard: bool,
    updated_at: u64,
}

fn thing(value: &Thing) -> ThingView<'_> {
    ThingView {
        id: id("thing-", value.id),
        name: &value.name,
        economic_kind: value.kind,
        unit: value.unit,
        standard: value.standard,
        updated_at: value.updated_at,
    }
}

pub(crate) fn status(
    state: &State,
    records: usize,
    generation: u64,
    checkpoint_supported: bool,
    output: &mut [u8],
) -> Result<usize, Error> {
    #[derive(Serialize)]
    struct Status<'a> {
        decimals: u8,
        money_epoch: u64,
        sequence: u64,
        lending_enabled: bool,
        provisioned: bool,
        household: &'a str,
        currency: &'a str,
        users: usize,
        transactions: u64,
        retained_transactions: usize,
        transaction_capacity: usize,
        oldest_transaction: u64,
        active_listings: usize,
        circulation: i64,
        journal_used: u64,
        journal_capacity: usize,
        journal_generation: u64,
        checkpoint_supported: bool,
        ledger_balanced: bool,
        logs_enabled: bool,
        diag_enabled: bool,
        login_migration_required: bool,
    }
    serialize(
        &Status {
            decimals: state.decimals,
            money_epoch: state.money_epoch,
            sequence: state.sequence,
            lending_enabled: true,
            provisioned: !state.members.is_empty(),
            household: if state.household_name.is_empty() {
                "NanaCoin"
            } else {
                &state.household_name
            },
            currency: &state.currency,
            users: state.members.len(),
            transactions: state.transactions,
            retained_transactions: state.history.len(),
            transaction_capacity: HISTORY,
            oldest_transaction: state.history.front().map(|t| t.id).unwrap_or(0),
            active_listings: state
                .listings
                .iter()
                .filter(|l| l.status == ListingStatus::Active)
                .count(),
            circulation: -state.issuance_balance,
            journal_used: (records * FRAME_SIZE) as u64,
            journal_capacity: if checkpoint_supported {
                2048 * FRAME_SIZE
            } else {
                MAX_RECORDS * FRAME_SIZE
            },
            journal_generation: generation,
            checkpoint_supported,
            ledger_balanced: state.check_invariants().is_ok(),
            logs_enabled: false,
            diag_enabled: cfg!(target_os = "espidf"),
            login_migration_required: state.needs_login_migration(),
        },
        output,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn route<J: Journal>(
    s: &mut Service<J>,
    actor: MemberId,
    method: &str,
    uri: &str,
    key: &str,
    body: &[u8],
    output: &mut [u8],
) -> Result<usize, Error> {
    if let Some(result) = commerce::route(
        s,
        actor,
        method,
        uri.split_once('?').map_or(uri, |v| v.0),
        key,
        body,
        output,
    ) {
        return result;
    }
    if let Some(result) = history::route(s, actor, method, uri, key, body, output) {
        return result;
    }
    let (path, query) = uri.split_once('?').unwrap_or((uri, ""));
    if let Some(result) = fulfillment::route(s, actor, method, path, key, body, output) {
        return result;
    }
    if let Some(result) = lotto::route(s, actor, method, path, key, body, output) {
        return result;
    }
    if let Some(result) = loans::route(s, actor, method, path, key, body, output) {
        return result;
    }
    if let Some(result) = offers::route(s, actor, method, path, key, body, output) {
        return result;
    }
    if let Some(result) = forex::route(s, actor, method, path, key, body, output) {
        return result;
    }
    let is_admin = s.state.member(actor)?.role == Role::Nana;
    let limit = query
        .split('&')
        .find_map(|p| p.strip_prefix("limit="))
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(if path == "/api/v1/transactions" {
            100
        } else {
            50
        })
        .min(100);
    if path == "/api/v1/admin/config" {
        if s.state.member(actor)?.role != Role::Nana {
            return Err(Error::Forbidden);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Changes {
            household_name: Option<Name>,
            initial_grant: Option<i64>,
            currency: Option<Name>,
            offer_settles_after: Option<u64>,
        }
        if method == "PATCH" {
            let r: Changes = parse(body)?;
            s.execute(
                actor,
                s.state.member(actor)?.last_request + 1,
                Command::Configure {
                    household_name: r
                        .household_name
                        .unwrap_or_else(|| s.state.household_name.clone()),
                    initial_grant: r.initial_grant.unwrap_or(s.state.initial_grant),
                    currency: r.currency.unwrap_or_else(|| s.state.currency.clone()),
                    offer_settles_after: r.offer_settles_after,
                },
            )?;
        } else if method != "GET" {
            return Err(Error::NotFound);
        }
        #[derive(Serialize)]
        struct Config<'a> {
            household_name: &'a str,
            initial_grant: i64,
            currency: &'a str,
            offer_settles_after: u64,
        }
        return serialize(
            &Config {
                household_name: &s.state.household_name,
                initial_grant: s.state.initial_grant,
                currency: &s.state.currency,
                offer_settles_after: s.state.offer_settles_after,
            },
            output,
        );
    }
    match (method, path) {
        ("GET", "/api/v1/me") => return serialize(&user(s.state.member(actor)?), output),
        ("GET", "/api/v1/users") => {
            #[derive(Serialize)]
            struct Response<T> {
                users: T,
            }
            return serialize(
                &Response {
                    // Balances are part of the shared household ledger. A
                    // future private-description flag may hide memo text, but
                    // amounts and postings remain auditable by every member.
                    users: Rows(s.state.members.iter().map(user)),
                },
                output,
            );
        }
        ("GET", "/api/v1/listings") => {
            let wanted = query
                .split('&')
                .find_map(|p| p.strip_prefix("status="))
                .filter(|s| !s.is_empty());
            if wanted.is_some_and(|s| !["ACTIVE", "SOLD", "CANCELLED"].contains(&s)) {
                return Err(Error::InvalidInput);
            }
            return listings_query(&s.state, wanted, output);
        }
        ("GET", "/api/v1/things") => {
            #[derive(Serialize)]
            struct Response<T> {
                things: T,
            }
            return serialize(
                &Response {
                    things: Rows(s.state.things.iter().map(thing)),
                },
                output,
            );
        }
        ("GET", "/api/v1/transactions") => {
            return public_ledger(&s.state, limit, output);
        }
        _ => {}
    }
    if method == "GET" {
        if let Some(account_id) = path
            .strip_prefix("/api/v1/accounts/")
            .and_then(|p| p.strip_suffix("/transactions"))
        {
            let (member, usd) = parse_account(account_id)?;
            if !is_admin && actor != member {
                return Err(Error::Forbidden);
            }
            #[derive(Serialize)]
            struct Response<T> {
                account: Id,
                balance: i64,
                transactions: T,
            }
            return serialize(
                &Response {
                    account: currency_account(member, usd),
                    balance: account_balance(&s.state, member, usd)?,
                    transactions: Rows(
                        s.state
                            .history
                            .iter()
                            .rev()
                            .filter(|t| t.usd == usd && (t.from == member || t.to == member))
                            .filter(|t| t.amount != 0 || t.from == actor || t.to == actor)
                            .take(limit)
                            .map(|t| transaction(&s.state, t)),
                    ),
                },
                output,
            );
        }
        if let Some(tx) = path.strip_prefix("/api/v1/transactions/") {
            let tx_id = number(tx, "tx-")?;
            s.state
                .history
                .iter()
                .find(|t| t.id == tx_id && (t.amount != 0 || t.from == actor || t.to == actor))
                .ok_or(Error::NotFound)?;
            return transaction_response(&s.state, tx_id, output);
        }
    }
    if method == "GET" {
        if let Some(value) = path.strip_prefix("/api/v1/listings/") {
            return serialize(
                &listing(&s.state, s.state.listing(number(value, "listing-")?)?),
                output,
            );
        }
        if let Some(value) = path.strip_prefix("/api/v1/accounts/") {
            let (member, usd) = parse_account(value)?;
            if !is_admin && actor != member {
                return Err(Error::Forbidden);
            }
            #[derive(Serialize)]
            struct Account<'a> {
                id: Id,
                user_id: Id,
                name: &'a str,
                status: &'static str,
                balance: i64,
            }
            let m = if member == MemberId(0) {
                None
            } else {
                Some(s.state.member(member)?)
            };
            return serialize(
                &Account {
                    id: currency_account(member, usd),
                    user_id: m.map(|m| id("user-", m.id.0 as u64)).unwrap_or_default(),
                    name: m.map(|m| m.name.as_str()).unwrap_or("Issuance"),
                    status: if m.is_some_and(|m| m.disabled) {
                        "DISABLED"
                    } else {
                        "ACTIVE"
                    },
                    balance: account_balance(&s.state, member, usd)?,
                },
                output,
            );
        }
    }
    if method == "PATCH" && path.starts_with("/api/v1/listings/") {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Update {
            title: Option<Title>,
            description: Option<Memo>,
            price: Option<i64>,
        }
        let listing_id = number(&path[17..], "listing-")?;
        let r: Update = parse(body)?;
        s.execute(
            actor,
            s.state.member(actor)?.last_request + 1,
            Command::UpdateListing {
                listing: listing_id,
                title: r.title,
                description: r.description,
                price: r.price,
            },
        )?;
        return serialize(
            &listing(
                &s.state,
                s.state
                    .listings
                    .iter()
                    .find(|l| l.id == listing_id)
                    .ok_or(Error::NotFound)?,
            ),
            output,
        );
    }
    if method == "PATCH" && path.starts_with("/api/v1/users/") {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Update {
            display_name: Option<Name>,
            password: Option<String<128>>,
            role: Option<Role>,
            status: Option<String<16>>,
            mastodon_id: Option<MastodonId>,
        }
        let member = member_id(&path[14..], "user-")?;
        let req: Update = parse(body)?;
        let disabled = req
            .status
            .as_ref()
            .map(|v| match v.as_str() {
                "ACTIVE" => Ok(false),
                "DISABLED" => Ok(true),
                _ => Err(Error::InvalidInput),
            })
            .transpose()?;
        if s.state.member(actor)?.role != Role::Nana
            && (member != actor || req.role.is_some() || disabled.is_some())
        {
            return Err(Error::Forbidden);
        }
        let password = req
            .password
            .as_ref()
            .map(|p| crate::auth::PasswordVerifier::hash(p))
            .transpose()?;
        s.execute(
            actor,
            s.state.member(actor)?.last_request + 1,
            Command::UpdateMember {
                member,
                display_name: req.display_name,
                password,
                role: req.role,
                disabled,
                mastodon_id: req.mastodon_id,
            },
        )?;
        return serialize(&user(s.state.member(member)?), output);
    }
    if method != "POST" {
        return Err(Error::NotFound);
    }
    let command = match path {
        "/api/v1/transfers" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Transfer {
                to: Id,
                amount: i64,
                #[serde(default)]
                memo: Memo,
                economic_kind: Option<EconomicKind>,
                thing: Option<Id>,
                quantity: Option<String<24>>,
                unit: Option<Unit>,
            }
            let r: Transfer = parse(body)?;
            if r.amount == 0 {
                Command::Transfer {
                    to: member_id(&r.to, "account-")?,
                    amount: 0,
                    memo: r.memo,
                }
            } else if r.economic_kind.is_some()
                || r.thing.is_some()
                || r.quantity.is_some()
                || r.unit.is_some()
            {
                Command::ClassifiedTransfer {
                    to: member_id(&r.to, "account-")?,
                    amount: r.amount,
                    memo: r.memo,
                    economic: EconomicDetails {
                        kind: r.economic_kind.ok_or(Error::InvalidInput)?,
                        thing: r
                            .thing
                            .as_deref()
                            .map(|v| number(v, "thing-"))
                            .transpose()?
                            .unwrap_or(0),
                        quantity_milli: quantity_milli(
                            r.quantity.as_deref().ok_or(Error::InvalidInput)?,
                        )?,
                        unit: r.unit.ok_or(Error::InvalidInput)?,
                    },
                }
            } else {
                Command::Transfer {
                    to: member_id(&r.to, "account-")?,
                    amount: r.amount,
                    memo: r.memo,
                }
            }
        }
        "/api/v1/admin/issue" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Issue {
                to: Id,
                amount: i64,
                #[serde(default)]
                reason: Memo,
            }
            let r: Issue = parse(body)?;
            Command::Issue {
                to: member_id(&r.to, "account-")?,
                amount: r.amount,
                memo: r.reason,
            }
        }
        "/api/v1/admin/retire" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Retire {
                from: Id,
                amount: i64,
                #[serde(default)]
                reason: Memo,
            }
            let r: Retire = parse(body)?;
            Command::Retire {
                from: member_id(&r.from, "account-")?,
                amount: r.amount,
                memo: r.reason,
            }
        }
        "/api/v1/listings" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Create {
                title: Title,
                #[serde(default)]
                description: Memo,
                price: i64,
                side: Option<String<4>>,
                kind: Option<String<16>>,
                currency: Option<String<8>>,
                minor_units: Option<i64>,
                economic_kind: Option<EconomicKind>,
                thing: Option<Id>,
                quantity: Option<String<24>>,
                unit: Option<Unit>,
                #[serde(default)]
                standard: bool,
            }
            let r: Create = parse(body)?;
            // Currency listings describe external settlement; forex quotes move USD wallets.
            let side = match r.side.as_deref() {
                None | Some("SELL") => Side::Sell,
                Some("BUY") => Side::Buy,
                _ => return Err(Error::InvalidInput),
            };
            let command = if r.economic_kind.is_some()
                || r.thing.is_some()
                || r.quantity.is_some()
                || r.unit.is_some()
                || r.standard
            {
                Command::ClassifiedList {
                    title: r.title,
                    description: r.description,
                    price: r.price,
                    side,
                    economic: EconomicDetails {
                        kind: r.economic_kind.ok_or(Error::InvalidInput)?,
                        thing: r
                            .thing
                            .as_deref()
                            .map(|v| number(v, "thing-"))
                            .transpose()?
                            .unwrap_or(0),
                        quantity_milli: quantity_milli(
                            r.quantity.as_deref().ok_or(Error::InvalidInput)?,
                        )?,
                        unit: r.unit.ok_or(Error::InvalidInput)?,
                    },
                    standard: r.standard,
                }
            } else {
                Command::List {
                    title: r.title,
                    description: r.description,
                    price: r.price,
                    side,
                    details: Some(ListingDetails {
                        kind: r.kind.unwrap_or_default(),
                        currency: r.currency.unwrap_or_default(),
                        minor_units: r.minor_units.unwrap_or_default(),
                    }),
                }
            };
            let receipt = s.execute(actor, s.state.member(actor)?.last_request + 1, command)?;
            let l = s
                .state
                .listings
                .iter()
                .find(|l| l.id == receipt.sequence)
                .ok_or(Error::NotFound)?;
            return serialize(&listing(&s.state, l), output);
        }
        _ => {
            if let Some(tx) = path
                .strip_prefix("/api/v1/transactions/")
                .and_then(|p| p.strip_suffix("/reverse"))
            {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Reverse {
                    #[serde(default)]
                    reason: Memo,
                }
                let r: Reverse = parse(body)?;
                Command::Reverse {
                    transaction: number(tx, "tx-")?,
                    memo: r.reason,
                }
            } else if let Some(l) = path
                .strip_prefix("/api/v1/listings/")
                .and_then(|p| p.strip_suffix("/purchase"))
            {
                Command::Buy {
                    listing: number(l, "listing-")?,
                }
            } else if let Some(l) = path
                .strip_prefix("/api/v1/listings/")
                .and_then(|p| p.strip_suffix("/cancel"))
            {
                let listing_id = number(l, "listing-")?;
                s.execute(
                    actor,
                    s.state.member(actor)?.last_request + 1,
                    Command::Cancel {
                        listing: listing_id,
                    },
                )?;
                return serialize(
                    &listing(
                        &s.state,
                        s.state
                            .listings
                            .iter()
                            .find(|l| l.id == listing_id)
                            .ok_or(Error::NotFound)?,
                    ),
                    output,
                );
            } else {
                return Err(Error::NotFound);
            }
        }
    };
    let receipt = s.execute_keyed(actor, key, command)?;
    let tx = s
        .state
        .history
        .iter()
        .find(|t| t.id == receipt.sequence)
        .ok_or(Error::StaleRequest)?;
    if let Some(listing_id) = tx.listing.filter(|_| tx.reverses.is_none()) {
        #[derive(Serialize)]
        struct Purchase<'a> {
            listing: ListingView<'a>,
            transaction: TransactionView<'a>,
        }
        let l = s
            .state
            .listings
            .iter()
            .find(|l| l.id == listing_id)
            .ok_or(Error::StaleRequest)?;
        serialize(
            &Purchase {
                listing: listing(&s.state, l),
                transaction: transaction(&s.state, tx),
            },
            output,
        )
    } else {
        transaction_response(&s.state, receipt.sequence, output)
    }
}

/// Shared immutable query used by the listings route and read-only benchmarks.
pub(crate) fn listings_query(
    state: &State,
    wanted: Option<&str>,
    output: &mut [u8],
) -> Result<usize, Error> {
    #[derive(Serialize)]
    struct Response<T> {
        listings: T,
    }
    let mut sorted: heapless::Vec<&Listing, LISTINGS> = state.listings.iter().collect();
    sorted.sort_unstable_by_key(|l| (core::cmp::Reverse(l.created_at), l.id));
    serialize(
        &Response {
            listings: Rows(
                sorted
                    .iter()
                    .map(|l| listing(state, l))
                    .filter(|l| wanted.is_none_or(|w| l.status == w)),
            ),
        },
        output,
    )
}

/** The household notebook is public by design. Authentication still protects
 * writes, account-scoped endpoints, and administration, but not audit data. */
pub(crate) fn public_ledger(
    state: &State,
    limit: usize,
    output: &mut [u8],
) -> Result<usize, Error> {
    #[derive(Serialize)]
    struct Response<T> {
        transactions: T,
        circulation: i64,
    }
    serialize(
        &Response {
            transactions: Rows(
                state
                    .history
                    .iter()
                    .rev()
                    .filter(|t| t.amount != 0)
                    .take(limit)
                    .map(|t| transaction(state, t)),
            ),
            circulation: -state.issuance_balance,
        },
        output,
    )
}

pub(crate) fn public_transaction(
    state: &State,
    id: &str,
    output: &mut [u8],
) -> Result<usize, Error> {
    let sequence = number(id, "tx-")?;
    if !state
        .history
        .iter()
        .any(|t| t.id == sequence && t.amount != 0)
    {
        return Err(Error::NotFound);
    }
    transaction_response(state, sequence, output)
}
fn transaction_response(state: &State, sequence: u64, output: &mut [u8]) -> Result<usize, Error> {
    serialize(
        &transaction(
            state,
            state
                .history
                .iter()
                .find(|t| t.id == sequence)
                .ok_or(Error::NotFound)?,
        ),
        output,
    )
}

fn currency_account(member: MemberId, usd: bool) -> Id {
    if !usd {
        return account(member);
    }
    if member == MemberId(0) {
        Id::try_from("account:usd-issuance").unwrap()
    } else {
        let mut out = account(member);
        out.push_str("-usd").unwrap();
        out
    }
}
fn parse_account(value: &str) -> Result<(MemberId, bool), Error> {
    if value == "account:system-issuance" {
        return Ok((MemberId(0), false));
    }
    if value == "account:usd-issuance" {
        return Ok((MemberId(0), true));
    }
    let (value, usd) = value
        .strip_suffix("-usd")
        .map(|v| (v, true))
        .unwrap_or((value, false));
    Ok((member_id(value, "account-")?, usd))
}
fn account_balance(state: &State, member: MemberId, usd: bool) -> Result<i64, Error> {
    if member == MemberId(0) {
        Ok(if usd {
            state.usd_issuance_balance
        } else {
            state.issuance_balance
        })
    } else {
        let m = state.member(member)?;
        Ok(if usd { m.usd_cents } else { m.balance })
    }
}

pub(crate) fn ledger_page<J: Journal>(
    s: &mut Service<J>,
    query: &str,
    output: &mut [u8],
) -> Result<usize, Error> {
    history::page(s, query, None, None, output)
}
