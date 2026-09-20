package core

import (
	"bytes"
	"errors"
	"fmt"
	"math/rand"
	"strings"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

func TestReceiptArenaMatchesFIFOModel(t *testing.T) {
	c := new(receiptCache)
	type item struct {
		key  string
		body []byte
	}
	var model []item
	r := rand.New(rand.NewSource(43))
	for i := 0; i < 500; i++ {
		key := fmt.Sprint(i)
		body := bytes.Repeat([]byte{byte(i)}, r.Intn(MaxIdempotencyBytes+50))
		c.remember("u", "transfer", key, body)
		if len(body) <= MaxIdempotencyBytes {
			used := 0
			for _, v := range model {
				used += len(v.body)
			}
			for len(model) > 0 && (len(model) == MaxIdempotencyEntries || used+len(body) > MaxIdempotencyBytes) {
				used -= len(model[0].body)
				model = model[1:]
			}
			model = append(model, item{key, append([]byte(nil), body...)})
		}
		clear(body) // Cache must own the bytes even when the caller reuses them.
		used := 0
		for _, v := range model {
			off, n, ok := c.find("u", "transfer", v.key)
			if !ok || !bytes.Equal(c.data[off:off+n], v.body) {
				t.Fatalf("receipt corrupted at %d key %s", i, v.key)
			}
			used += len(v.body)
		}
		if c.count != len(model) || c.used != used {
			t.Fatal("accounting drift")
		}
	}
}

func TestReceiptCopiesSurviveEvictionAndCallerMutation(t *testing.T) {
	s := newService(t, memory.New())
	var first, retry [128]byte
	copy(first[:], "original")
	_, err := s.IdempotentInto("u", "transfer", "k", first[:], func() ([]byte, error) { return first[:8], nil })
	if err != nil {
		t.Fatal(err)
	}
	copy(first[:], "modified")
	got, err := s.IdempotentInto("u", "transfer", "k", retry[:], func() ([]byte, error) { t.Fatal("duplicate executed"); return nil, nil })
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < MaxIdempotencyEntries*3; i++ {
		_, err := s.Idempotent("u", "transfer", fmt.Sprint(i), func() ([]byte, error) { return []byte("replacement"), nil })
		if err != nil {
			t.Fatal(err)
		}
	}
	if string(got) != "original" {
		t.Fatal("response changed during eviction", string(got))
	}
}

func TestReceiptIdentitySeparatesFields(t *testing.T) {
	c := new(receiptCache)
	c.remember("a|b", "c", "d", []byte("first"))
	c.remember("a", "b|c", "d", []byte("second"))
	for _, x := range []struct {
		u    ledger.UserID
		e, w string
	}{{"a|b", "c", "first"}, {"a", "b|c", "second"}} {
		o, n, ok := c.find(x.u, x.e, "d")
		if !ok || string(c.data[o:o+n]) != x.w {
			t.Fatal("key components collided")
		}
	}
}

func TestReceiptChurnDoesNotAllocate(t *testing.T) {
	c := new(receiptCache)
	keys := make([]string, MaxIdempotencyEntries+1)
	for i := range keys {
		keys[i] = fmt.Sprint(i)
	}
	i := 0
	body := []byte("receipt")
	if n := testing.AllocsPerRun(200, func() { c.remember("u", "issue", keys[i%len(keys)], body); i++ }); n != 0 {
		t.Fatal(n)
	}
}

func TestOversizedCommitFailsBeforeJournalOrState(t *testing.T) {
	j := memory.New()
	s := newService(t, j)
	before := s.Config()
	size, _ := j.Size()
	e := configUpdatedEvent{Config: Config{HouseholdName: strings.Repeat("x", wireCapacity+1)}}
	if err := s.commitEvent(storage.TypeConfigUpdated, &e); err == nil {
		t.Fatal("oversized event accepted")
	}
	after, _ := j.Size()
	if s.Config() != before || after != size {
		t.Fatal("failed commit changed state")
	}
}

func TestFixedCommitBufferAndTypedReplayAgree(t *testing.T) {
	j := memory.New()
	s := newService(t, j)
	nana, alice, bob := household(t, s)
	var out WriteResult
	for i := 0; i < 20; i++ {
		if _, err := s.Issue(nana, alice.Account, 2, "issued", &out); err != nil {
			t.Fatal(err)
		}
		if _, err := s.Transfer(alice, bob.Account, 1, "paid", &out); err != nil {
			t.Fatal(err)
		}
	}
	r := newService(t, j)
	if r.Balance(alice.Account) != s.Balance(alice.Account) || r.Balance(bob.Account) != s.Balance(bob.Account) || r.Status().Transactions != s.Status().Transactions {
		t.Fatal("live application differs from replay")
	}
	if len(s.wireBuf) != wireCapacity || cap(s.wireBuf) != wireCapacity {
		t.Fatal("wire buffer grew")
	}
}

func TestDiscardCommitFailureDoesNotMoveMoney(t *testing.T) {
	j := memory.NewDiscarding()
	s := newService(t, j)
	nana, alice, _ := household(t, s)
	if s.wireBuf != nil {
		t.Fatal("discard backend reserved unused encoding buffer")
	}
	before := s.Balance(alice.Account)
	count := s.Status().Transactions
	j.SetFailure(j.Appends(), errors.New("append failed"))
	if _, err := s.Issue(nana, alice.Account, 1, "failure"); err == nil {
		t.Fatal("ignored append failure")
	}
	if s.Balance(alice.Account) != before || s.Status().Transactions != count {
		t.Fatal("failed commit moved money")
	}
}

func TestListingCurrencyBoundForEveryKind(t *testing.T) {
	for _, kind := range []string{"", "currency"} {
		if err := validateListingInput(ListingInput{Title: "offer", Price: 1, Kind: kind, Currency: strings.Repeat("x", 9), MinorUnits: 1}); err == nil {
			t.Fatalf("kind %q accepted unbounded currency", kind)
		}
	}
}

func TestListingTextPressureRejectsBeforeCommitWithoutLosingID(t *testing.T) {
	j := memory.New()
	s := newService(t, j)
	_, alice, _ := household(t, s)
	existing, err := s.CreateListing(alice, ListingInput{Title: "original", Price: 1})
	if err != nil {
		t.Fatal(err)
	}
	// Consume all remaining text blocks as other domain/ledger records can.
	var occupied []ledger.Slot
	for {
		slot := s.store.arena.Put("0123456789abcdef")
		if slot.IsEmpty() {
			break
		}
		occupied = append(occupied, slot)
	}
	before, _ := j.Size()
	_, err = s.CreateListing(alice, ListingInput{Title: "new", Description: strings.Repeat("x", 500), Price: 1})
	if !errors.Is(err, ErrCapacity) {
		t.Fatalf("expected clean capacity refusal, got %v", err)
	}
	bigger := strings.Repeat("y", 500)
	if _, err = s.UpdateListing(alice, existing.ID, nil, &bigger, nil); !errors.Is(err, ErrCapacity) {
		t.Fatal(err)
	}
	after, _ := j.Size()
	got, ok := s.Listing(existing.ID)
	if !ok || got.Title != "original" || got.Description != "" || before != after {
		t.Fatal("refusal changed existing listing or committed unusable ID")
	}
	for _, slot := range occupied {
		s.store.arena.Release(slot)
	}
	if _, err := s.CreateListing(alice, ListingInput{Title: "new", Description: bigger, Price: 1}); err != nil {
		t.Fatal(err)
	}
}

func TestClosedListingRecycledUnderTextPressureBeforeRowLimit(t *testing.T) {
	s := newService(t, memory.NewDiscarding())
	_, alice, _ := household(t, s)
	// An older tiny offer cannot free enough space. It must not prevent
	// recycling a later closed offer that does have sufficient text storage.
	tiny, err := s.CreateListing(alice, ListingInput{Title: "tiny", Price: 1})
	if err != nil {
		t.Fatal(err)
	}
	if _, err = s.CancelListing(alice, tiny.ID); err != nil {
		t.Fatal(err)
	}
	old, err := s.CreateListing(alice, ListingInput{Title: "old", Description: strings.Repeat("x", 500), Price: 1})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.CancelListing(alice, old.ID); err != nil {
		t.Fatal(err)
	}
	for {
		if s.store.arena.Put("0123456789abcdef").IsEmpty() {
			break
		}
	}
	next, err := s.CreateListing(alice, ListingInput{Title: "new", Description: strings.Repeat("y", 500), Price: 1})
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := s.Listing(old.ID); ok {
		t.Fatal("closed history was not recycled")
	}
	if got, ok := s.Listing(next.ID); !ok || got.Description != strings.Repeat("y", 500) {
		t.Fatal("new listing text was truncated")
	}
}

// Reproduces the board sequence: a full ledger followed by sustained market
// churn. Closed offers must remain reusable even before the row limit fills.
func TestMarketChurnAfterFullLedgerPreservesListingIDs(t *testing.T) {
	s := newService(t, memory.NewDiscarding())
	nana, alice, bob := household(t, s)
	var out WriteResult
	for i := 0; i < 1000; i++ {
		if _, err := s.Issue(nana, alice.Account, 1, "Locust issuance", &out); err != nil {
			t.Fatal(err)
		}
	}
	tiny, err := s.CreateListing(bob, ListingInput{Title: "tiny", Price: 1})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err = s.Purchase(alice, tiny.ID, &out); err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 300; i++ {
		offer, err := s.CreateListing(bob, ListingInput{Title: "Locust item", Description: strings.Repeat("x", 200), Price: 1})
		if err != nil {
			t.Fatalf("create %d: %v", i, err)
		}
		bought, _, err := s.Purchase(alice, offer.ID, &out)
		if err != nil || bought.ID != offer.ID || bought.Description != offer.Description {
			t.Fatalf("purchase %d corrupted: %v", i, err)
		}
	}
	if err := s.book.CheckInvariants(); err != nil {
		t.Fatal(err)
	}
}

func TestMarketChurnWithProductionIDsAndUptimeClock(t *testing.T) {
	var seconds int64 = 1
	s, err := New(memory.NewDiscarding(), Options{Now: func() time.Time { seconds++; return time.Unix(seconds, 0) }})
	if err != nil {
		t.Fatal(err)
	}
	nana, alice, bob := household(t, s)
	if _, err := s.Issue(nana, alice.Account, 10000, "funding"); err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 300; i++ {
		l, err := s.CreateListing(bob, ListingInput{Title: "Locust item", Description: strings.Repeat("x", 200), Price: 1})
		if err != nil {
			t.Fatalf("create %d: %v", i, err)
		}
		if _, _, err := s.Purchase(alice, l.ID); err != nil {
			t.Fatalf("purchase %d: %v", i, err)
		}
	}
}
