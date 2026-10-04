package api

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

// Where a request's cost goes, layer by layer.
//
// Read the "0-harness-only" line first. httptest.NewRequest plus
// NewRecorder allocate about 5.5 kB between them, so every figure below that
// includes them is mostly scaffolding. The server's own cost is measured in
// BenchmarkServerCost, which reuses one request and one writer: 256 bytes for
// a read and 1,539 for a transfer, not the ~7,000 these numbers suggest.
//
// This file is kept because the layer breakdown is still useful and because
// the harness line is worth having on the record - a benchmark that measures
// its own scaffolding is easy to write and hard to notice.
//
// Run with:
//
//	go test ./internal/api/ -run XXX -bench BenchmarkRequestLayers -benchmem
func BenchmarkRequestLayers(b *testing.B) {
	h := newHarness(b)
	_, alice, _, _, _ := h.setup()

	// 1. httptest's own request construction, which the real adapter does
	//    differently. Subtract this from the rest: it is the harness, not
	//    the server.
	b.Run("0-harness-only", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/me", nil)
			r.Header.Set("Authorization", "Bearer "+alice)
			w := httptest.NewRecorder()
			_, _ = r, w
		}
	})

	// 2. The session lookup, which hashes the token and reads a map.
	b.Run("1-session-lookup", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			if _, err := h.sessions.Lookup(alice); err != nil {
				b.Fatal(err)
			}
		}
	})

	// 3. Building the response view, without serialising it.
	b.Run("2-build-view", func(b *testing.B) {
		sess, err := h.sessions.Lookup(alice)
		if err != nil {
			b.Fatal(err)
		}
		u, ok := h.svc.User(sess.UserID)
		if !ok {
			b.Fatal("no such user")
		}
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			bal := h.svc.Balance(u.Account)
			_ = viewUser(u, &bal)
		}
	})

	// 4. Serialising it, which is what writeJSON does.
	b.Run("3-serialise-view", func(b *testing.B) {
		sess, err := h.sessions.Lookup(alice)
		if err != nil {
			b.Fatal(err)
		}
		u, _ := h.svc.User(sess.UserID)
		bal := h.svc.Balance(u.Account)
		view := viewUser(u, &bal)
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			var buf bytes.Buffer
			if err := json.NewEncoder(&buf).Encode(view); err != nil {
				b.Fatal(err)
			}
		}
	})

	// 5. The whole thing through the router.
	b.Run("4-full-read", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/me", nil)
			r.Header.Set("Authorization", "Bearer "+alice)
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})

	// 6. With an Origin header, so the CORS path and the health header run -
	//    which is what a browser actually sends.
	b.Run("5-full-read-cors", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/me", nil)
			r.Header.Set("Authorization", "Bearer "+alice)
			r.Header.Set("Origin", "https://nanacoin.example.net")
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})
}

// The event log records one entry per request. It is bounded in total, but
// each Add stores strings, so it has a per-request cost worth knowing.
func BenchmarkEventLogPerRequest(b *testing.B) {
	h := newHarness(b)
	_, alice, _, _, _ := h.setup()

	b.Run("with-logging", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/me", nil)
			r.Header.Set("Authorization", "Bearer "+alice)
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})

	// The log endpoint is exempted from logging, so this is the same
	// machinery without the Add - the difference is what recording costs.
	b.Run("without-logging", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/logs?limit=1", nil)
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})
}
