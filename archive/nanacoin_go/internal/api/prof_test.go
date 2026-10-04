package api

import (
	"bytes"
	"encoding/json"
	"net/http/httptest"
	"testing"
)

// Where a transfer's allocation goes, layer by layer.
//
// Measured on the board: a transfer costs about 8 kB, which against ~12 kB of
// free heap is room for one more. Packing the stored record from 552 bytes to
// 55 did nothing for this, because the cost is in serving the request rather
// than in keeping the result - so the layers are measured separately instead
// of guessed at.
//
// Run with:
//
//	go test ./internal/api/ -run XXX -bench BenchmarkTransferLayers -benchmem
func BenchmarkTransferLayers(b *testing.B) {
	h := newHarness(b)
	_, alice, _, _, bobAcct := h.setup()
	body, _ := json.Marshal(transferRequest{To: acct(bobAcct), Amount: 1, Memo: "chores"})

	// Decoding the request body, which is what the handler does first.
	b.Run("json-decode", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			var req transferRequest
			dec := json.NewDecoder(bytes.NewReader(body))
			dec.DisallowUnknownFields()
			if err := dec.Decode(&req); err != nil {
				b.Fatal(err)
			}
		}
	})

	// Decoding with Unmarshal instead, for comparison: a Decoder allocates a
	// buffered reader that Unmarshal does not need when the whole body is
	// already in memory - which on the board it always is.
	b.Run("json-unmarshal", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			var req transferRequest
			if err := json.Unmarshal(body, &req); err != nil {
				b.Fatal(err)
			}
		}
	})

	// Encoding a response view.
	b.Run("json-encode-view", func(b *testing.B) {
		view := transactionView{
			ID: "txn-42", Kind: "TRANSFER", CreatedAt: 1789000000,
			Actor: "user-abcdefghijkl", Description: "chores",
			Postings: []postingView{
				{Account: "account-abcdefghijkl", Name: "Alice", Amount: -1},
				{Account: "account-mnopqrstuvwx", Name: "Bob", Amount: 1},
			},
		}
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			if _, err := json.Marshal(view); err != nil {
				b.Fatal(err)
			}
		}
	})

	// The whole request through the router, which is the figure the board
	// actually pays.
	b.Run("full-request", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("POST", "/api/v1/transfers", bytes.NewReader(body))
			r.Header.Set("Authorization", "Bearer "+alice)
			r.Header.Set("Content-Type", "application/json")
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})

	// A read, for contrast: no body to decode and no journal write, so the
	// difference isolates what a write costs beyond a read.
	b.Run("full-read", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/me", nil)
			r.Header.Set("Authorization", "Bearer "+alice)
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})
}
