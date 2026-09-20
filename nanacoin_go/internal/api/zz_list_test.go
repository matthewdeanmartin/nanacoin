package api

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func BenchmarkListEndpoints(b *testing.B) {
	h := newHarness(b)
	nana, alice, _, aliceAcct, bobAcct := h.setup()
	for i := 0; i < 30; i++ {
		h.do("POST", "/api/v1/transfers", alice, transferRequest{To: acct(bobAcct), Amount: 1, Memo: "chore"})
	}
	cases := []struct{ name, path, tok string }{
		{"ledger-30", "/api/v1/transactions?limit=30", nana},
		{"history-30", "/api/v1/accounts/" + aliceAcct + "/transactions?limit=30", alice},
		{"users", "/api/v1/users", nana},
		{"me", "/api/v1/me", alice},
	}
	for _, tc := range cases {
		b.Run(tc.name, func(b *testing.B) {
			b.ReportAllocs()
			for i := 0; i < b.N; i++ {
				r := httptest.NewRequest("GET", tc.path, nil)
				r.Header.Set("Authorization", "Bearer "+tc.tok)
				w := &discardWriter{header: make(http.Header, 6)}
				h.srv.ServeHTTP(w, r)
			}
		})
	}
}
