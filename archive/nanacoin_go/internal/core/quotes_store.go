package core

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Quote storage.
//
// MaxQuotes is small on purpose. A quote board is a handful of live rates, not
// a history: a household with sixteen members does not need sixteen standing
// prices each, and a board that can hold hundreds invites someone to fill it.
// Filled and cancelled quotes are recycled oldest-first, exactly as closed
// listings and offers are.
const MaxQuotes = 16

// packedQuote is one standing rate. No arena text at all - a quote is four
// integers and two account references, which is the whole point of separating
// it from a listing.
type packedQuote struct {
	CreatedAt    int64
	UpdatedAt    int64
	ExpiresAt    int64
	CentsPerCoin int64
	Coins        int64

	// Maker and Taker are interned: they are account IDs, of which a
	// household has a handful.
	Maker ledger.Ref
	Taker ledger.Ref

	// ID is an arena slot because quote IDs are unique per quote and would
	// otherwise fill the intern table - the same reasoning as listing IDs.
	ID ledger.Slot

	// The two ledger sequence numbers of the legs, not string IDs. Zero means
	// unfilled.
	CoinTx uint32
	CashTx uint32

	Side   uint8
	Status uint8
	InUse  bool
}

const (
	quoteStatusOpen uint8 = iota
	quoteStatusFilled
	quoteStatusCancelled
	quoteStatusExpired
)

func quoteStatusFrom(s marketplace.QuoteStatus) uint8 {
	switch s {
	case marketplace.QuoteFilled:
		return quoteStatusFilled
	case marketplace.QuoteCancelled:
		return quoteStatusCancelled
	case marketplace.QuoteExpired:
		return quoteStatusExpired
	}
	return quoteStatusOpen
}

func quoteStatusTo(code uint8) marketplace.QuoteStatus {
	switch code {
	case quoteStatusFilled:
		return marketplace.QuoteFilled
	case quoteStatusCancelled:
		return marketplace.QuoteCancelled
	case quoteStatusExpired:
		return marketplace.QuoteExpired
	}
	return marketplace.QuoteOpen
}

// closed reports whether a quote has reached a state nothing further happens
// from, which is what makes its slot recyclable.
func (q *packedQuote) closed() bool {
	return q.Status != quoteStatusOpen
}

func (s *store) findQuote(id ledger.QuoteID) int {
	if id == "" {
		return -1
	}
	// Arena.Equal, not Get: Get builds a string per slot examined, so a miss
	// over a full table allocated one string per quote to return -1.
	want := string(id)
	for i := range s.quotesArr {
		p := &s.quotesArr[i]
		if p.InUse && s.arena.Equal(p.ID, want) {
			return i
		}
	}
	return -1
}

// quoteSlot returns where a quote should be written: its existing slot, a free
// one, or a recycled closed one. -1 means every slot holds a live quote.
func (s *store) quoteSlot(q *marketplace.Quote) int {
	if i := s.findQuote(q.ID); i >= 0 {
		return i
	}
	for i := range s.quotesArr {
		if !s.quotesArr[i].InUse {
			return i
		}
	}
	return s.recycleQuote()
}

// recycleQuote frees the oldest closed slot, or -1 if every quote is live.
//
// Open quotes are never recycled: someone is relying on that rate standing.
// An expired one is fair game, since it is no longer tradeable - but it is
// only treated as closed once something has marked it expired, so a quote does
// not vanish from the book the instant it lapses.
func (s *store) recycleQuote() int {
	oldest, at := -1, int64(0)
	for i := range s.quotesArr {
		p := &s.quotesArr[i]
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
	s.freeQuote(oldest)
	return oldest
}

func (s *store) freeQuote(i int) {
	p := &s.quotesArr[i]
	s.arena.Release(p.ID)
	if p.InUse {
		s.nQuotes--
	}
	*p = packedQuote{}
}

// canWriteQuote reports whether a quote would fit, without writing it.
func (s *store) canWriteQuote(q *marketplace.Quote) bool {
	return s.quoteSlot(q) >= 0
}

func (s *store) writeQuote(q *marketplace.Quote) bool {
	i := s.quoteSlot(q)
	if i < 0 {
		return false
	}
	p := &s.quotesArr[i]
	wasInUse := p.InUse

	// The ID is written once, on creation, and never changes afterwards.
	if !wasInUse || s.arena.Get(p.ID) != string(q.ID) {
		if wasInUse {
			s.arena.Release(p.ID)
		}
		// Stored whole or not at all: Arena.Put truncates to a block boundary
		// when it is short of room, and a clipped ID is a reference to
		// nothing. See the same check in writeOffer, and the arena test that
		// documents the truncation.
		id := s.arena.Put(string(q.ID))
		if s.arena.Get(id) != string(q.ID) {
			s.arena.Release(id)
			return false
		}
		p.ID = id
	}

	p.CreatedAt = q.CreatedAt
	p.UpdatedAt = q.UpdatedAt
	p.ExpiresAt = q.ExpiresAt
	p.CentsPerCoin = q.CentsPerCoin
	p.Coins = q.Coins
	p.Maker = s.strs.Intern(string(q.Maker))
	p.Taker = s.strs.Intern(string(q.Taker))
	p.Side = uint8(q.Side)
	p.Status = quoteStatusFrom(q.Status)
	p.CoinTx = seqOf(q.CoinTx)
	p.CashTx = seqOf(q.CashTx)

	if !wasInUse {
		p.InUse = true
		s.nQuotes++
	}
	return true
}

func seqOf(id ledger.TransactionID) uint32 {
	if id == "" {
		return 0
	}
	seq, _ := ledger.SeqForTransactionID(id)
	return seq
}

func txIDOf(seq uint32) ledger.TransactionID {
	if seq == 0 {
		return ""
	}
	return ledger.TransactionIDFor(seq)
}

func (s *store) unpackQuote(i int) *marketplace.Quote {
	p := &s.quotesArr[i]
	return &marketplace.Quote{
		ID:           ledger.QuoteID(s.arena.Get(p.ID)),
		Maker:        ledger.AccountID(s.strs.Lookup(p.Maker)),
		Side:         marketplace.QuoteSide(p.Side),
		CentsPerCoin: p.CentsPerCoin,
		Coins:        p.Coins,
		Status:       quoteStatusTo(p.Status),
		CreatedAt:    p.CreatedAt,
		UpdatedAt:    p.UpdatedAt,
		ExpiresAt:    p.ExpiresAt,
		Taker:        ledger.AccountID(s.strs.Lookup(p.Taker)),
		CoinTx:       txIDOf(p.CoinTx),
		CashTx:       txIDOf(p.CashTx),
	}
}

func (s *Service) quoteByID(id ledger.QuoteID) (*marketplace.Quote, error) {
	i := s.store.findQuote(id)
	if i < 0 {
		return nil, ErrQuoteUnknown
	}
	return s.store.unpackQuote(i), nil
}
