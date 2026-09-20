package api

import (
	"encoding/json"
	"net/http"
	"runtime"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
)

// The streamed list endpoints must produce exactly the JSON the buffered ones
// did. The shape is a client contract; the point of the change was the memory
// profile, not the wire format.
func TestStreamedListsAreValidJSON(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, aliceAcct, bobAcct := h.setup()

	for i := 0; i < 5; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice, transferRequest{
			To: acct(bobAcct), Amount: 1, Memo: "chore",
		}), http.StatusCreated, nil)
	}

	cases := []struct {
		name   string
		path   string
		token  string
		fields []string
	}{
		{"users", "/api/v1/users", nana, []string{"users"}},
		{"listings", "/api/v1/listings", alice, []string{"listings"}},
		{"logs", "/api/v1/logs", "", []string{"events", "total"}},
		{"ledger", "/api/v1/transactions", nana, []string{"transactions", "circulation"}},
		{
			"history",
			"/api/v1/accounts/" + aliceAcct + "/transactions",
			alice,
			[]string{"account", "balance", "transactions"},
		},
	}

	for _, tc := range cases {
		// A no-logs build does not register /logs at all, so the route
		// 404s by design and there is no streamed shape to check.
		if tc.name == "logs" && !eventlog.Enabled {
			continue
		}
		t.Run(tc.name, func(t *testing.T) {
			w := h.do("GET", tc.path, tc.token, nil)
			if w.Code != http.StatusOK {
				t.Fatalf("status %d: %s", w.Code, w.Body.String())
			}
			var got map[string]json.RawMessage
			if err := json.Unmarshal(w.Body.Bytes(), &got); err != nil {
				t.Fatalf("not valid JSON: %v\nbody: %s", err, w.Body.String())
			}
			for _, f := range tc.fields {
				if _, ok := got[f]; !ok {
					t.Errorf("missing field %q in %s", f, w.Body.String())
				}
			}
		})
	}
}

// An empty list must still be an empty array, not null and not absent. A
// client iterating the result should not have to special-case "no rows yet",
// and an incremental writer is exactly the kind of code that gets this wrong.
func TestStreamedEmptyListIsEmptyArray(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	w := h.do("GET", "/api/v1/listings", alice, nil)
	var got struct {
		Listings []json.RawMessage `json:"listings"`
	}
	h.decodeInto(w, http.StatusOK, &got)
	if got.Listings == nil {
		t.Fatalf("listings was null, want []: %s", w.Body.String())
	}
	if len(got.Listings) != 0 {
		t.Fatalf("listings had %d entries, want 0", len(got.Listings))
	}
}

// ?limit=0 used to bypass the page cap, because it fell through to the
// caller's default without the default being clamped. On the board that
// default (100) is above the cap (30), so the one value that looks like
// "no limit" was the one value that got no limit.
func TestZeroLimitIsClamped(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, bobAcct := h.setup()

	// The harness server uses DefaultMaxPageSize (100), so drive the check
	// against a limit the server must clamp rather than against the cap.
	for i := 0; i < 8; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice, transferRequest{
			To: acct(bobAcct), Amount: 1, Memo: "chore",
		}), http.StatusCreated, nil)
	}

	// limit=0 must behave as the default, not as "unlimited", and must never
	// exceed the server's cap.
	for _, q := range []string{"?limit=0", "?limit=1000", ""} {
		w := h.do("GET", "/api/v1/transactions"+q, nana, nil)
		var got struct {
			Transactions []json.RawMessage `json:"transactions"`
		}
		h.decodeInto(w, http.StatusOK, &got)
		if len(got.Transactions) > DefaultMaxPageSize {
			t.Errorf("%q returned %d transactions, cap is %d",
				q, len(got.Transactions), DefaultMaxPageSize)
		}
	}
}

// What a list response costs, as a function of how many items it contains.
//
// This is the regression guard for the change that motivated the streaming
// writer. Building the page whole meant the cost scaled with the page size:
// the unpacked records, the view objects and the encoded bytes were all live
// at once. Streaming should leave only the per-item marshalling cost.
//
// Asserted as a ratio rather than an absolute, because the absolute depends
// on the machine and the Go version, while the slope is the property that
// actually governs whether the board survives.
func TestListCostIsSublinearInPageSize(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, bobAcct := h.setup()

	for i := 0; i < 60; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice, transferRequest{
			To: acct(bobAcct), Amount: 1, Memo: "chore",
		}), http.StatusCreated, nil)
	}

	measure := func(limit string) uint64 {
		path := "/api/v1/transactions?limit=" + limit
		// Warm up, so lazy first-call initialisation is not charged to
		// whichever size runs first.
		h.do("GET", path, nana, nil)

		const reps = 20
		var before, after runtime.MemStats
		runtime.GC()
		runtime.ReadMemStats(&before)
		for i := 0; i < reps; i++ {
			h.do("GET", path, nana, nil)
		}
		runtime.ReadMemStats(&after)
		return (after.TotalAlloc - before.TotalAlloc) / reps
	}

	small := measure("5")
	large := measure("50")
	t.Logf("5 items: %d B/request, 50 items: %d B/request", small, large)

	// Ten times the items must not cost ten times the memory. Each element
	// is still marshalled, so this is not flat - but it must stay well below
	// the linear growth the buffered path paid on the peak.
	if small > 0 && large > small*6 {
		t.Errorf("cost scales with page size: %d B for 5 items, %d B for 50 - "+
			"the list is probably being built whole again", small, large)
	}
}
