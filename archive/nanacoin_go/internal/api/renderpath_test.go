package api

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// The render path must produce exactly what the view path did.
func TestRenderPathMatchesViewPath(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, aliceAcct, bobAcct := h.setup()
	h.decodeInto(h.do("POST", "/api/v1/transfers", alice, transferRequest{
		To: acct(bobAcct), Amount: 3, Memo: "emoji 🎉 and \"quotes\" and 日本",
	}), http.StatusCreated, nil)

	for _, p := range []string{
		"/api/v1/transactions?limit=10",
		"/api/v1/accounts/" + aliceAcct + "/transactions?limit=10",
	} {
		r := httptest.NewRequest("GET", p, nil)
		r.Header.Set("Authorization", "Bearer "+nana)
		w := httptest.NewRecorder()
		h.srv.ServeHTTP(w, r)
		var got map[string]any
		if err := json.Unmarshal(w.Body.Bytes(), &got); err != nil {
			t.Fatalf("%s: invalid JSON: %v\n%s", p, err, w.Body.String())
		}
		txns, _ := got["transactions"].([]any)
		if len(txns) == 0 {
			t.Fatalf("%s: no transactions", p)
		}
		first, _ := txns[0].(map[string]any)
		for _, f := range []string{"id", "kind", "created_at", "actor", "description", "postings"} {
			if _, ok := first[f]; !ok {
				t.Errorf("%s: missing %q in %v", p, f, first)
			}
		}
		if d, _ := first["description"].(string); d != "emoji 🎉 and \"quotes\" and 日本" {
			t.Errorf("%s: description round-tripped as %q", p, d)
		}
	}
}
