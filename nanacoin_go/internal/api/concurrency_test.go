package api

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
)

type stalledRecordWriter struct {
	*httptest.ResponseRecorder
	entered, release chan struct{}
	once             sync.Once
}

func (w *stalledRecordWriter) Write(p []byte) (int, error) {
	if bytes.Contains(p, []byte(`"id":`)) {
		w.once.Do(func() { close(w.entered); <-w.release })
	}
	return w.ResponseRecorder.Write(p)
}

func TestListSocketWritesDoNotHoldServiceLock(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, acct, _ := h.setup()
	h.decodeInto(h.do("POST", "/api/v1/listings", alice, createListingRequest{Title: "Item", Description: "Details", Price: 1}), 201, nil)
	for _, path := range []string{"/api/v1/users", "/api/v1/transactions", "/api/v1/accounts/" + acct + "/transactions", "/api/v1/listings"} {
		t.Run(path, func(t *testing.T) {
			w := &stalledRecordWriter{ResponseRecorder: httptest.NewRecorder(), entered: make(chan struct{}), release: make(chan struct{})}
			var once sync.Once
			unblock := func() { once.Do(func() { close(w.release) }) }
			defer unblock()
			r := httptest.NewRequest("GET", path, nil)
			r.Header.Set("Authorization", "Bearer "+nana)
			done := make(chan struct{})
			go func() { defer close(done); h.srv.ServeHTTP(w, r) }()
			select {
			case <-w.entered:
			case <-time.After(time.Second):
				t.Fatal("no record reached socket")
			}
			read := make(chan struct{})
			go func() { h.svc.Balance(ledger.AccountID(acct)); close(read) }()
			select {
			case <-read:
			case <-time.After(time.Second):
				t.Fatal("socket write blocks service")
			}
			unblock()
			select {
			case <-done:
			case <-time.After(time.Second):
				t.Fatal("stream stuck")
			}
			if !json.Valid(w.Body.Bytes()) {
				t.Fatalf("invalid JSON: %s", w.Body.String())
			}
		})
	}
}

func TestRecordBufferFitsEscapedListingAndRefusesOverflow(t *testing.T) {
	b := &recordBuffer{}
	sw := &streamWriter{}
	v := listingView{
		ID: ledger.ListingID(strings.Repeat("x", 32)), Seller: ledger.AccountID(strings.Repeat("x", 32)), Buyer: ledger.AccountID(strings.Repeat("x", 32)),
		SellerName: strings.Repeat(`"`, core.MaxNameLen), BuyerName: strings.Repeat(`"`, core.MaxNameLen),
		Title: strings.Repeat(`"`, core.MaxTitleLen), Description: strings.Repeat(`\`, core.MaxDescriptionLen),
		Price: 1<<63 - 1, CreatedAt: 1<<63 - 1, UpdatedAt: 1<<63 - 1, MinorUnits: 1<<63 - 1, SoldTx: "txn-4294967295", Kind: "money", Currency: "USD",
	}
	if !b.prepare(sw, func(j *jsonw) { j.listingView(&v) }) || !json.Valid(b.data[:b.n]) {
		t.Fatalf("valid maximum record doesn't fit: %v", sw.err)
	}
	if b.prepare(sw, func(j *jsonw) { j.str(strings.Repeat("x", RecordBufferBytes+1)) }) {
		t.Fatal("overflow silently accepted")
	}
}
