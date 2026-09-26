use super::*;
use crate::offers::{Offer, OfferId, OfferMessage, OfferPhase};

#[derive(Serialize)]
struct OfferView<'a> {
    id: Id,
    listing: Id,
    listing_title: &'a str,
    listing_owner: Id,
    listing_owner_name: &'a str,
    listing_side: &'static str,
    offerer: Id,
    offerer_name: &'a str,
    amount: i64,
    message: &'a str,
    status: &'static str,
    created_at: u64,
    updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    settled_tx: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settles_at: Option<u64>,
    reversible: bool,
}

fn view<'a>(state: &'a State, offer: &'a Offer, now: u64) -> OfferView<'a> {
    OfferView {
        id: id("offer-", offer.id.0),
        listing: id("listing-", offer.listing),
        listing_title: &offer.listing_title,
        listing_owner: account(offer.owner),
        listing_owner_name: &state.member(offer.owner).unwrap().name,
        listing_side: if offer.listing_side == Side::Buy {
            "BUY"
        } else {
            "SELL"
        },
        offerer: account(offer.offerer),
        offerer_name: state
            .member(offer.offerer)
            .map(|m| m.name.as_str())
            .unwrap_or(""),
        amount: offer.amount,
        message: &offer.message,
        status: if offer.phase == OfferPhase::Open
            && !state
                .listing(offer.listing)
                .is_ok_and(|listing| listing.status == ListingStatus::Active)
        {
            "NOT_SELECTED"
        } else {
            offer.status(now)
        },
        created_at: offer.created_at,
        updated_at: offer.updated_at,
        settled_tx: offer.settlement().map(|s| id("tx-", s.transaction)),
        settles_at: offer.settlement().map(|s| s.settles_at),
        reversible: offer.reversible(now),
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
    let make = path
        .strip_prefix("/api/v1/listings/")
        .and_then(|p| p.strip_suffix("/offers"));
    let target = path.strip_prefix("/api/v1/offers/");
    if path != "/api/v1/offers" && make.is_none() && target.is_none() {
        return None;
    }
    Some((|| {
        let now = s.now();
        if method == "GET" && path == "/api/v1/offers" {
            #[derive(Serialize)]
            struct Page<T> {
                offers: T,
            }
            let member = s.state.member(actor)?;
            return serialize(
                &Page {
                    offers: Rows(
                        s.state
                            .offers
                            .iter()
                            .rev()
                            .filter(|o| o.visible_to(member))
                            .map(|o| view(&s.state, o, now)),
                    ),
                },
                output,
            );
        }
        if let Some(listing) = make {
            if method != "POST" {
                return Err(Error::NotFound);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Make {
                amount: i64,
                #[serde(default)]
                message: OfferMessage,
            }
            let r: Make = parse(body)?;
            let receipt = s.execute(
                actor,
                s.state.member(actor)?.last_request + 1,
                Command::MakeOffer {
                    listing: number(listing, "listing-")?,
                    amount: r.amount,
                    message: r.message,
                },
            )?;
            return serialize(
                &view(&s.state, s.state.offer(OfferId(receipt.sequence))?, s.now()),
                output,
            );
        }
        let target = target.ok_or(Error::NotFound)?;
        let (target, action) = target.split_once('/').unwrap_or((target, ""));
        let offer_id = OfferId(number(target, "offer-")?);
        let offer = s.state.offer(offer_id)?;
        // A private offer is indistinguishable from an absent one on reads.
        if method == "GET" && action.is_empty() {
            if !offer.visible_to(s.state.member(actor)?) {
                return Err(Error::NotFound);
            }
            return serialize(&view(&s.state, offer, now), output);
        }
        if method != "POST" {
            return Err(Error::NotFound);
        }
        let command = match action {
            "accept" => Command::AcceptOffer { offer: offer_id },
            "unaccept" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Undo {
                    #[serde(default)]
                    reason: TransactionMemo,
                }
                Command::UnacceptOffer {
                    offer: offer_id,
                    reason: parse::<Undo>(body)?.reason,
                }
            }
            "decline" => Command::DeclineOffer { offer: offer_id },
            "withdraw" => Command::WithdrawOffer { offer: offer_id },
            _ => return Err(Error::NotFound),
        };
        if action == "decline" || action == "withdraw" {
            s.execute(actor, s.state.member(actor)?.last_request + 1, command)?;
            return serialize(&view(&s.state, s.state.offer(offer_id)?, s.now()), output);
        }
        let receipt = s.execute_keyed(actor, key, command)?;
        // Reconstruct the original operation's phase, even after an undo. Never
        // pretend a retry accepted an offer again or extend its original deadline.
        let timestamp = s.event_timestamp(receipt.sequence)?;
        let mut offer = s.state.offer(offer_id)?.clone();
        let settlement = offer.settlement().ok_or(Error::StaleRequest)?;
        offer.phase = if action == "accept" {
            OfferPhase::Accepted(settlement)
        } else {
            OfferPhase::Reversed(settlement)
        };
        offer.updated_at = timestamp;
        let tx = s
            .state
            .history
            .iter()
            .find(|t| t.id == receipt.sequence)
            .ok_or(Error::StaleRequest)?;
        let mut transaction = transaction(&s.state, tx);
        transaction.reversed_by = None;
        transaction.refunded = 0;
        // Acceptance retries describe the payment at acceptance; fetch live fulfillment separately.
        transaction.fulfillment = None;
        transaction.created_at = timestamp;
        #[derive(Serialize)]
        struct ResultView<'a> {
            offer: OfferView<'a>,
            transaction: TransactionView<'a>,
        }
        serialize(
            &ResultView {
                offer: view(&s.state, &offer, timestamp),
                transaction,
            },
            output,
        )
    })())
}
