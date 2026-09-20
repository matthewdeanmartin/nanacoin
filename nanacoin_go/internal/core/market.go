package core

import (
	"fmt"
	"sort"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// ListingInput is what a client may set on a listing. The seller, status and
// timestamps are the server's to decide.
type ListingInput struct {
	Title       string
	Description string
	Price       ledger.Amount
	Kind        string
	Currency    string
	MinorUnits  int64
	Side        marketplace.Side
}

// CreateListing posts an offer. The seller is always the authenticated user:
// listing something on someone else's behalf is not a v1 feature, and allowing
// it by accident would let anyone sell anyone's belongings.
func (s *Service) CreateListing(actor *users.User, in ListingInput) (*marketplace.Listing, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, err
	}
	if err := validateListingInput(in); err != nil {
		return nil, err
	}

	now := s.now().Unix()
	l := marketplace.Listing{
		ID:          ledger.ListingID(s.newID("listing")),
		Seller:      actor.Account,
		Title:       in.Title,
		Description: in.Description,
		Price:       in.Price,
		Quantity:    1,
		Status:      marketplace.StatusActive,
		CreatedAt:   now,
		UpdatedAt:   now,
		Kind:        in.Kind,
		Currency:    in.Currency,
		MinorUnits:  in.MinorUnits,
		Side:        in.Side,
	}
	if !s.store.canWriteListing(&l) {
		return nil, ErrCapacity
	}
	if err := s.commitEvent(storage.TypeListingCreated, &listingCreatedEvent{Listing: l}); err != nil {
		return nil, err
	}
	return s.listingByID(l.ID)
}

// UpdateListing edits an active listing. The seller may edit their own; Nana
// may edit any (spec 6, listing:edit_all).
func (s *Service) UpdateListing(actor *users.User, id ledger.ListingID, title, description *string, price *ledger.Amount) (*marketplace.Listing, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	l, err := s.listingByID(id)
	if err != nil {
		return nil, err
	}
	if l.Seller != actor.Account && !actor.IsNana() {
		return nil, ErrForbidden
	}
	if l.Status != marketplace.StatusActive {
		// Editing a sold listing would rewrite the description of
		// something already paid for, which is the marketplace equivalent
		// of editing the ledger.
		return nil, ErrListingClosed
	}

	ev := listingUpdatedEvent{ID: id, UpdatedAt: s.now().Unix()}
	if title != nil {
		if err := validateText(*title, MaxTitleLen, "title"); err != nil {
			return nil, err
		}
		if *title == "" {
			return nil, fmt.Errorf("%w: title is required", ErrBadInput)
		}
		ev.Title = title
	}
	if description != nil {
		if err := validateText(*description, MaxDescriptionLen, "description"); err != nil {
			return nil, err
		}
		ev.Description = description
	}
	if price != nil {
		if err := validateAmount(*price); err != nil {
			return nil, err
		}
		ev.Price = price
	}

	if title != nil {
		l.Title = *title
	}
	if description != nil {
		l.Description = *description
	}
	if !s.store.canWriteListing(l) {
		return nil, ErrCapacity
	}
	if err := s.commitEvent(storage.TypeListingUpdated, &ev); err != nil {
		return nil, err
	}
	return s.listingByID(id)
}

// CancelListing withdraws an active listing.
func (s *Service) CancelListing(actor *users.User, id ledger.ListingID) (*marketplace.Listing, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	l, err := s.listingByID(id)
	if err != nil {
		return nil, err
	}
	if l.Seller != actor.Account && !actor.IsNana() {
		return nil, ErrForbidden
	}
	if l.Status != marketplace.StatusActive {
		return nil, ErrListingClosed
	}

	cancelled := marketplace.StatusCancelled
	ev := listingUpdatedEvent{ID: id, Status: &cancelled, UpdatedAt: s.now().Unix()}
	if err := s.commitEvent(storage.TypeListingCancelled, &ev); err != nil {
		return nil, err
	}
	return s.listingByID(id)
}

// Purchase buys a listing: one call, one journal record, one state change.
//
// This is the operation the spec is most emphatic about (13). The payment and
// the listing becoming SOLD are a single event, so there is no window in which
// the buyer has paid for something still on sale, and no way for a client to
// perform half of it.
func (s *Service) Purchase(actor *users.User, id ledger.ListingID, scratch ...*WriteResult) (*marketplace.Listing, *ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, nil, err
	}
	l, err := s.listingByID(id)
	if err != nil {
		return nil, nil, err
	}
	if l.Status != marketplace.StatusActive {
		return nil, nil, ErrListingClosed
	}
	if l.Seller == actor.Account {
		return nil, nil, ErrSelfDeal
	}
	if err := s.checkUserAccountLocked(l.Seller); err != nil {
		return nil, nil, fmt.Errorf("seller unavailable: %w", err)
	}

	now := s.now().Unix()
	out := writeResult(scratch)
	txn := &out.Transaction
	*txn = ledger.Transaction{
		ID:          s.book.NextID(),
		Kind:        ledger.KindPurchase,
		CreatedAt:   now,
		Actor:       actor.ID,
		Description: l.Title,
		Reference:   string(l.ID),
		Postings:    out.Postings[:],
	}
	out.Postings = [ledger.MaxInlinePostings]ledger.Posting{
		{Account: actor.Account, Amount: -l.Price},
		{Account: l.Seller, Amount: l.Price},
	}
	if err := s.book.Validate(txn, false); err != nil {
		return nil, nil, err
	}

	ev := listingPurchasedEvent{
		ListingID: l.ID,
		Buyer:     actor.Account,
		Txn:       *txn,
		UpdatedAt: now,
	}
	if err := s.commitEvent(storage.TypeListingPurchased, &ev); err != nil {
		return nil, nil, err
	}
	updated, err := s.listingByID(id)
	if err != nil {
		return nil, nil, err
	}
	return updated, txn, nil
}

// Listings returns listings, newest first. status filters by status; an empty
// status returns all of them.
func (s *Service) Listings(status marketplace.Status) []*marketplace.Listing {
	s.mu.Lock()
	defer s.mu.Unlock()

	out := make([]*marketplace.Listing, 0, MaxListings)
	for _, l := range s.snapshotListings() {
		if status == "" || l.Status == status {
			out = append(out, l)
		}
	}
	// Map iteration order is random, so an explicit sort is what makes the
	// list stable between requests.
	sort.Slice(out, func(i, j int) bool {
		if out[i].CreatedAt != out[j].CreatedAt {
			return out[i].CreatedAt > out[j].CreatedAt
		}
		return out[i].ID < out[j].ID
	})
	return out
}

func (s *Service) Listing(id ledger.ListingID) (*marketplace.Listing, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	l, err := s.listingByID(id)
	return l, err == nil
}

func validateListingInput(in ListingInput) error {
	if in.Title == "" {
		return fmt.Errorf("%w: title is required", ErrBadInput)
	}
	if err := validateText(in.Title, MaxTitleLen, "title"); err != nil {
		return err
	}
	if err := validateText(in.Description, MaxDescriptionLen, "description"); err != nil {
		return err
	}
	if err := validateAmount(in.Price); err != nil {
		return err
	}
	// Currency is emitted even on item/service listings. Bound it there too,
	// so it cannot bypass the response/journal memory budget through Kind.
	if err := validateText(in.Currency, 8, "currency code"); err != nil {
		return err
	}
	switch in.Kind {
	case "", "item", "service":
		// Nothing further to check; these are descriptive only.
	case "currency":
		// NanaCoin does not hold or move external currency (spec 14), but
		// if a listing claims to be one, the claim should at least be
		// well formed enough to display.
		if in.Currency == "" {
			return fmt.Errorf("%w: currency listing needs a currency code", ErrBadInput)
		}
		if len(in.Currency) > 8 {
			return fmt.Errorf("%w: currency code is too long", ErrBadInput)
		}
		if in.MinorUnits <= 0 {
			return fmt.Errorf("%w: currency listing needs positive minor_units", ErrBadInput)
		}
	default:
		return fmt.Errorf("%w: unknown listing kind %q", ErrBadInput, in.Kind)
	}
	return nil
}
