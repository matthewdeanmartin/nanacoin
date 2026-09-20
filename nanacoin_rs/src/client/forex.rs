use super::*;
use crate::forex::{Quote, QuoteSide, QuoteStatus};

#[derive(Serialize)]
struct QuoteView<'a> {
    id: Id,
    maker: Id,
    maker_name: &'a str,
    side: QuoteSide,
    cents_per_coin: i64,
    coins: i64,
    cents: i64,
    status: &'static str,
    created_at: u64,
    updated_at: u64,
    expires_at: u64,
    live: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    taker: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    taker_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coin_tx: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cash_tx: Option<Id>,
}
fn view<'a>(state: &'a State, q: &'a Quote, now: u64) -> QuoteView<'a> {
    QuoteView {
        id: id("quote-", q.id),
        maker: account(q.maker),
        maker_name: state.member(q.maker).unwrap().name.as_str(),
        side: q.side,
        cents_per_coin: q.cents_per_coin,
        coins: q.coins,
        cents: q.cents(),
        status: match q.status {
            QuoteStatus::Filled => "FILLED",
            QuoteStatus::Cancelled => "CANCELLED",
            QuoteStatus::Open if now != 0 && q.expires_at != 0 && now >= q.expires_at => "EXPIRED",
            QuoteStatus::Open => "OPEN",
        },
        created_at: q.created_at,
        updated_at: q.updated_at,
        expires_at: q.expires_at,
        live: q.live(now),
        taker: q.taker.map(account),
        taker_name: q
            .taker
            .and_then(|m| state.member(m).ok())
            .map(|m| m.name.as_str()),
        coin_tx: q.coin_tx.map(|n| id("tx-", n)),
        cash_tx: q.cash_tx.map(|n| id("tx-", n)),
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
    let target = path.strip_prefix("/api/v1/quotes/");
    if path != "/api/v1/quotes" && path != "/api/v1/admin/issue-usd" && target.is_none() {
        return None;
    }
    Some((|| {
        if path == "/api/v1/admin/issue-usd" {
            if method != "POST" {
                return Err(Error::NotFound);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Issue {
                to: Id,
                cents: i64,
                #[serde(default)]
                reason: Memo,
            }
            let r: Issue = parse(body)?;
            let receipt = s.execute_keyed(
                actor,
                key,
                Command::IssueUsd {
                    to: member_id(&r.to, "account-")?,
                    cents: r.cents,
                    memo: r.reason,
                },
            )?;
            if !s.state.history.iter().any(|t| t.id == receipt.sequence) {
                return Err(Error::StaleRequest);
            }
            return transaction_response(&s.state, receipt.sequence, output);
        }
        if path == "/api/v1/quotes" {
            if method == "GET" {
                // Sort a bounded array of references; reading the book allocates nothing.
                let mut sorted: heapless::Vec<&Quote, { crate::forex::QUOTES }> =
                    s.state.quotes.iter().collect();
                sorted.sort_unstable_by_key(|q| {
                    (
                        if q.side == QuoteSide::ASK { 0 } else { 1 },
                        if q.side == QuoteSide::ASK {
                            q.cents_per_coin
                        } else {
                            -q.cents_per_coin
                        },
                        q.id,
                    )
                });
                #[derive(Serialize)]
                struct Page<T> {
                    quotes: T,
                }
                return serialize(
                    &Page {
                        quotes: Rows(sorted.iter().map(|q| view(&s.state, q, s.now()))),
                    },
                    output,
                );
            }
            if method != "POST" {
                return Err(Error::NotFound);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Post {
                side: QuoteSide,
                cents_per_coin: i64,
                coins: i64,
                #[serde(default)]
                expires_at: u64,
            }
            let r: Post = parse(body)?;
            let receipt = s.execute(
                actor,
                s.state.member(actor)?.last_request + 1,
                Command::PostQuote {
                    side: r.side,
                    cents_per_coin: r.cents_per_coin,
                    coins: r.coins,
                    expires_at: r.expires_at,
                },
            )?;
            return serialize(
                &view(&s.state, s.state.quote(receipt.sequence)?, s.now()),
                output,
            );
        }
        let (target, action) = target
            .unwrap()
            .split_once('/')
            .unwrap_or((target.unwrap(), ""));
        let quote_id = number(target, "quote-")?;
        if method == "GET" && action.is_empty() {
            return serialize(&view(&s.state, s.state.quote(quote_id)?, s.now()), output);
        }
        if method != "POST" {
            return Err(Error::NotFound);
        }
        if action == "cancel" {
            s.execute(
                actor,
                s.state.member(actor)?.last_request + 1,
                Command::CancelQuote { quote: quote_id },
            )?;
            return serialize(&view(&s.state, s.state.quote(quote_id)?, s.now()), output);
        }
        if action != "take" {
            return Err(Error::NotFound);
        }
        let receipt = s.execute_keyed(actor, key, Command::TakeQuote { quote: quote_id })?;
        let q = s.state.quote(quote_id).map_err(|_| Error::StaleRequest)?;
        let coin = s
            .state
            .history
            .iter()
            .find(|t| t.id == receipt.sequence)
            .ok_or(Error::StaleRequest)?;
        let cash = s
            .state
            .history
            .iter()
            .find(|t| Some(t.id) == q.cash_tx)
            .ok_or(Error::StaleRequest)?;
        #[derive(Serialize)]
        struct Trade<'a> {
            quote: QuoteView<'a>,
            coin_transaction: TransactionView<'a>,
            cash_transaction: TransactionView<'a>,
        }
        serialize(
            &Trade {
                quote: view(&s.state, q, s.now()),
                coin_transaction: transaction(&s.state, coin),
                cash_transaction: transaction(&s.state, cash),
            },
            output,
        )
    })())
}
