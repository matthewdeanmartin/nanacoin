package core

import (
	"errors"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// offerHousehold is a service with Nana and two members holding coins, plus a
// controllable clock - the settlement window is a deadline, so the tests have
// to be able to cross it without sleeping for two days.
type offerHousehold struct {
	svc   *Service
	nana  *users.User
	alice *users.User
	bob   *users.User
	clock *time.Time
}

func newOfferHousehold(t *testing.T) *offerHousehold {
	t.Helper()

	// A movable clock: the settlement window is a deadline, so the tests have
	// to cross it without sleeping for two days.
	now := time.Unix(1_700_000_000, 0)
	var seq int
	opts := testOptions(&seq)
	opts.Now = func() time.Time { return now }

	svc, err := New(memory.New(), opts)
	if err != nil {
		t.Fatalf("New: %v", err)
	}

	// household grants the initial allocation, and the issues below add to
	// it - so each member starts at startingBalance, not at 100.
	nana, alice, bob := household(t, svc)
	if _, err := svc.Issue(nana, alice.Account, 100, "start"); err != nil {
		t.Fatalf("issue to alice: %v", err)
	}
	if _, err := svc.Issue(nana, bob.Account, 100, "start"); err != nil {
		t.Fatalf("issue to bob: %v", err)
	}
	return &offerHousehold{svc: svc, nana: nana, alice: alice, bob: bob, clock: &now}
}

// startingBalance is what each member holds once newOfferHousehold returns:
// the initial allocation household() grants, plus the 100 issued here.
const startingBalance ledger.Amount = 200

func (h *offerHousehold) advance(d time.Duration) { *h.clock = h.clock.Add(d) }

// replay rebuilds a service from the same journal, which is what a reboot
// does. The offers have to come back or an accepted deal loses its deadline.
func (h *offerHousehold) replay(t *testing.T) *Service {
	t.Helper()
	var seq int
	replayed, err := New(h.svc.journal, testOptions(&seq))
	if err != nil {
		t.Fatalf("replay: %v", err)
	}
	return replayed
}

// countOffers walks the streaming iterator, which is what the API does.
func (h *offerHousehold) countOffers(u *users.User) int {
	n := 0
	h.svc.EachOffer(u, func(*marketplace.Offer) bool { n++; return true })
	return n
}

func (h *offerHousehold) balance(u *users.User) ledger.Amount {
	return h.svc.Balance(u.Account)
}

// sell posts something for sale; want posts a want-ad.
func (h *offerHousehold) sell(t *testing.T, seller *users.User, title string, price ledger.Amount) *marketplace.Listing {
	t.Helper()
	l, err := h.svc.CreateListing(seller, ListingInput{Title: title, Price: price})
	if err != nil {
		t.Fatalf("create listing: %v", err)
	}
	return l
}

func (h *offerHousehold) want(t *testing.T, poster *users.User, title string, price ledger.Amount) *marketplace.Listing {
	t.Helper()
	l, err := h.svc.CreateListing(poster, ListingInput{Title: title, Price: price, Side: marketplace.SideBuy})
	if err != nil {
		t.Fatalf("create want-ad: %v", err)
	}
	return l
}

func TestOfferMovesNoMoneyUntilAccepted(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Switch time", 20)

	if _, err := h.svc.MakeOffer(h.bob, l.ID, 15, "would you take 15?"); err != nil {
		t.Fatalf("make offer: %v", err)
	}

	if got := h.balance(h.bob); got != startingBalance {
		t.Errorf("bob's balance is %d after offering, want %d - an offer is not a payment", got, startingBalance)
	}
	if got := h.balance(h.alice); got != startingBalance {
		t.Errorf("alice's balance is %d, want %d", got, startingBalance)
	}
	if l, _ := h.svc.Listing(l.ID); l.Status != marketplace.StatusActive {
		t.Errorf("listing is %s, want ACTIVE", l.Status)
	}
}

func TestAcceptPaysTheOfferedPriceNotTheAsking(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Switch time", 20)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 15, "")

	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}

	// 15, not the 20 asking price: haggling is the point.
	if got := h.balance(h.alice); got != startingBalance+15 {
		t.Errorf("alice has %d, want %d", got, startingBalance+15)
	}
	if got := h.balance(h.bob); got != startingBalance-15 {
		t.Errorf("bob has %d, want %d", got, startingBalance-15)
	}
}

func TestWantAdPaysTheOtherWayRound(t *testing.T) {
	// The poster has the money and wants the thing done, so accepting sends
	// coins from the poster to whoever offered. Getting this backwards would
	// be the worst bug in the feature.
	h := newOfferHousehold(t)
	l := h.want(t, h.alice, "Bake cookies", 25)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 20, "Saturday")

	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}

	if got := h.balance(h.alice); got != startingBalance-20 {
		t.Errorf("alice (who posted the want-ad) has %d, want %d", got, startingBalance-20)
	}
	if got := h.balance(h.bob); got != startingBalance+20 {
		t.Errorf("bob (who offered to do it) has %d, want %d", got, startingBalance+20)
	}
}

func TestAcceptChecksFundsAtAcceptanceNotAtOffer(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Expensive thing", 500)

	// Offering more than you hold is allowed: an allowance may be coming.
	o, err := h.svc.MakeOffer(h.bob, l.ID, 500, "")
	if err != nil {
		t.Fatalf("offering beyond your balance should be allowed: %v", err)
	}

	// Accepting it is not.
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err == nil {
		t.Fatal("accepting an unaffordable offer succeeded")
	}
	if got := h.balance(h.bob); got != startingBalance {
		t.Errorf("bob's balance moved to %d on a refused acceptance", got)
	}
}

func TestOnlyTheListingOwnerMayAccept(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 8, "")

	if _, _, err := h.svc.AcceptOffer(h.bob, o.ID); !errors.Is(err, ErrForbidden) {
		t.Errorf("bob accepting his own offer gave %v, want ErrForbidden", err)
	}
	// Not even Nana: accepting is a decision about your own property.
	if _, _, err := h.svc.AcceptOffer(h.nana, o.ID); !errors.Is(err, ErrForbidden) {
		t.Errorf("nana accepting gave %v, want ErrForbidden", err)
	}
}

func TestCannotOfferOnYourOwnListing(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	if _, err := h.svc.MakeOffer(h.alice, l.ID, 8, ""); !errors.Is(err, ErrSelfDeal) {
		t.Errorf("got %v, want ErrSelfDeal", err)
	}
}

// --- the settlement window --------------------------------------------------
//
// The feature this exists for: a child accepts every offer in the house,
// becomes rich, and delivers none of it.

func TestAcceptanceIsReversibleInsideTheWindow(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Wash the car", 40)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 40, "")

	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}
	if got := h.balance(h.bob); got != startingBalance-40 {
		t.Fatalf("bob has %d after accepting, want %d", got, startingBalance-40)
	}

	h.advance(47 * time.Hour)

	updated, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "never washed it")
	if err != nil {
		t.Fatalf("unaccept inside the window: %v", err)
	}
	if updated.Status != marketplace.OfferReversed {
		t.Errorf("status is %s, want REVERSED", updated.Status)
	}
	if got := h.balance(h.bob); got != startingBalance {
		t.Errorf("bob has %d after the reversal, want his %d back", got, startingBalance)
	}
	if got := h.balance(h.alice); got != startingBalance {
		t.Errorf("alice has %d, want %d", got, startingBalance)
	}
}

func TestAcceptanceIsFinalAfterTheWindow(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Wash the car", 40)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 40, "")
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}

	h.advance(49 * time.Hour)

	if _, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "too late"); !errors.Is(err, ErrOfferSettled) {
		t.Errorf("got %v, want ErrOfferSettled", err)
	}
	// Even Nana cannot walk back a settled deal casually - Reverse is still
	// there for a genuine correction, which is a different act.
	if _, _, err := h.svc.UnacceptOffer(h.nana, o.ID, "nana says"); !errors.Is(err, ErrOfferSettled) {
		t.Errorf("nana got %v, want ErrOfferSettled", err)
	}
	if got := h.balance(h.bob); got != startingBalance-40 {
		t.Errorf("bob has %d, want the deal to have stood at %d", got, startingBalance-40)
	}
}

func TestSettlementWindowIsConfigurable(t *testing.T) {
	h := newOfferHousehold(t)
	cfg := h.svc.Config()
	cfg.OfferSettlesAfter = 3600 // one hour
	if _, err := h.svc.SetConfig(h.nana, cfg); err != nil {
		t.Fatalf("set config: %v", err)
	}

	l := h.sell(t, h.alice, "Thing", 10)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 10, "")
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}

	h.advance(90 * time.Minute)
	if _, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "late"); !errors.Is(err, ErrOfferSettled) {
		t.Errorf("with a one-hour window, got %v after 90 minutes, want ErrOfferSettled", err)
	}
}

func TestEitherPartyMayUndoAnAcceptance(t *testing.T) {
	// The deal can fall through from either side, and requiring one specific
	// person to be available would leave the money stuck.
	for _, who := range []string{"offerer", "owner", "nana"} {
		t.Run(who, func(t *testing.T) {
			h := newOfferHousehold(t)
			l := h.sell(t, h.alice, "Thing", 10)
			o, _ := h.svc.MakeOffer(h.bob, l.ID, 10, "")
			if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
				t.Fatalf("accept: %v", err)
			}

			actor := h.bob
			switch who {
			case "owner":
				actor = h.alice
			case "nana":
				actor = h.nana
			}
			if _, _, err := h.svc.UnacceptOffer(actor, o.ID, "fell through"); err != nil {
				t.Fatalf("%s could not undo: %v", who, err)
			}
		})
	}
}

func TestUndoingAnAcceptancePutsTheListingBack(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 10, "")
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}
	if got, _ := h.svc.Listing(l.ID); got.Status != marketplace.StatusSold {
		t.Fatalf("listing is %s after acceptance, want SOLD", got.Status)
	}

	if _, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "fell through"); err != nil {
		t.Fatalf("unaccept: %v", err)
	}

	// The deal fell through, so the thing is for sale again rather than
	// sitting sold against a payment that has been undone.
	got, _ := h.svc.Listing(l.ID)
	if got.Status != marketplace.StatusActive {
		t.Errorf("listing is %s, want ACTIVE again", got.Status)
	}
	if got.Buyer != "" {
		t.Errorf("listing still names %s as buyer", got.Buyer)
	}
}

func TestUndoIsAppendedNotErased(t *testing.T) {
	// The audit trail must show the deal and its unwinding, not a deal that
	// silently never happened.
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 10, "")
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}
	before := h.svc.Status().Transactions

	if _, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "fell through"); err != nil {
		t.Fatalf("unaccept: %v", err)
	}

	if after := h.svc.Status().Transactions; after != before+1 {
		t.Errorf("transactions went %d -> %d, want one more", before, after)
	}
	if !h.svc.Status().LedgerBalance {
		t.Error("the ledger does not balance after an undone acceptance")
	}
}

func TestCannotUndoTwice(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 10, "")
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}
	if _, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "once"); err != nil {
		t.Fatalf("first undo: %v", err)
	}
	if _, _, err := h.svc.UnacceptOffer(h.bob, o.ID, "twice"); err == nil {
		t.Error("undoing twice succeeded, which would pay the money back twice")
	}
}

// --- declining, withdrawing and visibility ----------------------------------

func TestDeclineAndWithdrawBelongToDifferentPeople(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)

	o, _ := h.svc.MakeOffer(h.bob, l.ID, 8, "")
	if _, err := h.svc.DeclineOffer(h.bob, o.ID); !errors.Is(err, ErrForbidden) {
		t.Errorf("bob declining his own offer gave %v, want ErrForbidden", err)
	}
	if _, err := h.svc.DeclineOffer(h.alice, o.ID); err != nil {
		t.Errorf("alice declining an offer on her listing: %v", err)
	}

	o2, _ := h.svc.MakeOffer(h.bob, l.ID, 9, "")
	if _, err := h.svc.WithdrawOffer(h.alice, o2.ID); !errors.Is(err, ErrForbidden) {
		t.Errorf("alice withdrawing bob's offer gave %v, want ErrForbidden", err)
	}
	if _, err := h.svc.WithdrawOffer(h.bob, o2.ID); err != nil {
		t.Errorf("bob withdrawing his own offer: %v", err)
	}
}

func TestClosedOffersCannotBeAccepted(t *testing.T) {
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	o, _ := h.svc.MakeOffer(h.bob, l.ID, 8, "")
	if _, err := h.svc.WithdrawOffer(h.bob, o.ID); err != nil {
		t.Fatalf("withdraw: %v", err)
	}
	if _, _, err := h.svc.AcceptOffer(h.alice, o.ID); !errors.Is(err, ErrOfferClosed) {
		t.Errorf("got %v, want ErrOfferClosed", err)
	}
}

func TestOffersAreVisibleToTheTwoPartiesAndNana(t *testing.T) {
	h := newOfferHousehold(t)
	carol, err := h.svc.CreateUser(h.nana, "carol", "Carol", "carol-pin-12", users.RoleUser, false)
	if err != nil {
		t.Fatalf("create carol: %v", err)
	}

	l := h.sell(t, h.alice, "Thing", 10)
	if _, err := h.svc.MakeOffer(h.bob, l.ID, 8, ""); err != nil {
		t.Fatalf("make offer: %v", err)
	}

	if got := h.countOffers(h.bob); got != 1 {
		t.Errorf("the offerer sees %d offers, want 1", got)
	}
	if got := h.countOffers(h.alice); got != 1 {
		t.Errorf("the listing owner sees %d offers, want 1", got)
	}
	if got := h.countOffers(h.nana); got != 1 {
		t.Errorf("nana sees %d offers, want 1", got)
	}
	if got := h.countOffers(carol); got != 0 {
		t.Errorf("an unrelated member sees %d offers, want 0", got)
	}
}

func TestOffersSurviveReplay(t *testing.T) {
	// Offers are journalled, so a reboot must not lose an open proposal or
	// the settlement deadline on an accepted one.
	h := newOfferHousehold(t)
	l := h.sell(t, h.alice, "Thing", 10)
	open, _ := h.svc.MakeOffer(h.bob, l.ID, 8, "still thinking")
	accepted, _ := h.svc.MakeOffer(h.bob, l.ID, 10, "final offer")
	if _, _, err := h.svc.AcceptOffer(h.alice, accepted.ID); err != nil {
		t.Fatalf("accept: %v", err)
	}

	// Replay from the same journal, as a reboot does.
	replayed := h.replay(t)

	got, err := replayed.offerByID(open.ID)
	if err != nil {
		t.Fatalf("the open offer did not survive replay: %v", err)
	}
	if got.Status != marketplace.OfferOpen || got.Message != "still thinking" {
		t.Errorf("open offer came back as %s %q", got.Status, got.Message)
	}

	settled, err := replayed.offerByID(accepted.ID)
	if err != nil {
		t.Fatalf("the accepted offer did not survive replay: %v", err)
	}
	if settled.Status != marketplace.OfferAccepted {
		t.Errorf("accepted offer came back as %s", settled.Status)
	}
	if settled.SettlesAt == 0 {
		t.Error("the settlement deadline was lost in replay, so the window would never close")
	}
	if !replayed.Status().LedgerBalance {
		t.Error("the replayed ledger does not balance")
	}
}

func TestWantAdSurvivesBeingStored(t *testing.T) {
	// Side was missing from packedListing at first, so a want-ad came back
	// as a SELL and acceptance paid the wrong way round - the one bug in
	// this feature that silently moves money in the wrong direction.
	h := newOfferHousehold(t)
	l := h.want(t, h.alice, "Bake cookies", 25)

	stored, ok := h.svc.Listing(l.ID)
	if !ok {
		t.Fatal("listing vanished")
	}
	if stored.Side != marketplace.SideBuy {
		t.Fatalf("a want-ad came back as %s", stored.Side)
	}

	// And after a reboot.
	replayed := h.replay(t)
	again, ok := replayed.Listing(l.ID)
	if !ok {
		t.Fatal("listing vanished in replay")
	}
	if again.Side != marketplace.SideBuy {
		t.Errorf("after replay a want-ad came back as %s", again.Side)
	}
}

// A USD posting must come back as USD after a reboot.
//
// The journal encodes postings itself, separately from the packed in-RAM
// record, so tagging the struct is not enough: a currency the wire format
// drops replays as NanaCoin and silently rewrites the household's money. That
// failure is invisible - nothing errors, the ledger still balances, and the
// dollars have simply become coins.
func TestCurrencySurvivesTheJournal(t *testing.T) {
	h := newOfferHousehold(t)

	// A dollar issuance: USD enters the household from its issuance account,
	// mirroring how coins do.
	txn := &ledger.Transaction{
		ID:        h.svc.book.NextID(),
		Kind:      ledger.KindIssue,
		CreatedAt: h.svc.Now(),
		Actor:     h.nana.ID,
		Postings: []ledger.Posting{
			{Account: ledger.USDIssuance, Amount: -500, Currency: ledger.USD},
			{Account: h.alice.Account, Amount: 500, Currency: ledger.USD},
		},
	}
	if err := h.svc.book.Validate(txn, false); err != nil {
		t.Fatalf("a balanced USD issuance was refused: %v", err)
	}
	if err := h.svc.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		t.Fatalf("commit: %v", err)
	}

	replayed := h.replay(t)

	found, _, ok := replayed.Transaction(txn.ID)
	if !ok {
		t.Fatal("the USD transaction did not survive replay")
	}
	for i, p := range found.Postings {
		if p.Currency != ledger.USD {
			t.Errorf("posting %d replayed as %s, want USD - dollars became coins", i, p.Currency)
		}
	}
	if !replayed.Status().LedgerBalance {
		t.Error("the replayed ledger does not balance")
	}
}
