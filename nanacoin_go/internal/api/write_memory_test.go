package api

import (
	"bytes"
	"encoding/json"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"math"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

func TestPurchaseResponseFitsFixedBufferAndMatchesOldEncoder(t *testing.T) {
	h := newHarness(t)
	nana, err := h.svc.Provision("nana", "Nana", "nana-pin", "House")
	if err != nil {
		t.Fatal(err)
	}
	alice, err := h.svc.CreateUser(nana, "alice", strings.Repeat(`"`, core.MaxNameLen), "alice-pin", users.RoleUser, true)
	if err != nil {
		t.Fatal(err)
	}
	bob, err := h.svc.CreateUser(nana, "bob", strings.Repeat(`\`, core.MaxNameLen), "bob-pin", users.RoleUser, true)
	if err != nil {
		t.Fatal(err)
	}
	listing, err := h.svc.CreateListing(bob, core.ListingInput{Title: strings.Repeat(`"`, core.MaxTitleLen), Description: strings.Repeat(`\`, core.MaxDescriptionLen), Price: 1, Kind: "currency", Currency: strings.Repeat(`"`, 8), MinorUnits: 1 << 62})
	if err != nil {
		t.Fatal(err)
	}
	l, tx, err := h.svc.Purchase(alice, listing.ID)
	if err != nil {
		t.Fatal(err)
	}
	// Include production-length random IDs and widest numeric fields, not just
	// the short deterministic IDs used by the harness.
	l.ID = ledger.ListingID("listing-abcdefghijkl")
	tx.ID = ledger.TransactionID("txn-abcdefghijkl")
	l.SoldTx = tx.ID
	tx.Actor = ledger.UserID("user-abcdefghijkl")
	tx.Reference = string(l.ID)
	l.CreatedAt, l.UpdatedAt, tx.CreatedAt = math.MaxInt64, math.MaxInt64, math.MaxInt64
	l.Price, l.MinorUnits = 1_000_000_000, math.MaxInt64
	tx.Postings[0].Amount, tx.Postings[1].Amount = -1_000_000_000, 1_000_000_000
	b := new(recordBuffer)
	got, err := b.encodePurchase(namer{h.svc}, l, tx)
	if err != nil {
		t.Fatal("valid maximum fields do not fit", err)
	}
	v := purchaseResponse{Listing: namer{h.svc}.listing(l), Transaction: namer{h.svc}.transaction(tx)}
	want, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	var a, c any
	if err := json.Unmarshal(got, &a); err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(want, &c); err != nil {
		t.Fatal(err)
	}
	aa, _ := json.Marshal(a)
	cc, _ := json.Marshal(c)
	if !bytes.Equal(aa, cc) {
		t.Fatalf("response changed\n%s\n%s", got, want)
	}
	t.Logf("maximum escaped purchase response: %d / %d bytes", len(got), len(b.data))
}

func TestCapacityErrorRemainsReadable(t *testing.T) {
	w := httptest.NewRecorder()
	writeError(w, core.ErrCapacity)
	if w.Code != http.StatusInsufficientStorage || !strings.Contains(w.Body.String(), "storage_full") || !strings.Contains(w.Body.String(), "storage is full") {
		t.Fatal(w.Code, w.Body.String())
	}
}
