package core

import (
	"fmt"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Offers: proposing a deal, accepting one, and undoing an acceptance.
//
// Before this the marketplace could do exactly one thing - buy a listing at
// its asking price. There was no way to offer 25 coins for peanut butter
// cookies, and no way to haggle. An offer is the missing step: a proposal
// that moves no money until the other person accepts.
//
// # Why acceptance is reversible
//
// The failure this is built around is a child accepting every offer in the
// house, becoming rich, and delivering none of it.
//
// Money could not simply be held: escrow needs a pending-funds concept the
// ledger does not have, and adding one would mean a balance that is not just
// the sum of postings - which is the single invariant everything else here
// rests on. So acceptance pays immediately and stays *undoable* for a window
// afterwards, which gets the same protection without a new kind of money.
//
// The undo is an ordinary ledger reversal - a mirror transaction, not an edit
// - so the audit trail shows the deal and its unwinding rather than a deal
// that silently never happened.

// MaxOfferMessage bounds the note attached to an offer. Short: it is a line
// of context ("I can do it Saturday"), not a description.
const MaxOfferMessage = 140

// MakeOffer proposes a deal against a listing. No money moves.
func (s *Service) MakeOffer(
	actor *users.User,
	listingID ledger.ListingID,
	amount ledger.Amount,
	message string,
) (*marketplace.Offer, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, err
	}
	if err := validateAmount(amount); err != nil {
		return nil, err
	}
	if err := validateText(message, MaxOfferMessage, "message"); err != nil {
		return nil, err
	}

	l, err := s.listingByID(listingID)
	if err != nil {
		return nil, err
	}
	if l.Status != marketplace.StatusActive {
		return nil, ErrListingClosed
	}
	if l.Seller == actor.Account {
		return nil, ErrSelfDeal
	}

	// Funds are *not* checked here. An offer is a proposal, and someone may
	// reasonably offer on the strength of an allowance not yet paid. The
	// check happens at acceptance, which is when money actually moves.

	now := s.now().Unix()
	offer := marketplace.Offer{
		ID:        ledger.OfferID(s.newID("offer")),
		Listing:   l.ID,
		Offerer:   actor.Account,
		Amount:    amount,
		Message:   message,
		Status:    marketplace.OfferOpen,
		CreatedAt: now,
		UpdatedAt: now,
	}
	if !s.store.canWriteOffer(&offer) {
		return nil, ErrCapacity
	}
	if err := s.commitEvent(storage.TypeOfferCreated, &offerCreatedEvent{Offer: offer}); err != nil {
		return nil, err
	}
	return s.offerByID(offer.ID)
}

// AcceptOffer moves the money and closes the listing, in one event.
//
// Which way the coins go depends on the listing's side. On a SELL the offerer
// is buying, so they pay. On a BUY - a want-ad - the poster is paying someone
// to do the thing, so the money goes the other way. Getting this backwards
// would be the worst possible bug here, which is why Side is read from the
// listing rather than inferred from who is accepting.
func (s *Service) AcceptOffer(
	actor *users.User,
	id ledger.OfferID,
	scratch ...*WriteResult,
) (*marketplace.Offer, *ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, nil, err
	}

	offer, err := s.offerByID(id)
	if err != nil {
		return nil, nil, err
	}
	l, err := s.listingByID(offer.Listing)
	if err != nil {
		return nil, nil, ErrOfferOrphaned
	}
	if l.Seller != actor.Account {
		return nil, nil, ErrForbidden
	}
	if offer.Status != marketplace.OfferOpen {
		return nil, nil, ErrOfferClosed
	}
	if l.Status != marketplace.StatusActive {
		return nil, nil, ErrListingClosed
	}

	payer, payee := offer.Offerer, l.Seller
	if l.Side == marketplace.SideBuy {
		payer, payee = l.Seller, offer.Offerer
	}
	if err := s.checkUserAccountLocked(payer); err != nil {
		return nil, nil, fmt.Errorf("payer unavailable: %w", err)
	}
	if err := s.checkUserAccountLocked(payee); err != nil {
		return nil, nil, fmt.Errorf("payee unavailable: %w", err)
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
		{Account: payer, Amount: -offer.Amount},
		{Account: payee, Amount: offer.Amount},
	}
	// Funds are checked here, at the moment they move.
	if err := s.book.Validate(txn, false); err != nil {
		return nil, nil, err
	}

	ev := offerAcceptedEvent{
		OfferID:   offer.ID,
		ListingID: l.ID,
		Buyer:     payer,
		Txn:       *txn,
		SettlesAt: now + s.cfg.SettlementWindow(),
		UpdatedAt: now,
	}
	if err := s.commitEvent(storage.TypeOfferAccepted, &ev); err != nil {
		return nil, nil, err
	}

	updated, err := s.offerByID(id)
	if err != nil {
		return nil, nil, err
	}
	return updated, txn, nil
}

// UnacceptOffer undoes an acceptance inside its settlement window.
//
// Either party may do it, and so may Nana. That is deliberate: the deal can
// fall through from either side - the thing was never delivered, or the payer
// changed their mind - and requiring one particular person to be available
// would leave the money stuck where it does not belong.
//
// After the window it refuses, which is the point of having one: a settled
// deal is settled, and an acceptance that has stood for two days is no longer
// something to be quietly walked back.
func (s *Service) UnacceptOffer(
	actor *users.User,
	id ledger.OfferID,
	reason string,
	scratch ...*WriteResult,
) (*marketplace.Offer, *ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, nil, err
	}
	if err := validateText(reason, MaxMemoLen, "reason"); err != nil {
		return nil, nil, err
	}

	offer, err := s.offerByID(id)
	if err != nil {
		return nil, nil, err
	}
	l, err := s.listingByID(offer.Listing)
	if err != nil {
		return nil, nil, ErrOfferOrphaned
	}

	if offer.Offerer != actor.Account && l.Seller != actor.Account && !actor.IsNana() {
		return nil, nil, ErrForbidden
	}
	if offer.Status != marketplace.OfferAccepted {
		return nil, nil, ErrOfferClosed
	}

	now := s.now().Unix()
	if !offer.Reversible(now) {
		return nil, nil, ErrOfferSettled
	}

	// The money comes back the way it went, as a mirror transaction. Overdraft
	// is allowed: if the payee has already spent it, the correction still
	// happens and their balance goes negative where the household can see it -
	// the same rule Reverse follows, and for the same reason.
	out := writeResult(scratch)
	txn, err := s.book.BuildReversalInto(offer.SettledTx, actor.ID, reason, now, &out.Transaction, out.Postings[:])
	if err != nil {
		return nil, nil, err
	}
	txn.ID = s.book.NextID()
	if err := s.book.Validate(txn, true); err != nil {
		return nil, nil, err
	}

	if err := s.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		return nil, nil, err
	}

	// The listing goes back on the market: the deal fell through, so the thing
	// is for sale again rather than sitting sold against a payment that has
	// been undone.
	ev := offerUpdatedEvent{
		ID:        offer.ID,
		Status:    marketplace.OfferReversed,
		UpdatedAt: now,
		Reopen:    true,
	}
	if err := s.commitEvent(storage.TypeOfferUpdated, &ev); err != nil {
		return nil, nil, err
	}

	updated, err := s.offerByID(id)
	if err != nil {
		return nil, nil, err
	}
	return updated, txn, nil
}

// DeclineOffer refuses a proposal, leaving the listing open for others.
func (s *Service) DeclineOffer(actor *users.User, id ledger.OfferID) (*marketplace.Offer, error) {
	return s.closeOffer(actor, id, marketplace.OfferDeclined)
}

// WithdrawOffer takes back an offer, while it is still open.
func (s *Service) WithdrawOffer(actor *users.User, id ledger.OfferID) (*marketplace.Offer, error) {
	return s.closeOffer(actor, id, marketplace.OfferWithdrawn)
}

func (s *Service) closeOffer(
	actor *users.User,
	id ledger.OfferID,
	to marketplace.OfferStatus,
) (*marketplace.Offer, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, err
	}
	offer, err := s.offerByID(id)
	if err != nil {
		return nil, err
	}
	if offer.Status != marketplace.OfferOpen {
		return nil, ErrOfferClosed
	}

	// Declining is the listing owner's; withdrawing is the offerer's. Nana may
	// do either, because a household needs someone who can clear a stalemate.
	if to == marketplace.OfferWithdrawn {
		if offer.Offerer != actor.Account && !actor.IsNana() {
			return nil, ErrForbidden
		}
	} else {
		l, err := s.listingByID(offer.Listing)
		if err != nil {
			return nil, err
		}
		if l.Seller != actor.Account && !actor.IsNana() {
			return nil, ErrForbidden
		}
	}

	ev := offerUpdatedEvent{ID: offer.ID, Status: to, UpdatedAt: s.now().Unix()}
	if err := s.commitEvent(storage.TypeOfferUpdated, &ev); err != nil {
		return nil, err
	}
	return s.offerByID(id)
}

// EachOffer walks the offers the caller may see, newest first, without
// building a list of them.
//
// Streaming rather than returning a slice, like every other list endpoint
// here. The first version allocated three times per request - a snapshot
// slice, 32 unpacked offers with their arena strings, and a slice of views -
// and the board ran out of memory after a handful of requests. One record is
// in flight at a time; the handler's pooled buffer holds it.
//
// send is the API layer's hook for writing the record out while the lock is
// released, matching EachListing.
func (s *Service) EachOffer(actor *users.User, fn func(*marketplace.Offer) bool, send ...func() bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// A stack array, not a slice: MaxOffers is 32, so this is 128 bytes of
	// stack rather than a heap allocation on every request. The same trick
	// eachListingLocked uses.
	var order [MaxOffers]int
	n := 0
	nana := actor.IsNana()

	for i := range s.store.offersArr {
		p := &s.store.offersArr[i]
		if !p.InUse {
			continue
		}
		if !nana && !s.maySeeOfferLocked(actor, p) {
			continue
		}
		order[n] = i
		n++
	}

	// Newest first. Insertion sort over the indices: at 32 entries it beats
	// sort.Slice, and unlike sort.Slice it allocates no closure.
	for a := 1; a < n; a++ {
		v := order[a]
		at := s.store.offersArr[v].CreatedAt
		b := a - 1
		for b >= 0 && s.store.offersArr[order[b]].CreatedAt < at {
			order[b+1] = order[b]
			b--
		}
		order[b+1] = v
	}

	for k := 0; k < n; k++ {
		i := order[k]
		if !s.store.offersArr[i].InUse {
			continue
		}
		if !fn(s.store.unpackOffer(i)) {
			return
		}
		if len(send) > 0 && !s.sendUnlocked(send[0]) {
			return
		}
	}
}

// maySeeOfferLocked is the visibility rule: your own offers, and any on your
// own listings. Nana is handled by the caller.
func (s *Service) maySeeOfferLocked(actor *users.User, p *packedOffer) bool {
	if s.store.strs.Lookup(p.Offerer) == string(actor.Account) {
		return true
	}
	li := s.store.findListing(ledger.ListingID(s.store.arena.Get(p.Listing)))
	if li < 0 {
		return false
	}
	return s.store.strs.Lookup(s.store.listingsArr[li].Seller) == string(actor.Account)
}

// Offer returns one offer, if the caller may see it.
func (s *Service) Offer(actor *users.User, id ledger.OfferID) (*marketplace.Offer, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	o, err := s.offerByID(id)
	if err != nil {
		return nil, false
	}
	if actor.IsNana() {
		return o, true
	}
	i := s.store.findOffer(id)
	if i >= 0 && s.maySeeOfferLocked(actor, &s.store.offersArr[i]) {
		return o, true
	}
	return nil, false
}
