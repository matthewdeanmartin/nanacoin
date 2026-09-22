use super::*;
use crate::lotto::{Lotto, LottoTerms};
#[derive(Serialize)]
struct View<'a> {
    id: u64,
    terms: &'a LottoTerms,
    house: Id,
    pool: i64,
    interest: i64,
    tickets: u64,
    my_tickets: u32,
    winner: Option<Id>,
    winner_name: Option<&'a str>,
    due_at: u64,
    status: &'static str,
}
fn view<'a>(s: &'a State, l: &'a Lotto, actor: MemberId, now: u64) -> View<'a> {
    View {
        id: l.id,
        terms: &l.terms,
        house: account(l.house),
        pool: l.pool,
        interest: l.interest,
        tickets: l.total_tickets(),
        my_tickets: l.tickets[actor.0 as usize - 1],
        winner: l.winner.map(account),
        winner_name: l
            .winner
            .and_then(|id| s.member(id).ok())
            .map(|m| m.name.as_str()),
        due_at: l.due_at(),
        status: if l.step == 19 {
            "SETTLED"
        } else if l.step > 0 {
            "PAYING"
        } else if now >= l.terms.closes_at {
            "WAITING"
        } else {
            "OPEN"
        },
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
    if path != "/api/v1/lottos" && !path.starts_with("/api/v1/lottos/") {
        return None;
    }
    Some((|| {
        if path == "/api/v1/lottos" && method == "GET" {
            #[derive(Serialize)]
            struct Book<T> {
                lottos: T,
                decimals: u8,
                money_epoch: u64,
            }
            return serialize(
                &Book {
                    lottos: Rows(
                        s.state
                            .lottos
                            .iter()
                            .rev()
                            .map(|l| view(&s.state, l, actor, s.now())),
                    ),
                    decimals: s.state.decimals,
                    money_epoch: s.state.money_epoch,
                },
                output,
            );
        }
        if method != "POST" {
            return Err(Error::NotFound);
        }
        let (command, existing) = if path == "/api/v1/lottos" {
            (
                Command::CreateLotto {
                    terms: parse(body)?,
                },
                None,
            )
        } else {
            let tail = path.strip_prefix("/api/v1/lottos/").unwrap();
            let id = tail
                .strip_suffix("/tickets")
                .ok_or(Error::NotFound)?
                .parse::<u64>()
                .map_err(|_| Error::InvalidInput)?;
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Buy {
                count: u32,
            }
            (
                Command::BuyTickets {
                    lotto: id,
                    count: parse::<Buy>(body)?.count,
                },
                Some(id),
            )
        };
        let receipt = s.execute_keyed(actor, key, command)?;
        serialize(
            &view(
                &s.state,
                s.state.lotto(existing.unwrap_or(receipt.sequence))?,
                actor,
                s.now(),
            ),
            output,
        )
    })())
}
