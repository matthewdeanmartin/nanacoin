package api

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

// What one request costs in allocations.
//
// This is the measurement the board's "fatal error: out of memory" needed and
// nobody had taken. The board has roughly 120 KB of heap spare after setup, so
// a per-request cost in the tens of KB is the difference between serving and
// dying - and the numbers differ enormously between endpoints, which is why
// POST /listings worked while GET history did not.
//
// Run with:
//
//	go test ./internal/api/ -run XXX -bench BenchmarkPerRequestAllocation -benchmem
func BenchmarkPerRequestAllocation(b *testing.B) {
	h := newHarness(b)
	nana, alice, bob, aliceAcct, bobAcct := h.setup()

	// Give offers something to render. An empty table measures nothing: the
	// first version of this endpoint allocated per *offer*, so the cost only
	// shows up with rows in it.
	{
		body, _ := json.Marshal(createListingRequest{Title: "Thing", Price: 5})
		r := httptest.NewRequest("POST", "/api/v1/listings", bytes.NewReader(body))
		r.Header.Set("Authorization", "Bearer "+alice)
		r.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		h.srv.ServeHTTP(w, r)
		var created listingView
		_ = json.Unmarshal(w.Body.Bytes(), &created)

		for i := 0; i < 10; i++ {
			ob, _ := json.Marshal(offerRequest{Amount: int64(i + 1), Message: "offer"})
			or := httptest.NewRequest("POST", "/api/v1/listings/"+string(created.ID)+"/offers",
				bytes.NewReader(ob))
			or.Header.Set("Authorization", "Bearer "+bob)
			or.Header.Set("Content-Type", "application/json")
			or.Header.Set("Idempotency-Key", "offer-"+string(rune('a'+i)))
			h.srv.ServeHTTP(httptest.NewRecorder(), or)
		}
	}

	// Give the exchange book something to render, and give both sides
	// dollars so a take can actually settle. An empty book measures nothing:
	// the sort in EachQuote is over live entries, so the cost only appears
	// once there are rows to order.
	{
		for _, who := range []string{aliceAcct, bobAcct} {
			ib, _ := json.Marshal(issueUSDRequest{To: acct(who), Cents: 100_000, Reason: "float"})
			ir := httptest.NewRequest("POST", "/api/v1/admin/issue-usd", bytes.NewReader(ib))
			ir.Header.Set("Authorization", "Bearer "+nana)
			ir.Header.Set("Content-Type", "application/json")
			ir.Header.Set("Idempotency-Key", "usd-"+who)
			h.srv.ServeHTTP(httptest.NewRecorder(), ir)
		}
		// Both sides of the book, so the ordering work is real.
		for i := 0; i < 8; i++ {
			side, tok := "ASK", alice
			if i%2 == 1 {
				side, tok = "BID", bob
			}
			qb, _ := json.Marshal(quoteRequest{
				Side: side, CentsPerCoin: int64(20 + i), Coins: 1,
			})
			qr := httptest.NewRequest("POST", "/api/v1/quotes", bytes.NewReader(qb))
			qr.Header.Set("Authorization", "Bearer "+tok)
			qr.Header.Set("Content-Type", "application/json")
			h.srv.ServeHTTP(httptest.NewRecorder(), qr)
		}
	}

	// Give history something to render.
	for i := 0; i < 20; i++ {
		body, _ := json.Marshal(transferRequest{To: acct(bobAcct), Amount: 1, Memo: "chore"})
		r := httptest.NewRequest("POST", "/api/v1/transfers", bytes.NewReader(body))
		r.Header.Set("Authorization", "Bearer "+alice)
		r.Header.Set("Content-Type", "application/json")
		h.srv.ServeHTTP(httptest.NewRecorder(), r)
	}

	cases := []struct {
		name   string
		method string
		path   string
		token  string
		body   any
	}{
		{"status", "GET", "/api/v1/status", "", nil},
		{"me", "GET", "/api/v1/me", alice, nil},
		{"listings", "GET", "/api/v1/listings", alice, nil},
		{"users", "GET", "/api/v1/users", nana, nil},
		{"history-50", "GET", "/api/v1/accounts/" + aliceAcct + "/transactions?limit=50", alice, nil},
		{"history-15", "GET", "/api/v1/accounts/" + aliceAcct + "/transactions?limit=15", alice, nil},
		{"ledger-50", "GET", "/api/v1/transactions?limit=50", nana, nil},
		{"logs", "GET", "/api/v1/logs?limit=50", "", nil},
		{"offers", "GET", "/api/v1/offers", alice, nil},
		{"quotes", "GET", "/api/v1/quotes", alice, nil},
	}

	for _, tc := range cases {
		b.Run(tc.name, func(b *testing.B) {
			var body []byte
			if tc.body != nil {
				body, _ = json.Marshal(tc.body)
			}
			b.ReportAllocs()
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				var r = httptest.NewRequest(tc.method, tc.path, bytes.NewReader(body))
				if tc.token != "" {
					r.Header.Set("Authorization", "Bearer "+tc.token)
				}
				if body != nil {
					r.Header.Set("Content-Type", "application/json")
				}
				h.srv.ServeHTTP(httptest.NewRecorder(), r)
			}
		})
	}
}

// The response sizes themselves, which is what the board's buffers have to
// hold. Reported so the board's MaxResponseBytes and page caps can be set
// from measurements rather than guesses.
func TestResponseSizes(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, aliceAcct, bobAcct := h.setup()

	for i := 0; i < 20; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice,
			transferRequest{To: acct(bobAcct), Amount: 1, Memo: "chore"}), 201, nil)
	}

	for _, tc := range []struct{ name, path, token string }{
		{"status", "/api/v1/status", ""},
		{"me", "/api/v1/me", alice},
		{"users", "/api/v1/users", nana},
		{"listings", "/api/v1/listings", alice},
		{"history-15", "/api/v1/accounts/" + aliceAcct + "/transactions?limit=15", alice},
		{"history-50", "/api/v1/accounts/" + aliceAcct + "/transactions?limit=50", alice},
		{"ledger-50", "/api/v1/transactions?limit=50", nana},
		{"logs-50", "/api/v1/logs?limit=50", ""},
		{"quotes", "/api/v1/quotes", alice},
	} {
		w := h.do("GET", tc.path, tc.token, nil)
		t.Logf("%-12s %d bytes (status %d)", tc.name, w.Body.Len(), w.Code)
	}
}
