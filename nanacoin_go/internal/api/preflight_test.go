//go:build !nanacoin_nologs

package api

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

// Every path this API answers must also answer OPTIONS with CORS headers.
//
// This exists because of a real failure. The board's httphi adapter
// registered OPTIONS only for non-GET paths, on the reasoning that a GET does
// not need a preflight. That is wrong: a GET carrying an Authorization header
// is not a CORS "simple request", so the browser preflights it. /me and
// /status therefore 404'd on OPTIONS, with no CORS headers on a 404, and the
// browser reported a missing Access-Control-Allow-Origin - which reads like a
// CORS misconfiguration rather than a missing route.
//
// The desktop mux answers OPTIONS on every path through its middleware, so
// this test passes here by construction. Its value is as the specification
// the board adapter's route list has to satisfy: the paths listed here are the
// paths boardhttp.Register must preflight.
func TestEveryPathAnswersPreflight(t *testing.T) {
	h := newHarness(t)

	const origin = "https://nanacoin.example.net"

	// Every path shape the API serves. Kept in step with routes.go and with
	// boardhttp.Register by hand; a path missing from any of the three is a
	// bug in that one.
	paths := []string{
		"/api/v1/status",
		"/api/v1/logs",
		"/api/v1/provision",
		"/api/v1/auth/authorize",
		"/api/v1/auth/token",
		"/api/v1/auth/logout",
		"/api/v1/me",
		"/api/v1/users",
		"/api/v1/users/user-1",
		"/api/v1/accounts/account-1",
		"/api/v1/accounts/account-1/transactions",
		"/api/v1/transfers",
		"/api/v1/transactions",
		"/api/v1/transactions/txn-1",
		"/api/v1/transactions/txn-1/reverse",
		"/api/v1/admin/issue",
		"/api/v1/admin/retire",
		"/api/v1/admin/config",
		"/api/v1/listings",
		"/api/v1/listings/listing-1",
		"/api/v1/listings/listing-1/purchase",
		"/api/v1/listings/listing-1/cancel",
	}

	for _, path := range paths {
		t.Run(path, func(t *testing.T) {
			r := httptest.NewRequest("OPTIONS", path, nil)
			r.Header.Set("Origin", origin)
			// What a browser actually sends when preflighting an
			// authenticated request.
			r.Header.Set("Access-Control-Request-Method", "GET")
			r.Header.Set("Access-Control-Request-Headers", "authorization")

			w := httptest.NewRecorder()
			h.srv.ServeHTTP(w, r)

			// A 404 here is the exact bug this test guards: the browser sees
			// no CORS headers on it and blames CORS, not routing.
			if w.Code == http.StatusNotFound {
				t.Fatalf("OPTIONS %s returned 404 - no preflight route", path)
			}
			if w.Code != http.StatusNoContent && w.Code != http.StatusOK {
				t.Errorf("OPTIONS %s returned %d, want 204 or 200", path, w.Code)
			}
			if got := w.Header().Get("Access-Control-Allow-Origin"); got != origin {
				t.Errorf("OPTIONS %s: Access-Control-Allow-Origin is %q, want %q", path, got, origin)
			}
			if got := w.Header().Get("Access-Control-Allow-Headers"); !strings.Contains(got, "Authorization") {
				t.Errorf("OPTIONS %s: Allow-Headers %q does not permit Authorization", path, got)
			}
		})
	}
}

// An authenticated GET is the request that caught this out, so it gets its own
// check: the response a browser reads must carry the origin header, not just
// the preflight.
func TestAuthenticatedGetCarriesCORSHeaders(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	const origin = "https://nanacoin.example.net"

	for _, path := range []string{"/api/v1/me", "/api/v1/status", "/api/v1/listings"} {
		r := httptest.NewRequest("GET", path, nil)
		r.Header.Set("Origin", origin)
		r.Header.Set("Authorization", "Bearer "+alice)

		w := httptest.NewRecorder()
		h.srv.ServeHTTP(w, r)

		if w.Code != http.StatusOK {
			t.Errorf("GET %s returned %d, want 200: %s", path, w.Code, w.Body.String())
			continue
		}
		if got := w.Header().Get("Access-Control-Allow-Origin"); got != origin {
			t.Errorf("GET %s: Access-Control-Allow-Origin is %q, want %q", path, got, origin)
		}
	}
}

// The event log is the board's only window: without it a browser error cannot
// distinguish "the request never arrived" from "it arrived and was refused".
// These pin the two facts a reader depends on.
func TestLogsRecordRequestsAndTheirStatus(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	// Something that succeeds and something that is refused.
	h.decodeInto(h.do("GET", "/api/v1/me", alice, nil), http.StatusOK, nil)
	if w := h.do("GET", "/api/v1/transactions", alice, nil); w.Code != http.StatusForbidden {
		t.Fatalf("expected the ledger to be refused to a non-Nana, got %d", w.Code)
	}

	var logs struct {
		Events []struct {
			Level  string `json:"level"`
			Kind   string `json:"kind"`
			Detail string `json:"detail"`
		} `json:"events"`
		Total uint64 `json:"total"`
	}
	h.decodeInto(h.do("GET", "/api/v1/logs", "", nil), http.StatusOK, &logs)

	var sawOK, sawForbidden bool
	for _, e := range logs.Events {
		if e.Kind == "200" && strings.Contains(e.Detail, "/api/v1/me") {
			sawOK = true
		}
		if e.Kind == "403" && strings.Contains(e.Detail, "/api/v1/transactions") {
			sawForbidden = true
			if e.Level != "warn" {
				t.Errorf("a 403 was logged at level %q, want warn", e.Level)
			}
		}
	}
	if !sawOK {
		t.Error("the successful GET /me was not logged")
	}
	if !sawForbidden {
		t.Error("the refused GET /transactions was not logged")
	}
	if logs.Total == 0 {
		t.Error("total is zero despite logged events")
	}
}

// A refused origin is the case the browser reports most misleadingly, so the
// log has to name it.
func TestLogsRecordRefusedOrigins(t *testing.T) {
	h := newHarness(t)

	r := httptest.NewRequest("GET", "/api/v1/status", nil)
	r.Header.Set("Origin", "http://not-allowed.example")
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	var logs struct {
		Events []struct {
			Kind   string `json:"kind"`
			Detail string `json:"detail"`
		} `json:"events"`
	}
	h.decodeInto(h.do("GET", "/api/v1/logs", "", nil), http.StatusOK, &logs)

	for _, e := range logs.Events {
		if e.Kind == "cors-refuse" && e.Detail == "http://not-allowed.example" {
			return
		}
	}
	t.Error("the refused origin was not logged")
}

// Reading the log must not fill it, or one look would evict what you came for.
func TestReadingLogsDoesNotLogItself(t *testing.T) {
	h := newHarness(t)

	var first, second struct {
		Total uint64 `json:"total"`
	}
	h.decodeInto(h.do("GET", "/api/v1/logs", "", nil), http.StatusOK, &first)
	h.decodeInto(h.do("GET", "/api/v1/logs", "", nil), http.StatusOK, &second)

	if second.Total != first.Total {
		t.Errorf("reading the log added %d events", second.Total-first.Total)
	}
}

// The client chooses the limit, so the server has to be able to refuse to
// honour a large one. On the board this is what stands between a ledger page
// and an out-of-memory.
func TestPageSizeIsCappedServerSide(t *testing.T) {
	h := newHarness(t)

	// A deliberately tiny cap, as the board sets.
	const cap = 3
	h.srv = NewServer(h.svc, h.sessions, Config{
		AllowedOrigins: []string{"https://nanacoin.example.net"},
		AllowProvision: true,
		MaxPageSize:    cap,
	}).Handler()

	nana, alice, _, _, bobAcct := h.setup()

	// More transactions than the cap allows.
	for i := 0; i < 6; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice,
			transferRequest{To: acct(bobAcct), Amount: 1, Memo: "x"}), http.StatusCreated, nil)
	}

	var page struct {
		Transactions []transactionView `json:"transactions"`
	}
	// Ask for far more than the cap.
	h.decodeInto(h.do("GET", "/api/v1/transactions?limit=500", nana, nil), http.StatusOK, &page)

	if len(page.Transactions) > cap {
		t.Errorf("asked for 500 and got %d transactions, want at most %d",
			len(page.Transactions), cap)
	}
	// And it must still return something: clamping, not refusing.
	if len(page.Transactions) == 0 {
		t.Error("clamping produced an empty page")
	}
}

// A limit below the cap is honoured, so a client can still ask for less.
func TestPageSizeHonoursSmallerRequests(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, bobAcct := h.setup()

	for i := 0; i < 5; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice,
			transferRequest{To: acct(bobAcct), Amount: 1, Memo: "x"}), http.StatusCreated, nil)
	}

	var page struct {
		Transactions []transactionView `json:"transactions"`
	}
	h.decodeInto(h.do("GET", "/api/v1/transactions?limit=2", nana, nil), http.StatusOK, &page)

	if len(page.Transactions) != 2 {
		t.Errorf("asked for 2 and got %d", len(page.Transactions))
	}
}

// The health header is the diagnostic that survives a crash: once the board
// is out of memory it cannot serve /logs either, so the headers of the last
// request that succeeded are the final reading anyone gets. It therefore has
// to be on every response, and readable cross-origin.
func TestHealthHeaderOnEveryResponse(t *testing.T) {
	h := newHarness(t)
	h.svc.Close()

	// A host that reports health, as the board does.
	log := eventlog.New(func() int64 { return 0 })

	svc, err := core.New(memory.New(), core.Options{})
	if err != nil {
		t.Fatalf("core.New: %v", err)
	}
	sessions := auth.NewStore(auth.Options{})
	server := NewServer(svc, sessions, Config{
		AllowedOrigins: []string{"https://nanacoin.example.net"},
		AllowProvision: true,
		Log:            log,
	})
	// Registered on the server, not on the log: the header must survive a
	// build with no event log at all.
	server.SetHealth(func() string { return "heap inuse 164000, delta 0" })
	srv := server.Handler()

	const origin = "https://nanacoin.example.net"

	for _, tc := range []struct {
		name   string
		method string
		path   string
	}{
		// A success and a refusal: the reading matters most on the failures.
		{"public status", "GET", "/api/v1/status"},
		{"unauthorized", "GET", "/api/v1/me"},
		{"not found", "GET", "/api/v1/transactions/nope"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			r := httptest.NewRequest(tc.method, tc.path, nil)
			r.Header.Set("Origin", origin)
			w := httptest.NewRecorder()
			srv.ServeHTTP(w, r)

			if got := w.Header().Get("X-Nanacoin-Health"); got == "" {
				t.Errorf("%s %s (status %d) carried no X-Nanacoin-Health header",
					tc.method, tc.path, w.Code)
			}
			// A header the browser cannot read is no use to the client.
			if got := w.Header().Get("Access-Control-Expose-Headers"); !strings.Contains(got, "X-Nanacoin-Health") {
				t.Errorf("Access-Control-Expose-Headers is %q, so the page cannot read the health header", got)
			}
		})
	}
}

// A host with no health to report must not emit an empty header.
func TestNoHealthHeaderWithoutAReporter(t *testing.T) {
	h := newHarness(t)

	r := httptest.NewRequest("GET", "/api/v1/status", nil)
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	if got := w.Header().Get("X-Nanacoin-Health"); got != "" {
		t.Errorf("a host with no reporter emitted %q", got)
	}
}
