package core

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Config returns household policy.
// Now is the service's clock, which the API needs to say whether an
// acceptance is still inside its settlement window. The client cannot work
// that out for itself: a board with no RTC and a phone in another timezone
// will not agree on what time it is.
func (s *Service) Now() int64 {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.now().Unix()
}

func (s *Service) Config() Config {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.cfg
}

// SetConfig updates household policy. Nana only.
func (s *Service) SetConfig(actor *users.User, cfg Config) (Config, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if !actor.IsNana() {
		return Config{}, ErrForbidden
	}
	if cfg.InitialGrant < 0 {
		return Config{}, ErrBadInput
	}
	if err := validateName(cfg.HouseholdName, "household name"); err != nil {
		return Config{}, err
	}
	if cfg.Currency == "" {
		cfg.Currency = s.cfg.Currency
	}
	if err := s.commitEvent(storage.TypeConfigUpdated, &configUpdatedEvent{Config: cfg}); err != nil {
		return Config{}, err
	}
	return s.cfg, nil
}

// Users returns every household member, ordered by creation. Callers decide
// what to show of them; this returns the full records including verifiers, so
// the API layer must project them before serialising.
func (s *Service) Users() []*users.User {
	s.mu.Lock()
	defer s.mu.Unlock()

	out := make([]*users.User, 0, MaxUsers)
	s.eachUserLocked(func(u *users.User) bool {
		out = append(out, u)
		return true
	})
	return out
}

// Account returns an account and its owner.
func (s *Service) Account(id ledger.AccountID) (*users.Account, *users.User, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	a := s.accountByID(id)
	if a == nil {
		return nil, nil, false
	}
	owner, _ := s.userByID(a.UserID)
	return a, owner, true
}

// Balance is the fold over the account's postings.
func (s *Service) Balance(id ledger.AccountID) ledger.Amount {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.book.Balance(id)
}

// History returns transactions affecting an account, newest first.
func (s *Service) History(id ledger.AccountID, limit int) []*ledger.Transaction {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.book.History(id, limit)
}

// Transaction returns one transaction and the ID of its reversal, if reversed.
func (s *Service) Transaction(id ledger.TransactionID) (*ledger.Transaction, ledger.TransactionID, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	t, ok := s.book.Get(id)
	if !ok {
		return nil, "", false
	}
	rev, _ := s.book.ReversalOf(id)
	return t, rev, true
}

// AllTransactions returns the whole ledger, newest first. Nana only - the
// caller enforces that, since this package has no notion of a request.
func (s *Service) AllTransactions(limit int) []*ledger.Transaction {
	s.mu.Lock()
	defer s.mu.Unlock()

	all := s.book.All()
	n := len(all)
	if limit > 0 && limit < n {
		n = limit
	}
	out := make([]*ledger.Transaction, 0, n)
	for i := len(all) - 1; i >= 0 && len(out) < n; i-- {
		out = append(out, all[i])
	}
	return out
}

// ReversalOf exposes the original-to-reversal index for list rendering.
func (s *Service) ReversalOf(id ledger.TransactionID) (ledger.TransactionID, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.book.ReversalOf(id)
}

// Status is the health and capacity summary behind GET /status. It is the one
// endpoint that works before provisioning and without a token, because a
// household needs to be able to tell whether the board is alive.
type Status struct {
	Provisioned          bool          `json:"provisioned"`
	Household            string        `json:"household"`
	Currency             string        `json:"currency"`
	Users                int           `json:"users"`
	Transactions         int           `json:"transactions"`
	RetainedTransactions int           `json:"retained_transactions"`
	TransactionCapacity  int           `json:"transaction_capacity"`
	OldestTransaction    uint32        `json:"oldest_transaction"`
	ActiveList           int           `json:"active_listings"`
	Circulation          ledger.Amount `json:"circulation"`
	JournalUsed          int64         `json:"journal_used"`
	JournalCap           int64         `json:"journal_capacity"`
	LedgerBalance        bool          `json:"ledger_balanced"`

	// LogsEnabled and DiagEnabled tell the client which optional
	// diagnostics this build carries, so it can leave out a Logs tab that
	// would only ever 404. The API layer fills them in: whether the routes
	// exist is its business, not the service's.
	LogsEnabled bool `json:"logs_enabled"`
	DiagEnabled bool `json:"diag_enabled"`
}

func (s *Service) Status() Status {
	s.mu.Lock()
	defer s.mu.Unlock()

	active := 0
	for i := range s.store.listingsArr {
		p := &s.store.listingsArr[i]
		if p.InUse && p.Status == statusActive {
			active++
		}
	}
	used, capacity := s.journal.Size()
	return Status{
		Provisioned:          s.provisionedLocked(),
		Household:            s.cfg.HouseholdName,
		Currency:             s.cfg.Currency,
		Users:                s.store.nUsers,
		Transactions:         int(s.book.Total()),
		RetainedTransactions: s.book.Len(),
		TransactionCapacity:  ledger.Capacity,
		OldestTransaction:    s.book.Oldest(),
		ActiveList:           active,
		Circulation:          s.book.Circulation(),
		JournalUsed:          used,
		JournalCap:           capacity,
		LedgerBalance:        s.book.CheckInvariants() == nil,
	}
}

// Close releases the journal.
func (s *Service) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.journal.Close()
}

// Incremental queries.
//
// The slice-returning queries above allocate the whole page before the caller
// has looked at any of it, and on the board the caller then builds a parallel
// slice of view objects and encodes that. Three copies of a page, live at
// once, is what exhausted the heap.
//
// The methods below hand each record to a callback instead. Nothing is
// retained between calls, so the peak is one record rather than a page. The
// preparation callback holds the service lock. EachUser and EachListing
// optionally release it for a send callback between records; these are live
// traversals rather than whole-page snapshots.

// EachHistory calls fn for each transaction affecting the account, newest
// first, up to limit. fn must not call back into the Service: the lock is
// held.
//
// Stops early if fn returns false.
func (s *Service) EachHistory(id ledger.AccountID, limit int, fn func(*ledger.Transaction) bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.book.EachHistory(id, limit, fn)
}

// EachTransaction walks the whole ledger, newest first, up to limit.
//
// Replaces AllTransactions for the list endpoint. The old path called
// book.All(), which unpacked every record in the ledger and then discarded
// all but the first `limit` of them - so asking for 30 transactions out of a
// year of history allocated the year. This walks backwards and unpacks only
// what it yields.
func (s *Service) EachTransaction(limit int, fn func(*ledger.Transaction) bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.book.EachRecent(limit, fn)
}

// EachUser walks the household in creation order.
//
// Sorted into a small index slice rather than a slice of users: at household
// size the sort is trivial, and the point is that the caller does not build a
// parallel slice of view objects. Optional send runs outside the lock after
// fn has copied the record into owned storage.
func (s *Service) EachUser(fn func(*users.User) bool, send ...func() bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	s.eachUserLocked(func(u *users.User) bool {
		if !fn(u) {
			return false
		}
		return len(send) == 0 || s.sendUnlocked(send[0])
	})
}

// BalanceLocked is Balance without taking the lock, for use inside an Each*
// callback where the lock is already held.
//
// Exported deliberately rather than duplicating the fold in the API layer: a
// view that needs a balance while walking must not deadlock, and must not see
// a balance from a different instant than the record it is rendering.
func (s *Service) BalanceLocked(id ledger.AccountID) ledger.Amount {
	return s.book.Balance(id)
}

// NameLocked resolves an account to its display name with the lock already
// held. The API layer's namer needs this while streaming.
func (s *Service) NameLocked(id ledger.AccountID) string {
	if id == ledger.SystemIssuance {
		return "Issuance"
	}
	return s.store.accountName(id)
}

// AccountName avoids unpacking an account and its owner just to render a name.
func (s *Service) AccountName(id ledger.AccountID) string {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.NameLocked(id)
}

// ReversalOfLocked is ReversalOf with the lock already held.
func (s *Service) ReversalOfLocked(id ledger.TransactionID) (ledger.TransactionID, bool) {
	return s.book.ReversalOf(id)
}

// EachListing walks the marketplace, optionally filtered by status. Optional
// send runs outside the lock after fn copies the record into owned storage.
func (s *Service) EachListing(status marketplace.Status, fn func(*marketplace.Listing) bool, send ...func() bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	s.eachListingLocked(status, func(l *marketplace.Listing) bool {
		if !fn(l) {
			return false
		}
		return len(send) == 0 || s.sendUnlocked(send[0])
	})
}

// Circulation is the total NanaCoin in existence.
//
// Split out from Status because the full-ledger endpoint needs only this one
// number, and Status runs CheckInvariants - a fold over every record - which
// is a surprising amount of work to do for a field in a list response.
func (s *Service) Circulation() ledger.Amount {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.book.Circulation()
}

// ListingTitleLocked captions an offer with what it is for, without taking
// the lock - the caller already holds it, which is what EachOffer does.
//
// An offer whose listing has been recycled out of the table returns an empty
// title rather than failing: the money part is in the ledger regardless, and
// a caption is not worth failing a whole list over.
func (s *Service) ListingTitleLocked(id ledger.ListingID) string {
	i := s.store.findListing(id)
	if i < 0 {
		return ""
	}
	return s.store.arena.Get(s.store.listingsArr[i].Title)
}
