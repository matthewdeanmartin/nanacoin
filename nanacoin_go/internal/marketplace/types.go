// Package marketplace holds listings: things household members offer each
// other for NanaCoin.
package marketplace

import "github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"

type Status string

const (
	StatusActive    Status = "ACTIVE"
	StatusSold      Status = "SOLD"
	StatusCancelled Status = "CANCELLED"
)

// Listing is an offer. Quantity is present but fixed at 1 for v1; the field
// exists so that adding multi-quantity later changes this package only and
// never the ledger.
type Listing struct {
	ID          ledger.ListingID `json:"id"`
	Seller      ledger.AccountID `json:"seller"`
	Title       string           `json:"title"`
	Description string           `json:"description"`
	Price       ledger.Amount    `json:"price"`
	Quantity    uint32           `json:"quantity"`
	Status      Status           `json:"status"`
	CreatedAt   int64            `json:"created_at"`
	UpdatedAt   int64            `json:"updated_at"`

	// Buyer and SoldTx are set when the listing sells, linking the
	// marketplace object to the ledger transaction that paid for it.
	Buyer  ledger.AccountID     `json:"buyer,omitempty"`
	SoldTx ledger.TransactionID `json:"sold_tx,omitempty"`

	// Side says whether this is something for sale or a want-ad. Zero is
	// SELL, so listings written before two-way listings read correctly.
	Side Side `json:"side,omitempty"`

	// Kind and Currency describe an external-currency listing (spec 14).
	// NanaCoin records only that NanaCoin changed hands; whether the $5 was
	// actually handed over is between the household and Nana.
	Kind       string `json:"kind,omitempty"`        // "" or "currency"
	Currency   string `json:"currency,omitempty"`    // e.g. "USD"
	MinorUnits int64  `json:"minor_units,omitempty"` // e.g. 500 for $5.00
}

// Side says which way round a listing is.
//
// SELL is the original kind: someone offers a thing and wants coins for it.
// BUY is the reverse - "25 NanaCoin for peanut butter cookies" - where the
// poster has the money and wants the thing done.
//
// The zero value is SELL, so every listing written before two-way listings
// existed reads back as what it was.
type Side uint8

const (
	SideSell Side = 0
	SideBuy  Side = 1
)

func (s Side) String() string {
	if s == SideBuy {
		return "BUY"
	}
	return "SELL"
}

// ParseSide reads the wire form. Anything unrecognised is SELL, which is the
// safe direction: a malformed side must not turn a sale into a payout.
func ParseSide(s string) Side {
	if s == "BUY" {
		return SideBuy
	}
	return SideSell
}

// OfferStatus is where an offer has got to.
//
// OPEN until the listing's owner decides. ACCEPTED means the money has moved
// but the deal is not yet final - see Offer.SettlesAt. SETTLED is past the
// point of return. DECLINED and WITHDRAWN are the two ways it ends without a
// deal, by the owner and by the offerer respectively. REVERSED is an
// acceptance undone inside the settlement window.
type OfferStatus string

const (
	OfferOpen      OfferStatus = "OPEN"
	OfferAccepted  OfferStatus = "ACCEPTED"
	OfferSettled   OfferStatus = "SETTLED"
	OfferDeclined  OfferStatus = "DECLINED"
	OfferWithdrawn OfferStatus = "WITHDRAWN"
	OfferReversed  OfferStatus = "REVERSED"
)

// Offer is a proposal against a listing. It is not a deal until accepted, and
// not final until it settles.
//
// # Why acceptance is not the end
//
// The failure this exists for is an eight-year-old accepting every offer in
// the house, becoming rich, and delivering none of it. Acceptance moves the
// money immediately - anything else would need a held-funds concept the
// ledger does not have - but it stays reversible for a window afterwards, so
// the household has a way to undo a deal that was never honoured.
//
// The reversal is an ordinary ledger reversal: a mirror transaction, not an
// edit. What SettlesAt adds is a deadline after which even Nana stops being
// able to do it casually, so a settled deal is genuinely settled.
type Offer struct {
	ID      ledger.OfferID   `json:"id"`
	Listing ledger.ListingID `json:"listing"`

	// Offerer is the account proposing. Which way the coins move on
	// acceptance depends on the listing's Side, not on this.
	Offerer ledger.AccountID `json:"offerer"`

	Amount  ledger.Amount `json:"amount"`
	Message string        `json:"message"`

	Status    OfferStatus `json:"status"`
	CreatedAt int64       `json:"created_at"`
	UpdatedAt int64       `json:"updated_at"`

	// SettledTx names the transaction acceptance created, and SettlesAt is
	// when it stops being reversible. Both are zero until accepted.
	SettledTx ledger.TransactionID `json:"settled_tx,omitempty"`
	SettlesAt int64                `json:"settles_at,omitempty"`
}

// Settled reports whether the reversal window has closed.
func (o *Offer) Settled(now int64) bool {
	if o.Status == OfferSettled {
		return true
	}
	return o.Status == OfferAccepted && o.SettlesAt != 0 && now >= o.SettlesAt
}

// Reversible reports whether an acceptance can still be undone.
func (o *Offer) Reversible(now int64) bool {
	return o.Status == OfferAccepted && !o.Settled(now)
}
