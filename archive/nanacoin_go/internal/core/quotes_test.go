package core

import (
	"errors"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// fx is an offerHousehold with dollars in it.
type fx struct {
	*offerHousehold
}

func newFX(t *testing.T) *fx {
	t.Helper()
	h := newOfferHousehold(t)
	// Dollars enter the same way coins do: Nana issues them from the issuance
	// account, and every one is accounted for by the same whole-book property.
	for _, u := range []*users.User{h.alice, h.bob} {
		if _, err := h.svc.IssueUSD(h.nana, u.Account, 1000, "opening dollars"); err != nil {
			t.Fatalf("issuing dollars to %s: %v", u.Username, err)
		}
	}
	return &fx{h}
}

func (f *fx) usd(u *users.User) ledger.Amount {
	return f.svc.USDBalance(u.Account)
}

func TestDollarsEnterThroughIssuance(t *testing.T) {
	f := newFX(t)

	if got := f.usd(f.alice); got != 1000 {
		t.Errorf("alice holds %d cents, want 1000", got)
	}
	// Every dollar accounted for: the issuance account's negative balance is
	// exactly what everyone else holds.
	if got := f.svc.USDHeld(); got != 2000 {
		t.Errorf("household holds %d cents, want 2000", got)
	}
	if !f.svc.Status().LedgerBalance {
		t.Error("the ledger does not balance after issuing dollars")
	}
}

func TestOnlyNanaIssuesDollars(t *testing.T) {
	f := newFX(t)
	if _, err := f.svc.IssueUSD(f.alice, f.bob.Account, 100, "nope"); !errors.Is(err, ErrForbidden) {
		t.Errorf("got %v, want ErrForbidden", err)
	}
}

func TestDollarsAndCoinsDoNotMix(t *testing.T) {
	// The property the whole design rests on: a dollar balance and a coin
	// balance are separate numbers, and neither can be spent as the other.
	f := newFX(t)

	coinsBefore := f.balance(f.alice)
	if _, err := f.svc.IssueUSD(f.nana, f.alice.Account, 500, "more dollars"); err != nil {
		t.Fatalf("issue: %v", err)
	}

	if got := f.balance(f.alice); got != coinsBefore {
		t.Errorf("issuing dollars changed the coin balance from %d to %d", coinsBefore, got)
	}
	if got := f.usd(f.alice); got != 1500 {
		t.Errorf("alice holds %d cents, want 1500", got)
	}
}

// --- quoting ----------------------------------------------------------------

func TestPostQuoteMovesNoMoney(t *testing.T) {
	f := newFX(t)
	coins, cents := f.balance(f.alice), f.usd(f.alice)

	if _, err := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0); err != nil {
		t.Fatalf("post: %v", err)
	}

	if f.balance(f.alice) != coins || f.usd(f.alice) != cents {
		t.Error("posting a quote moved money; it is an advertisement, not a trade")
	}
}

func TestQuoteRejectsImplausibleRates(t *testing.T) {
	f := newFX(t)
	for _, tc := range []struct {
		name        string
		rate, coins ledger.Amount
	}{
		{"zero rate", 0, 10},
		{"negative rate", -5, 10},
		{"absurd rate", MaxCentsPerCoin + 1, 10},
		{"zero coins", 25, 0},
		{"absurd size", 25, MaxQuoteCoins + 1},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := f.svc.PostQuote(f.alice, marketplace.Ask, tc.rate, tc.coins, 0); err == nil {
				t.Error("accepted an implausible quote")
			}
		})
	}
}

// The direction of money is the whole feature. Getting it backwards would move
// coins and dollars the wrong way, so both sides are checked explicitly.
func TestTakingAnAskBuysCoinsWithDollars(t *testing.T) {
	f := newFX(t)

	// Alice sells 10 coins at 25 cents each: $2.50 for 10 coins.
	q, err := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)
	if err != nil {
		t.Fatalf("post: %v", err)
	}

	aliceCoins, aliceCents := f.balance(f.alice), f.usd(f.alice)
	bobCoins, bobCents := f.balance(f.bob), f.usd(f.bob)

	if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID); err != nil {
		t.Fatalf("take: %v", err)
	}

	// Alice sold coins and received cents.
	if got := f.balance(f.alice); got != aliceCoins-10 {
		t.Errorf("alice has %d coins, want %d", got, aliceCoins-10)
	}
	if got := f.usd(f.alice); got != aliceCents+250 {
		t.Errorf("alice has %d cents, want %d", got, aliceCents+250)
	}
	// Bob bought coins and paid cents.
	if got := f.balance(f.bob); got != bobCoins+10 {
		t.Errorf("bob has %d coins, want %d", got, bobCoins+10)
	}
	if got := f.usd(f.bob); got != bobCents-250 {
		t.Errorf("bob has %d cents, want %d", got, bobCents-250)
	}
	if !f.svc.Status().LedgerBalance {
		t.Error("the ledger does not balance after a trade")
	}
}

func TestTakingABidSellsCoinsForDollars(t *testing.T) {
	f := newFX(t)

	// Alice bids: she will pay 30 cents each for 10 coins.
	q, err := f.svc.PostQuote(f.alice, marketplace.Bid, 30, 10, 0)
	if err != nil {
		t.Fatalf("post: %v", err)
	}

	aliceCoins, aliceCents := f.balance(f.alice), f.usd(f.alice)
	bobCoins, bobCents := f.balance(f.bob), f.usd(f.bob)

	if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID); err != nil {
		t.Fatalf("take: %v", err)
	}

	// The other way round from an ask: Alice receives coins and pays cents.
	if got := f.balance(f.alice); got != aliceCoins+10 {
		t.Errorf("alice has %d coins, want %d", got, aliceCoins+10)
	}
	if got := f.usd(f.alice); got != aliceCents-300 {
		t.Errorf("alice has %d cents, want %d", got, aliceCents-300)
	}
	if got := f.balance(f.bob); got != bobCoins-10 {
		t.Errorf("bob has %d coins, want %d", got, bobCoins-10)
	}
	if got := f.usd(f.bob); got != bobCents+300 {
		t.Errorf("bob has %d cents, want %d", got, bobCents+300)
	}
}

func TestBothLegsAreRecorded(t *testing.T) {
	f := newFX(t)
	q, _ := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)

	filled, coin, cash, err := f.svc.TakeQuote(f.bob, q.ID)
	if err != nil {
		t.Fatalf("take: %v", err)
	}

	if filled.Status != marketplace.QuoteFilled {
		t.Errorf("quote is %s, want FILLED", filled.Status)
	}
	// Both legs named, so a half-landed trade is findable: a quote with a
	// coin leg and no cash leg.
	if filled.CoinTx == "" || filled.CashTx == "" {
		t.Errorf("quote records coin=%q cash=%q; both are needed to audit a trade",
			filled.CoinTx, filled.CashTx)
	}
	if coin.ID != filled.CoinTx || cash.ID != filled.CashTx {
		t.Error("the returned transactions do not match the ones recorded on the quote")
	}
	// The legs cross-reference the quote, so the ledger explains itself.
	if coin.Reference != string(q.ID) || cash.Reference != string(q.ID) {
		t.Error("a leg does not reference its quote")
	}
}

func TestCannotTakeYourOwnQuote(t *testing.T) {
	f := newFX(t)
	q, _ := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)
	if _, _, _, err := f.svc.TakeQuote(f.alice, q.ID); !errors.Is(err, ErrSelfDeal) {
		t.Errorf("got %v, want ErrSelfDeal", err)
	}
}

func TestCannotTakeTwice(t *testing.T) {
	f := newFX(t)
	q, _ := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)
	if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID); err != nil {
		t.Fatalf("first take: %v", err)
	}
	if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID); !errors.Is(err, ErrQuoteClosed) {
		t.Errorf("got %v, want ErrQuoteClosed - a quote has no partial fills", err)
	}
}

func TestTradeRefusedWhenTheTakerCannotAfford(t *testing.T) {
	f := newFX(t)

	// More dollars than Bob holds: 1000 coins at 100 cents is $1000.
	q, err := f.svc.PostQuote(f.alice, marketplace.Ask, 100, 1000, 0)
	if err != nil {
		t.Fatalf("post: %v", err)
	}
	coins, cents := f.balance(f.bob), f.usd(f.bob)

	if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID); err == nil {
		t.Fatal("an unaffordable trade succeeded")
	}
	// Nothing moved: both legs are validated before either is committed.
	if f.balance(f.bob) != coins || f.usd(f.bob) != cents {
		t.Error("a refused trade moved money")
	}
	if got, _ := f.svc.Quote(q.ID); got.Status != marketplace.QuoteOpen {
		t.Errorf("the quote is %s after a refused take, want OPEN", got.Status)
	}
}

func TestExpiredQuotesCannotBeTaken(t *testing.T) {
	f := newFX(t)
	expiry := f.svc.Now() + 3600
	q, err := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, expiry)
	if err != nil {
		t.Fatalf("post: %v", err)
	}

	f.advance(2 * time.Hour)

	if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID); !errors.Is(err, ErrQuoteExpired) {
		t.Errorf("got %v, want ErrQuoteExpired", err)
	}
}

func TestCancelIsTheMakersOrNanas(t *testing.T) {
	f := newFX(t)

	q, _ := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)
	if _, err := f.svc.CancelQuote(f.bob, q.ID); !errors.Is(err, ErrForbidden) {
		t.Errorf("bob cancelling alice's quote gave %v, want ErrForbidden", err)
	}
	if _, err := f.svc.CancelQuote(f.alice, q.ID); err != nil {
		t.Errorf("the maker could not cancel: %v", err)
	}

	q2, _ := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)
	if _, err := f.svc.CancelQuote(f.nana, q2.ID); err != nil {
		t.Errorf("nana could not cancel: %v", err)
	}
}

// --- the book ---------------------------------------------------------------

func TestBookIsOrderedBestFirst(t *testing.T) {
	f := newFX(t)

	// Asks at three rates, posted out of order.
	for _, rate := range []ledger.Amount{30, 20, 25} {
		if _, err := f.svc.PostQuote(f.alice, marketplace.Ask, rate, 5, 0); err != nil {
			t.Fatalf("post ask %d: %v", rate, err)
		}
	}
	for _, rate := range []ledger.Amount{10, 18, 14} {
		if _, err := f.svc.PostQuote(f.bob, marketplace.Bid, rate, 5, 0); err != nil {
			t.Fatalf("post bid %d: %v", rate, err)
		}
	}

	var asks, bids []ledger.Amount
	f.svc.EachQuote(func(q *marketplace.Quote) bool {
		if q.Side == marketplace.Ask {
			asks = append(asks, q.CentsPerCoin)
		} else {
			bids = append(bids, q.CentsPerCoin)
		}
		return true
	})

	// Cheapest coins to buy first.
	if len(asks) != 3 || asks[0] != 20 || asks[2] != 30 {
		t.Errorf("asks came back %v, want ascending from 20", asks)
	}
	// Best price to sell into first.
	if len(bids) != 3 || bids[0] != 18 || bids[2] != 10 {
		t.Errorf("bids came back %v, want descending from 18", bids)
	}
}

func TestQuotesSurviveReplay(t *testing.T) {
	f := newFX(t)

	open, err := f.svc.PostQuote(f.alice, marketplace.Ask, 25, 10, 0)
	if err != nil {
		t.Fatalf("post: %v", err)
	}
	taken, _ := f.svc.PostQuote(f.alice, marketplace.Ask, 30, 5, 0)
	if _, _, _, err := f.svc.TakeQuote(f.bob, taken.ID); err != nil {
		t.Fatalf("take: %v", err)
	}

	replayed := f.replay(t)

	back, err := replayed.quoteByID(open.ID)
	if err != nil {
		t.Fatalf("the open quote did not survive replay: %v", err)
	}
	if back.Status != marketplace.QuoteOpen || back.CentsPerCoin != 25 || back.Coins != 10 {
		t.Errorf("open quote came back as %s %d@%d", back.Status, back.Coins, back.CentsPerCoin)
	}

	filled, err := replayed.quoteByID(taken.ID)
	if err != nil {
		t.Fatalf("the filled quote did not survive replay: %v", err)
	}
	if filled.Status != marketplace.QuoteFilled {
		t.Errorf("filled quote came back as %s", filled.Status)
	}
	if filled.CoinTx == "" || filled.CashTx == "" {
		t.Error("a leg was lost in replay; the trade is no longer auditable")
	}

	// And the money is where it should be, in both currencies.
	if replayed.Balance(f.bob.Account) != f.balance(f.bob) {
		t.Error("coin balances differ after replay")
	}
	if replayed.USDBalance(f.bob.Account) != f.usd(f.bob) {
		t.Error("dollar balances differ after replay")
	}
	if !replayed.Status().LedgerBalance {
		t.Error("the replayed ledger does not balance")
	}
}
