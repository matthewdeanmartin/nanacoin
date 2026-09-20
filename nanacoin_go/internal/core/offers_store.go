package core

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Offer storage, in the same fixed-array style as everything else here.
//
// MaxOffers is a separate cap from MaxListings because the two fill for
// different reasons: a household might have three listings and a dozen offers
// against one of them, or forty listings and no offers at all. Sizing offers
// off listings would be wrong in both directions.
//
// 32 slots at 56 bytes is 1,792 bytes - measured, not estimated. That is
// affordable next to the 4,640 bytes a no-logs build reclaims from the event
// ring, which is the trade this feature was budgeted against.
const MaxOffers = 32

// packedOffer is one proposal. 56 bytes, no pointers.
//
// Message goes to the arena rather than the intern table: "I can do it
// Saturday" is unique per offer, so interning it would fill the table with
// single-use entries.
type packedOffer struct {
	CreatedAt int64
	UpdatedAt int64
	SettlesAt int64
	Amount    int64

	// Offerer is interned - a household has a handful of accounts. ID and
	// Listing are arena slots for the same reason listing IDs are.
	Offerer ledger.Ref

	ID      ledger.Slot
	Listing ledger.Slot
	Message ledger.Slot

	// SettledTx is the ledger sequence number of the accepting transaction,
	// not a string ID. Zero means not accepted.
	SettledTx uint32

	Status uint8
	InUse  bool
}

// offerStatusCode packs the status into a byte. The string forms stay on the
// wire; only the code is stored.
const (
	offerStatusOpen uint8 = iota
	offerStatusAccepted
	offerStatusSettled
	offerStatusDeclined
	offerStatusWithdrawn
	offerStatusReversed
)

func offerStatusFrom(s marketplace.OfferStatus) uint8 {
	switch s {
	case marketplace.OfferAccepted:
		return offerStatusAccepted
	case marketplace.OfferSettled:
		return offerStatusSettled
	case marketplace.OfferDeclined:
		return offerStatusDeclined
	case marketplace.OfferWithdrawn:
		return offerStatusWithdrawn
	case marketplace.OfferReversed:
		return offerStatusReversed
	}
	return offerStatusOpen
}

func offerStatusTo(code uint8) marketplace.OfferStatus {
	switch code {
	case offerStatusAccepted:
		return marketplace.OfferAccepted
	case offerStatusSettled:
		return marketplace.OfferSettled
	case offerStatusDeclined:
		return marketplace.OfferDeclined
	case offerStatusWithdrawn:
		return marketplace.OfferWithdrawn
	case offerStatusReversed:
		return marketplace.OfferReversed
	}
	return marketplace.OfferOpen
}

// closed reports whether an offer has reached a state nothing further happens
// from, which is what makes its slot recyclable.
func (o *packedOffer) closed() bool {
	switch o.Status {
	case offerStatusSettled, offerStatusDeclined, offerStatusWithdrawn, offerStatusReversed:
		return true
	}
	return false
}

// --- store access, in the same shape as the listing helpers ---

func (s *store) findOffer(id ledger.OfferID) int {
	if id == "" {
		return -1
	}
	// See findQuote: comparing in place rather than rehydrating each slot.
	want := string(id)
	for i := range s.offersArr {
		p := &s.offersArr[i]
		if p.InUse && s.arena.Equal(p.ID, want) {
			return i
		}
	}
	return -1
}

// offerSlot returns where an offer should be written: its existing slot, a
// free one, or a recycled closed one. -1 means every slot holds a live offer.
func (s *store) offerSlot(o *marketplace.Offer) int {
	if i := s.findOffer(o.ID); i >= 0 {
		return i
	}
	for i := range s.offersArr {
		if !s.offersArr[i].InUse {
			return i
		}
	}
	return s.recycleOffer()
}

// recycleOffer frees the oldest closed slot, or -1 if every offer is live.
//
// Open and accepted offers are never recycled: an open one is a proposal
// someone is waiting on, and an accepted one inside its window is a deal that
// can still be undone. Losing either would lose something the household is
// relying on. Closed offers are history, and the ledger keeps the money part
// regardless.
func (s *store) recycleOffer() int {
	oldest, at := -1, int64(0)
	for i := range s.offersArr {
		p := &s.offersArr[i]
		if !p.InUse || !p.closed() {
			continue
		}
		if oldest < 0 || p.UpdatedAt < at {
			oldest, at = i, p.UpdatedAt
		}
	}
	if oldest < 0 {
		return -1
	}
	s.freeOffer(oldest)
	return oldest
}

func (s *store) freeOffer(i int) {
	p := &s.offersArr[i]
	s.arena.Release(p.ID)
	s.arena.Release(p.Listing)
	s.arena.Release(p.Message)
	if p.InUse {
		s.nOffers--
	}
	*p = packedOffer{}
}

// canWriteOffer reports whether an offer would fit, without writing it.
func (s *store) canWriteOffer(o *marketplace.Offer) bool {
	return s.offerSlot(o) >= 0
}

func (s *store) writeOffer(o *marketplace.Offer) bool {
	i := s.offerSlot(o)
	if i < 0 {
		return false
	}

	var settledTx uint32
	if o.SettledTx != "" {
		if seq, ok := ledger.SeqForTransactionID(o.SettledTx); ok {
			settledTx = seq
		}
	}

	p := &s.offersArr[i]
	wasInUse := p.InUse

	// Text is written once, on creation: an offer's id, listing and message
	// never change afterwards, only its status does.
	if !wasInUse || s.arena.Get(p.ID) != string(o.ID) {
		if wasInUse {
			s.arena.Release(p.ID)
			s.arena.Release(p.Listing)
			s.arena.Release(p.Message)
		}
		// The two identity fields must be stored whole or not at all.
		//
		// Arena.Put truncates rather than failing when it is short of room -
		// the right policy for a memo, where a clipped sentence still reads,
		// and the wrong one for an ID, where a clipped string is a reference
		// to nothing. That is exactly what happened: an offer was written
		// against "listing-dBibwBup", an eight-character prefix of a
		// twelve-character ID, and every later lookup 404'd. The offer showed
		// in the list with an empty title and could never be accepted.
		//
		// A short arena must refuse the offer, not store a broken one.
		id := s.arena.Put(string(o.ID))
		if s.arena.Get(id) != string(o.ID) {
			s.arena.Release(id)
			return false
		}
		listing := s.arena.Put(string(o.Listing))
		if s.arena.Get(listing) != string(o.Listing) {
			s.arena.Release(id)
			s.arena.Release(listing)
			return false
		}
		// The message may be truncated: it is prose, and a clipped note is
		// better than a refused offer.
		msg := s.arena.Put(o.Message)
		if o.Message != "" && msg.IsEmpty() {
			s.arena.Release(id)
			s.arena.Release(listing)
			return false
		}
		p.ID, p.Listing, p.Message = id, listing, msg
	}

	p.CreatedAt = o.CreatedAt
	p.UpdatedAt = o.UpdatedAt
	p.SettlesAt = o.SettlesAt
	p.Amount = o.Amount
	p.Offerer = s.strs.Intern(string(o.Offerer))
	p.SettledTx = settledTx
	p.Status = offerStatusFrom(o.Status)

	if !wasInUse {
		p.InUse = true
		s.nOffers++
	}
	return true
}

func (s *store) unpackOffer(i int) *marketplace.Offer {
	p := &s.offersArr[i]
	var settledTx ledger.TransactionID
	if p.SettledTx != 0 {
		settledTx = ledger.TransactionIDFor(p.SettledTx)
	}
	return &marketplace.Offer{
		ID:        ledger.OfferID(s.arena.Get(p.ID)),
		Listing:   ledger.ListingID(s.arena.Get(p.Listing)),
		Offerer:   ledger.AccountID(s.strs.Lookup(p.Offerer)),
		Amount:    p.Amount,
		Message:   s.arena.Get(p.Message),
		Status:    offerStatusTo(p.Status),
		CreatedAt: p.CreatedAt,
		UpdatedAt: p.UpdatedAt,
		SettledTx: settledTx,
		SettlesAt: p.SettlesAt,
	}
}

func (s *Service) offerByID(id ledger.OfferID) (*marketplace.Offer, error) {
	i := s.store.findOffer(id)
	if i < 0 {
		return nil, ErrOfferUnknown
	}
	return s.store.unpackOffer(i), nil
}
