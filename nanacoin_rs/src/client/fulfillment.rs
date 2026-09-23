use super::*;
use crate::fulfillment::{Action, Fulfillment, Kind, Status};
#[derive(Serialize)]
pub(super) struct View<'a> {
    transaction: Id,
    provider: Id,
    recipient: Id,
    provider_name: &'a str,
    recipient_name: &'a str,
    description: &'a str,
    kind: Kind,
    status: Status,
    updates: Updates<'a>,
}
#[derive(Serialize)]
struct UpdateView<'a> {
    id: Id,
    at: u64,
    actor: Id,
    actor_name: &'a str,
    status: Status,
    reason: &'a str,
}
pub(super) fn view<'a>(s: &'a State, f: &'a Fulfillment) -> View<'a> {
    View {
        transaction: id("tx-", f.transaction),
        provider: account(f.provider),
        recipient: account(f.recipient),
        provider_name: &s.member(f.provider).unwrap().name,
        recipient_name: &s.member(f.recipient).unwrap().name,
        description: &f.description,
        kind: f.kind,
        status: f.status,
        updates: Updates(s, f),
    }
}
struct Updates<'a>(&'a State, &'a Fulfillment);
impl Serialize for Updates<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Rows(self.1.updates.iter().map(|u| UpdateView {
            id: id("event-", u.sequence),
            at: u.at,
            actor: account(u.actor),
            actor_name: &self.0.member(u.actor).unwrap().name,
            status: u.status,
            reason: &u.reason,
        }))
        .serialize(serializer)
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
    if method == "GET" && path == "/api/v1/fulfillments" {
        return Some((|| {
            let nana = s.state.member(actor)?.role == Role::Nana;
            #[derive(Serialize)]
            struct Book<T: Serialize> {
                fulfillments: T,
            }
            serialize(
                &Book {
                    fulfillments: Rows(
                        s.state
                            .fulfillments
                            .iter()
                            .filter(|f| nana || f.provider == actor || f.recipient == actor)
                            .map(|f| view(&s.state, f)),
                    ),
                },
                output,
            )
        })());
    }
    let tx = path
        .strip_prefix("/api/v1/transactions/")?
        .strip_suffix("/fulfillment")?;
    Some((|| {
        if method != "POST" {
            return Err(Error::NotFound);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            action: Action,
            #[serde(default)]
            reason: Memo,
        }
        let request: Request = parse(body)?;
        let transaction = number(tx, "tx-")?;
        s.execute_keyed(
            actor,
            key,
            Command::SetFulfillment {
                transaction,
                action: request.action,
                reason: request.reason,
            },
        )?;
        let f = s
            .state
            .fulfillments
            .iter()
            .find(|f| f.transaction == transaction)
            .ok_or(Error::StaleRequest)?;
        serialize(&view(&s.state, f), output)
    })())
}
