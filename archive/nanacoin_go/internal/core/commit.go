package core

import (
	"fmt"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
)

// Receipt payload plus bounded identity fields. Allocated once, under mu.
const wireCapacity = MaxIdempotencyBytes + 256

func wireStrings(values ...string) int {
	n := 0
	for _, v := range values {
		if len(v) > 65535 {
			return wireCapacity + 1
		}
		n += 2 + len(v)
	}
	return n
}
func wireOptional(v *string) int {
	if v == nil {
		return 1
	}
	return 1 + wireStrings(*v)
}
func transactionWireSize(t *ledger.Transaction) int {
	n := 10 + wireStrings(string(t.ID), string(t.Kind), string(t.Actor), t.Description, t.Reference, string(t.Reverses))
	for i := range t.Postings {
		// 8 for the amount, 4 for the currency.
		n += 12 + wireStrings(string(t.Postings[i].Account))
	}
	return n
}
func eventWireSize(event any) int {
	switch e := event.(type) {
	case *userCreatedEvent:
		u, a := &e.User, &e.Account
		return 16 + wireStrings(string(u.ID), u.Username, u.DisplayName, string(u.Role), string(u.Status), string(u.Account), u.Verifier, string(a.ID), string(a.UserID), a.Name, string(a.Status))
	case *userUpdatedEvent:
		return wireStrings(string(e.ID)) + wireOptional(e.DisplayName) + wireOptional((*string)(e.Status)) + wireOptional((*string)(e.Role)) + wireOptional(e.Verifier)
	case *transactionEvent:
		return transactionWireSize(&e.Txn)
	case *listingCreatedEvent:
		l := &e.Listing
		// 40, not 36: Side is a trailing u32 on top of the four i64s and
		// the u32 quantity.
		return 40 + wireStrings(string(l.ID), string(l.Seller), l.Title, l.Description, string(l.Status), string(l.Buyer), string(l.SoldTx), l.Kind, l.Currency)
	case *listingUpdatedEvent:
		n := 9 + wireStrings(string(e.ID)) + wireOptional(e.Title) + wireOptional(e.Description) + wireOptional((*string)(e.Status))
		if e.Price != nil {
			n += 8
		}
		return n
	case *listingPurchasedEvent:
		return 8 + wireStrings(string(e.ListingID), string(e.Buyer)) + transactionWireSize(&e.Txn)
	case *offerCreatedEvent:
		// Four i64s: amount, created, updated, settles-at.
		return 32 + wireStrings(
			string(e.Offer.ID), string(e.Offer.Listing), string(e.Offer.Offerer),
			e.Offer.Message, string(e.Offer.Status), string(e.Offer.SettledTx))
	case *offerAcceptedEvent:
		// Two i64s: settles-at and updated-at.
		return 16 + wireStrings(
			string(e.OfferID), string(e.ListingID), string(e.Buyer)) +
			transactionWireSize(&e.Txn)
	case *offerUpdatedEvent:
		// One i64 for updated-at, plus the u32 reopen flag.
		return 12 + wireStrings(string(e.ID), string(e.Status))
	case *quoteCreatedEvent:
		q := &e.Quote
		// Five i64s - rate, coins, created, updated, expires - and a u32 side.
		return 44 + wireStrings(string(q.ID), string(q.Maker), string(q.Status),
			string(q.Taker), string(q.CoinTx), string(q.CashTx))
	case *quoteTakenEvent:
		return 8 + wireStrings(string(e.QuoteID), string(e.Taker)) +
			transactionWireSize(&e.CoinTxn)
	case *quoteSettledEvent:
		return 8 + wireStrings(string(e.QuoteID)) + transactionWireSize(&e.CashTxn)
	case *quoteUpdatedEvent:
		return 8 + wireStrings(string(e.ID), string(e.Status))
	case *configUpdatedEvent:
		// 16, not 8: InitialGrant and OfferSettlesAfter are both i64.
		return 16 + wireStrings(e.Config.HouseholdName, e.Config.Currency)
	case *idempotencyEvent:
		return 10 + wireStrings(e.Key, string(e.UserID), e.Endpoint) + len(e.Result)
	default:
		return wireCapacity + 1
	}
}

// Append remains before application. Live events no longer allocate a second
// decoded object graph; replay uses exactly the same applyEvent transition.
func (s *Service) commitEvent(typ storage.RecordType, event any) error {
	n := eventWireSize(event)
	if n > wireCapacity {
		return fmt.Errorf("%w: journal event needs %d bytes (capacity %d)", ErrCapacity, n, wireCapacity)
	}
	if s.discard != nil {
		if _, err := s.discard.AppendDiscarded(typ, n); err != nil {
			return fmt.Errorf("journal append failed: %w", err)
		}
		return s.applyEvent(event)
	}
	b := s.wireBuf[:0]
	switch e := event.(type) {
	case *userCreatedEvent:
		b = encodeUserCreated(b, e)
	case *userUpdatedEvent:
		b = encodeUserUpdated(b, e)
	case *transactionEvent:
		b = encodeTransactionEvent(b, e)
	case *listingCreatedEvent:
		b = encodeListingCreated(b, e)
	case *listingUpdatedEvent:
		b = encodeListingUpdated(b, e)
	case *listingPurchasedEvent:
		b = encodeListingPurchased(b, e)
	case *offerCreatedEvent:
		b = encodeOfferCreated(b, e)
	case *offerAcceptedEvent:
		b = encodeOfferAccepted(b, e)
	case *offerUpdatedEvent:
		b = encodeOfferUpdated(b, e)
	case *quoteCreatedEvent:
		b = encodeQuoteCreated(b, e)
	case *quoteTakenEvent:
		b = encodeQuoteTaken(b, e)
	case *quoteSettledEvent:
		b = encodeQuoteSettled(b, e)
	case *quoteUpdatedEvent:
		b = encodeQuoteUpdated(b, e)
	case *configUpdatedEvent:
		b = encodeConfigUpdated(b, e)
	case *idempotencyEvent:
		b = encodeIdempotency(b, e)
	default:
		return ErrBadInput
	}
	if len(b) != n {
		panic("nanacoin: journal size calculation disagrees with codec")
	}
	if _, err := s.journal.Append(typ, b); err != nil {
		return fmt.Errorf("journal append failed: %w", err)
	}
	return s.applyEvent(event)
}
