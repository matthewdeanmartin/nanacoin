// Package core is NanaCoin's application layer: the one place where a request
// becomes a durable change.
//
// Every mutation follows the same shape:
//
//	validate against the RAM model
//	encode an event
//	append it to the journal   <- the durability point
//	apply it to the RAM model
//
// The order matters. Applying first and journalling second would let a flash
// failure leave a balance in RAM that no record supports, and the next reboot
// would quietly undo a transfer the user was told had succeeded. Journalling
// first means the worst case is a transfer that was recorded and reported as
// failed, which Nana can see in the ledger and resolve.
package core

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Event payloads. These are the durable schema: changing a field here changes
// what a ten-year-old journal means, so fields are added, never repurposed.
//
// JSON rather than a packed binary encoding. At ten events a week the size
// difference is irrelevant, and being able to read a household's ledger with
// `strings` on a dead board is worth more than the bytes.

type userCreatedEvent struct {
	User    users.User    `json:"user"`
	Account users.Account `json:"account"`
}

type userUpdatedEvent struct {
	ID          ledger.UserID `json:"id"`
	DisplayName *string       `json:"display_name,omitempty"`
	Status      *users.Status `json:"status,omitempty"`
	Role        *users.Role   `json:"role,omitempty"`
	Verifier    *string       `json:"verifier,omitempty"`
}

type transactionEvent struct {
	Txn ledger.Transaction `json:"txn"`
}

type listingCreatedEvent struct {
	Listing marketplace.Listing `json:"listing"`
}

type listingUpdatedEvent struct {
	ID          ledger.ListingID    `json:"id"`
	Title       *string             `json:"title,omitempty"`
	Description *string             `json:"description,omitempty"`
	Price       *ledger.Amount      `json:"price,omitempty"`
	Status      *marketplace.Status `json:"status,omitempty"`
	UpdatedAt   int64               `json:"updated_at"`
}

// listingPurchasedEvent carries both halves of a purchase - the money and the
// listing state change - in one record, because a purchase must be atomic
// (spec 13). Two records could half-land; one cannot.
type listingPurchasedEvent struct {
	ListingID ledger.ListingID   `json:"listing_id"`
	Buyer     ledger.AccountID   `json:"buyer"`
	Txn       ledger.Transaction `json:"txn"`
	UpdatedAt int64              `json:"updated_at"`
}

// offerCreatedEvent records a proposal. No money moves here.
type offerCreatedEvent struct {
	Offer marketplace.Offer `json:"offer"`
}

// offerAcceptedEvent carries both halves of an acceptance - the money and the
// offer and listing state changes - in one record, for the same reason a
// purchase does: two records could half-land, one cannot.
type offerAcceptedEvent struct {
	OfferID   ledger.OfferID     `json:"offer_id"`
	ListingID ledger.ListingID   `json:"listing_id"`
	Buyer     ledger.AccountID   `json:"buyer"`
	Txn       ledger.Transaction `json:"txn"`
	SettlesAt int64              `json:"settles_at"`
	UpdatedAt int64              `json:"updated_at"`
}

// offerUpdatedEvent is every other state change: declined, withdrawn,
// settled, or an acceptance reversed inside the window.
type offerUpdatedEvent struct {
	ID        ledger.OfferID          `json:"id"`
	Status    marketplace.OfferStatus `json:"status"`
	UpdatedAt int64                   `json:"updated_at"`

	// Reopen puts the listing back on the market, which is what an undone
	// acceptance does: the deal fell through, so the thing is for sale again.
	Reopen bool `json:"reopen,omitempty"`
}

// quoteCreatedEvent records a standing rate. No money moves.
type quoteCreatedEvent struct {
	Quote marketplace.Quote `json:"quote"`
}

// quoteTakenEvent is the coin leg of a trade plus the quote closing.
//
// One record, so the coins moving and the quote being marked filled cannot
// half-land relative to each other. The cash leg is a second record - see
// TakeQuote for why, and what that costs.
type quoteTakenEvent struct {
	QuoteID   ledger.QuoteID     `json:"quote_id"`
	Taker     ledger.AccountID   `json:"taker"`
	CoinTxn   ledger.Transaction `json:"coin_txn"`
	UpdatedAt int64              `json:"updated_at"`
}

// quoteSettledEvent is the cash leg, written immediately after the coin leg.
type quoteSettledEvent struct {
	QuoteID   ledger.QuoteID     `json:"quote_id"`
	CashTxn   ledger.Transaction `json:"cash_txn"`
	UpdatedAt int64              `json:"updated_at"`
}

// quoteUpdatedEvent is cancellation or expiry.
type quoteUpdatedEvent struct {
	ID        ledger.QuoteID          `json:"id"`
	Status    marketplace.QuoteStatus `json:"status"`
	UpdatedAt int64                   `json:"updated_at"`
}

type configUpdatedEvent struct {
	Config Config `json:"config"`
}

// idempotencyEvent records that a key was used and what it produced. It is
// journalled so that a retry after reboot still returns the original result
// rather than moving money twice (spec 20).
type idempotencyEvent struct {
	Key      string        `json:"key"`
	UserID   ledger.UserID `json:"user_id"`
	Endpoint string        `json:"endpoint"`
	Result   []byte        `json:"result"`
	At       int64         `json:"at"`
}

// Config is household policy. It is journalled like everything else so that
// changing the initial grant is an auditable act, not an invisible one.
type Config struct {
	HouseholdName string        `json:"household_name"`
	InitialGrant  ledger.Amount `json:"initial_grant"`
	Currency      string        `json:"currency"` // display name, e.g. "NanaCoin"

	// OfferSettlesAfter is how long an accepted offer stays reversible, in
	// seconds. Zero means the default.
	//
	// The failure it exists for: a child accepts every offer in the house,
	// becomes rich, and delivers none of it. Acceptance has to move the money
	// immediately - holding funds would need a concept the ledger does not
	// have - so instead it stays undoable for a window afterwards.
	//
	// Configurable because households differ on how long "you said you would
	// do it" should hang over someone. Two days is long enough that a
	// Saturday promise can be judged on Sunday.
	OfferSettlesAfter int64 `json:"offer_settles_after"`
}

// DefaultOfferSettlement is the reversal window when none is configured.
const DefaultOfferSettlement int64 = 48 * 60 * 60

func DefaultConfig() Config {
	return Config{
		HouseholdName:     "Household",
		InitialGrant:      100,
		Currency:          "NanaCoin",
		OfferSettlesAfter: DefaultOfferSettlement,
	}
}

// SettlementWindow is the configured window, or the default when unset.
func (c Config) SettlementWindow() int64 {
	if c.OfferSettlesAfter > 0 {
		return c.OfferSettlesAfter
	}
	return DefaultOfferSettlement
}

var _ = storage.TypeUserCreated // keep the storage import meaningful to readers
