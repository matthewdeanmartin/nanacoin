package core

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Binary codecs for every journal event.
//
// One encode and one decode per event type, in a fixed field order. The order
// is the schema; see wire.go for the primitives and for why this replaced
// JSON.
//
// # Keeping the pair in step
//
// An encoder and decoder that disagree would corrupt the journal silently -
// the CRC would pass, because the bytes are exactly what the encoder wrote.
// So every type is round-tripped in eventswire_test.go with fuzzed-ish
// values, and the test fails on a desktop if a field is added to one side
// only. That test is what makes hand-writing these safe.

// --- user created -----------------------------------------------------------

func encodeUserCreated(buf []byte, e *userCreatedEvent) []byte {
	b := buf
	// User
	b = putStr(b, string(e.User.ID))
	b = putStr(b, e.User.Username)
	b = putStr(b, e.User.DisplayName)
	b = putStr(b, string(e.User.Role))
	b = putStr(b, string(e.User.Status))
	b = putStr(b, string(e.User.Account))
	b = putI64(b, e.User.CreatedAt)
	b = putStr(b, e.User.Verifier)
	// Account
	b = putStr(b, string(e.Account.ID))
	b = putStr(b, string(e.Account.UserID))
	b = putStr(b, e.Account.Name)
	b = putStr(b, string(e.Account.Status))
	b = putI64(b, e.Account.CreatedAt)
	return b
}

func decodeUserCreated(p []byte, e *userCreatedEvent) error {
	r := newReader(p)
	e.User.ID = ledger.UserID(r.str())
	e.User.Username = r.str()
	e.User.DisplayName = r.str()
	e.User.Role = users.Role(r.str())
	e.User.Status = users.Status(r.str())
	e.User.Account = ledger.AccountID(r.str())
	e.User.CreatedAt = r.i64()
	e.User.Verifier = r.str()
	e.Account.ID = ledger.AccountID(r.str())
	e.Account.UserID = ledger.UserID(r.str())
	e.Account.Name = r.str()
	e.Account.Status = users.Status(r.str())
	e.Account.CreatedAt = r.i64()
	return r.done()
}

// --- user updated -----------------------------------------------------------

func encodeUserUpdated(buf []byte, e *userUpdatedEvent) []byte {
	b := buf
	b = putStr(b, string(e.ID))
	b = putOptStr(b, e.DisplayName)
	b = putOptStr(b, (*string)(e.Status))
	b = putOptStr(b, (*string)(e.Role))
	b = putOptStr(b, e.Verifier)
	return b
}

func decodeUserUpdated(p []byte, e *userUpdatedEvent) error {
	r := newReader(p)
	e.ID = ledger.UserID(r.str())
	e.DisplayName = r.optStr()
	e.Status = (*users.Status)(r.optStr())
	e.Role = (*users.Role)(r.optStr())
	e.Verifier = r.optStr()
	return r.done()
}

// --- transaction ------------------------------------------------------------
//
// The postings are length-prefixed rather than fixed at MaxInlinePostings,
// so a future transaction with three postings needs no format change - only
// the RAM representation caps them at two.

func encodeTransaction(b []byte, t *ledger.Transaction) []byte {
	b = putStr(b, string(t.ID))
	b = putStr(b, string(t.Kind))
	b = putI64(b, t.CreatedAt)
	b = putStr(b, string(t.Actor))
	b = putStr(b, t.Description)
	b = putStr(b, t.Reference)
	b = putStr(b, string(t.Reverses))
	b = putU16(b, uint16(len(t.Postings)))
	for i := range t.Postings {
		b = putStr(b, string(t.Postings[i].Account))
		b = putI64(b, int64(t.Postings[i].Amount))
		// Appended per posting. Without it a USD leg replays as NanaCoin and
		// the household's money is silently rewritten at the next reboot -
		// the worst kind of bug this format can have, because nothing fails.
		b = putU32(b, uint32(t.Postings[i].Currency))
	}
	return b
}

func decodeTransaction(r *reader, t *ledger.Transaction) {
	t.ID = ledger.TransactionID(r.str())
	t.Kind = ledger.TransactionKind(r.str())
	t.CreatedAt = r.i64()
	t.Actor = ledger.UserID(r.str())
	t.Description = r.str()
	t.Reference = r.str()
	t.Reverses = ledger.TransactionID(r.str())

	n := int(r.u16())
	if r.err != nil {
		return
	}
	// Bounded before allocating: a corrupt length must not make replay try to
	// allocate 65535 postings on a board with 85 kB of heap.
	if n > 64 {
		r.err = errWireTooLong
		return
	}
	if n == 0 {
		t.Postings = nil
		return
	}
	t.Postings = make([]ledger.Posting, n)
	for i := 0; i < n; i++ {
		t.Postings[i].Account = ledger.AccountID(r.str())
		t.Postings[i].Amount = ledger.Amount(r.i64())
		// A record written before currencies existed has no more bytes here;
		// the reader yields zero, which is NanaCoin - exactly what those
		// postings were.
		t.Postings[i].Currency = ledger.Currency(r.u32())
	}
}

func encodeTransactionEvent(buf []byte, e *transactionEvent) []byte {
	return encodeTransaction(buf, &e.Txn)
}

func decodeTransactionEvent(p []byte, e *transactionEvent) error {
	r := newReader(p)
	decodeTransaction(r, &e.Txn)
	return r.done()
}

// --- listing ----------------------------------------------------------------

func encodeListing(b []byte, l *marketplace.Listing) []byte {
	b = putStr(b, string(l.ID))
	b = putStr(b, string(l.Seller))
	b = putStr(b, l.Title)
	b = putStr(b, l.Description)
	b = putI64(b, int64(l.Price))
	b = putU32(b, l.Quantity)
	b = putStr(b, string(l.Status))
	b = putI64(b, l.CreatedAt)
	b = putI64(b, l.UpdatedAt)
	b = putStr(b, string(l.Buyer))
	b = putStr(b, string(l.SoldTx))
	b = putStr(b, l.Kind)
	b = putStr(b, l.Currency)
	b = putI64(b, l.MinorUnits)
	// Appended last on purpose: a listing written before two-way listings
	// existed has no side field, and a reader that runs out of bytes leaves
	// it zero - which is SELL, what those listings were.
	b = putU32(b, uint32(l.Side))
	return b
}

func decodeListing(r *reader, l *marketplace.Listing) {
	l.ID = ledger.ListingID(r.str())
	l.Seller = ledger.AccountID(r.str())
	l.Title = r.str()
	l.Description = r.str()
	l.Price = ledger.Amount(r.i64())
	l.Quantity = r.u32()
	l.Status = marketplace.Status(r.str())
	l.CreatedAt = r.i64()
	l.UpdatedAt = r.i64()
	l.Buyer = ledger.AccountID(r.str())
	l.SoldTx = ledger.TransactionID(r.str())
	l.Kind = r.str()
	l.Currency = r.str()
	l.MinorUnits = r.i64()
	l.Side = marketplace.Side(r.u32())
}

func encodeListingCreated(buf []byte, e *listingCreatedEvent) []byte {
	return encodeListing(buf, &e.Listing)
}

func decodeListingCreated(p []byte, e *listingCreatedEvent) error {
	r := newReader(p)
	decodeListing(r, &e.Listing)
	return r.done()
}

// --- listing updated --------------------------------------------------------

func encodeListingUpdated(buf []byte, e *listingUpdatedEvent) []byte {
	b := buf
	b = putStr(b, string(e.ID))
	b = putOptStr(b, e.Title)
	b = putOptStr(b, e.Description)
	b = putOptI64(b, (*int64)(e.Price))
	b = putOptStr(b, (*string)(e.Status))
	b = putI64(b, e.UpdatedAt)
	return b
}

func decodeListingUpdated(p []byte, e *listingUpdatedEvent) error {
	r := newReader(p)
	e.ID = ledger.ListingID(r.str())
	e.Title = r.optStr()
	e.Description = r.optStr()
	e.Price = (*ledger.Amount)(r.optI64())
	e.Status = (*marketplace.Status)(r.optStr())
	e.UpdatedAt = r.i64()
	return r.done()
}

// --- listing purchased ------------------------------------------------------

func encodeListingPurchased(buf []byte, e *listingPurchasedEvent) []byte {
	b := buf
	b = putStr(b, string(e.ListingID))
	b = putStr(b, string(e.Buyer))
	b = putI64(b, e.UpdatedAt)
	b = encodeTransaction(b, &e.Txn)
	return b
}

func decodeListingPurchased(p []byte, e *listingPurchasedEvent) error {
	r := newReader(p)
	e.ListingID = ledger.ListingID(r.str())
	e.Buyer = ledger.AccountID(r.str())
	e.UpdatedAt = r.i64()
	decodeTransaction(r, &e.Txn)
	return r.done()
}

// --- offers -----------------------------------------------------------------

func encodeOffer(b []byte, o *marketplace.Offer) []byte {
	b = putStr(b, string(o.ID))
	b = putStr(b, string(o.Listing))
	b = putStr(b, string(o.Offerer))
	b = putI64(b, int64(o.Amount))
	b = putStr(b, o.Message)
	b = putStr(b, string(o.Status))
	b = putI64(b, o.CreatedAt)
	b = putI64(b, o.UpdatedAt)
	b = putStr(b, string(o.SettledTx))
	b = putI64(b, o.SettlesAt)
	return b
}

func decodeOffer(r *reader, o *marketplace.Offer) {
	o.ID = ledger.OfferID(r.str())
	o.Listing = ledger.ListingID(r.str())
	o.Offerer = ledger.AccountID(r.str())
	o.Amount = ledger.Amount(r.i64())
	o.Message = r.str()
	o.Status = marketplace.OfferStatus(r.str())
	o.CreatedAt = r.i64()
	o.UpdatedAt = r.i64()
	o.SettledTx = ledger.TransactionID(r.str())
	o.SettlesAt = r.i64()
}

func encodeOfferCreated(buf []byte, e *offerCreatedEvent) []byte {
	return encodeOffer(buf, &e.Offer)
}

func decodeOfferCreated(p []byte, e *offerCreatedEvent) error {
	r := newReader(p)
	decodeOffer(r, &e.Offer)
	return r.done()
}

func encodeOfferAccepted(buf []byte, e *offerAcceptedEvent) []byte {
	b := buf
	b = putStr(b, string(e.OfferID))
	b = putStr(b, string(e.ListingID))
	b = putStr(b, string(e.Buyer))
	b = putI64(b, e.SettlesAt)
	b = putI64(b, e.UpdatedAt)
	b = encodeTransaction(b, &e.Txn)
	return b
}

func decodeOfferAccepted(p []byte, e *offerAcceptedEvent) error {
	r := newReader(p)
	e.OfferID = ledger.OfferID(r.str())
	e.ListingID = ledger.ListingID(r.str())
	e.Buyer = ledger.AccountID(r.str())
	e.SettlesAt = r.i64()
	e.UpdatedAt = r.i64()
	decodeTransaction(r, &e.Txn)
	return r.done()
}

func encodeOfferUpdated(buf []byte, e *offerUpdatedEvent) []byte {
	b := buf
	b = putStr(b, string(e.ID))
	b = putStr(b, string(e.Status))
	b = putI64(b, e.UpdatedAt)
	var reopen uint32
	if e.Reopen {
		reopen = 1
	}
	b = putU32(b, reopen)
	return b
}

func decodeOfferUpdated(p []byte, e *offerUpdatedEvent) error {
	r := newReader(p)
	e.ID = ledger.OfferID(r.str())
	e.Status = marketplace.OfferStatus(r.str())
	e.UpdatedAt = r.i64()
	e.Reopen = r.u32() == 1
	return r.done()
}

// --- config -----------------------------------------------------------------

func encodeConfigUpdated(buf []byte, e *configUpdatedEvent) []byte {
	b := buf
	b = putStr(b, e.Config.HouseholdName)
	b = putI64(b, int64(e.Config.InitialGrant))
	b = putStr(b, e.Config.Currency)
	b = putI64(b, e.Config.OfferSettlesAfter)
	return b
}

func decodeConfigUpdated(p []byte, e *configUpdatedEvent) error {
	r := newReader(p)
	e.Config.HouseholdName = r.str()
	e.Config.InitialGrant = ledger.Amount(r.i64())
	e.Config.Currency = r.str()
	e.Config.OfferSettlesAfter = r.i64()
	return r.done()
}

// --- idempotency ------------------------------------------------------------
//
// Result is the marshalled response body, which is opaque here: it was
// produced by the API layer and is handed back verbatim on a retry. Stored as
// length-prefixed bytes rather than parsed.

func encodeIdempotency(buf []byte, e *idempotencyEvent) []byte {
	b := buf
	b = putStr(b, e.Key)
	b = putStr(b, string(e.UserID))
	b = putStr(b, e.Endpoint)
	b = putBytes(b, e.Result)
	b = putI64(b, e.At)
	return b
}

func decodeIdempotency(p []byte, e *idempotencyEvent) error {
	r := newReader(p)
	e.Key = r.str()
	e.UserID = ledger.UserID(r.str())
	e.Endpoint = r.str()
	e.Result = r.bytes()
	e.At = r.i64()
	return r.done()
}

// --- quotes -----------------------------------------------------------------

func encodeQuote(b []byte, q *marketplace.Quote) []byte {
	b = putStr(b, string(q.ID))
	b = putStr(b, string(q.Maker))
	b = putU32(b, uint32(q.Side))
	b = putI64(b, int64(q.CentsPerCoin))
	b = putI64(b, int64(q.Coins))
	b = putStr(b, string(q.Status))
	b = putI64(b, q.CreatedAt)
	b = putI64(b, q.UpdatedAt)
	b = putI64(b, q.ExpiresAt)
	b = putStr(b, string(q.Taker))
	b = putStr(b, string(q.CoinTx))
	b = putStr(b, string(q.CashTx))
	return b
}

func decodeQuote(r *reader, q *marketplace.Quote) {
	q.ID = ledger.QuoteID(r.str())
	q.Maker = ledger.AccountID(r.str())
	q.Side = marketplace.QuoteSide(r.u32())
	q.CentsPerCoin = ledger.Amount(r.i64())
	q.Coins = ledger.Amount(r.i64())
	q.Status = marketplace.QuoteStatus(r.str())
	q.CreatedAt = r.i64()
	q.UpdatedAt = r.i64()
	q.ExpiresAt = r.i64()
	q.Taker = ledger.AccountID(r.str())
	q.CoinTx = ledger.TransactionID(r.str())
	q.CashTx = ledger.TransactionID(r.str())
}

func encodeQuoteCreated(buf []byte, e *quoteCreatedEvent) []byte {
	return encodeQuote(buf, &e.Quote)
}

func decodeQuoteCreated(p []byte, e *quoteCreatedEvent) error {
	r := newReader(p)
	decodeQuote(r, &e.Quote)
	return r.done()
}

func encodeQuoteTaken(buf []byte, e *quoteTakenEvent) []byte {
	b := buf
	b = putStr(b, string(e.QuoteID))
	b = putStr(b, string(e.Taker))
	b = putI64(b, e.UpdatedAt)
	b = encodeTransaction(b, &e.CoinTxn)
	return b
}

func decodeQuoteTaken(p []byte, e *quoteTakenEvent) error {
	r := newReader(p)
	e.QuoteID = ledger.QuoteID(r.str())
	e.Taker = ledger.AccountID(r.str())
	e.UpdatedAt = r.i64()
	decodeTransaction(r, &e.CoinTxn)
	return r.done()
}

func encodeQuoteSettled(buf []byte, e *quoteSettledEvent) []byte {
	b := buf
	b = putStr(b, string(e.QuoteID))
	b = putI64(b, e.UpdatedAt)
	b = encodeTransaction(b, &e.CashTxn)
	return b
}

func decodeQuoteSettled(p []byte, e *quoteSettledEvent) error {
	r := newReader(p)
	e.QuoteID = ledger.QuoteID(r.str())
	e.UpdatedAt = r.i64()
	decodeTransaction(r, &e.CashTxn)
	return r.done()
}

func encodeQuoteUpdated(buf []byte, e *quoteUpdatedEvent) []byte {
	b := buf
	b = putStr(b, string(e.ID))
	b = putStr(b, string(e.Status))
	b = putI64(b, e.UpdatedAt)
	return b
}

func decodeQuoteUpdated(p []byte, e *quoteUpdatedEvent) error {
	r := newReader(p)
	e.ID = ledger.QuoteID(r.str())
	e.Status = marketplace.QuoteStatus(r.str())
	e.UpdatedAt = r.i64()
	return r.done()
}
