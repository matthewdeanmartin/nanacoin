package core

import (
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Allocation budgets for the write paths, as a test rather than a benchmark.
//
// # Why this exists
//
// The board's failure mode is "fatal error: out of memory" - an abort that
// recover() cannot catch, on a heap of a few kilobytes that the allocator has
// to find room in after fragmentation. Every allocation on a request path is
// a step toward that, and the regressions that caused it were never obvious
// in review: TakeQuote built two transactions as locals with slice literals
// for their postings, which reads perfectly normally and put four objects on
// the heap per trade.
//
// The benchmark next door reports the numbers. This turns them into a rule,
// so a regression fails CI instead of waiting to be found on hardware during
// a load test.
//
// # About the budgets
//
// They are the measured count plus a small margin - deliberately tight. A
// generous budget is worse than no test: the first version of this file
// allowed 60 for take-quote, and re-introducing the exact four-allocation
// regression it was written to catch still passed. The margin is there to
// absorb a runtime change, not to leave room for new allocations.
//
// If a change pushes a path over its budget, the question to ask is what new
// allocation appeared - profile with:
//
//	go test ./internal/core/ -run XXX -bench BenchmarkWriteAllocs \
//	    -memprofile=mem.prof -memprofilerate=1
//	go tool pprof -sample_index=alloc_objects -top mem.prof
//
// Raise a budget only when the new cost is understood and justified.
func TestWritePathsStayWithinAllocationBudget(t *testing.T) {
	cases := []struct {
		name   string
		budget float64
		run    func(t *testing.T) func()
	}{
		{
			// The heaviest write in the system: two ledger records, where
			// every other endpoint writes one. Both legs live in the caller's
			// WriteResult; what remains is the journal event, the encoding,
			// and the unpack machinery shared with every other endpoint.
			// Measured at 49, which includes the PostQuote that sets each
			// iteration up; the take itself is the larger half.
			name:   "take-quote",
			budget: 53,
			run: func(t *testing.T) func() {
				f := newFX(t)
				var scratch WriteResult
				return func() {
					q, err := f.svc.PostQuote(f.alice, marketplace.Ask, 1, 1, 0)
					if err != nil {
						t.Fatalf("PostQuote: %v", err)
					}
					if _, _, _, err := f.svc.TakeQuote(f.bob, q.ID, &scratch); err != nil {
						t.Fatalf("TakeQuote: %v", err)
					}
				}
			},
		},
		{
			// The established single-record write, as the reference point a
			// trade is judged against.
			// Measured at 9.
			name:   "transfer",
			budget: 12,
			run: func(t *testing.T) func() {
				f := newFX(t)
				var scratch WriteResult
				return func() {
					if _, err := f.svc.Transfer(f.alice, f.bob.Account, 1, "chore", &scratch); err != nil {
						t.Fatalf("Transfer: %v", err)
					}
				}
			},
		},
		{
			// Zero, and it must stay zero. The dollar wallet's name is
			// derived rather than stored, so the obvious implementation
			// concatenates a string - once per user, on every household
			// listing. It is built in a stack buffer instead.
			name:   "usd-balance",
			budget: 0,
			run: func(t *testing.T) func() {
				f := newFX(t)
				return func() {
					if got := f.svc.USDBalance(f.alice.Account); got == -1 {
						t.Fatal("unexpected balance")
					}
				}
			},
		},
		{
			// Reading the book is what the exchange page polls, so it runs
			// far more often than any write.
			//
			// The ordering is free - EachQuote sorts indices in a stack array
			// - but unpacking is not: each quote costs the returned struct
			// plus its ID string from the arena, so the budget is per quote
			// and not per call. Eight quotes, two allocations each, plus
			// slack. The other names come back interned and cost nothing.
			//
			// This is the same shape as the offers and listings endpoints,
			// which the board already serves; it is recorded here rather than
			// optimised because the fix belongs to all three at once.
			// Measured at 16: eight quotes, two allocations each.
			name:   "read-book",
			budget: 18,
			run: func(t *testing.T) func() {
				f := newFX(t)
				for i := 0; i < 8; i++ {
					if _, err := f.svc.PostQuote(f.alice, marketplace.Ask, int64(20+i), 1, 0); err != nil {
						t.Fatalf("PostQuote: %v", err)
					}
				}
				return func() {
					n := 0
					f.svc.EachQuote(func(*marketplace.Quote) bool { n++; return true })
					if n == 0 {
						t.Fatal("no quotes walked")
					}
				}
			},
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			step := tc.run(t)
			// A warm-up pass first: the first call through a path touches
			// lazily-built state that a steady-state request does not.
			step()
			if got := testing.AllocsPerRun(50, step); got > tc.budget {
				t.Errorf("%s allocates %.0f objects per call, budget %.0f\n"+
					"Profile with: go test ./internal/core/ -run XXX -bench BenchmarkWriteAllocs -memprofile=mem.prof -memprofilerate=1",
					tc.name, got, tc.budget)
			}
		})
	}
}
