package core

import (
	"fmt"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// What a write costs in allocations, measured at the service layer.
//
// The API-level benchmark next door measures a whole request, where net/http's
// own overhead - a few dozen allocations before any of our code runs - drowns
// the thing being changed. This one calls the service directly, so the number
// is ours.
//
// TakeQuote is the reason this exists. It writes two ledger records where
// every other endpoint writes one, and its first version built both as locals
// with slice literals for their postings: four heap objects per trade, on a
// board whose free heap is single-digit kilobytes and whose allocator has to
// find room in a fragmented arena. Both legs now live in the caller's
// WriteResult, the same pre-allocated scratch every other write endpoint uses.
//
//	go test ./internal/core/ -run XXX -bench BenchmarkWriteAllocs -benchmem
func benchHousehold(b *testing.B) (svc *Service, nana, alice, bob *users.User) {
	b.Helper()
	var seq int
	svc, err := New(memory.New(), Options{
		Now: func() time.Time { return time.Unix(1_700_000_000, 0) },
		NewID: func(prefix string) string {
			seq++
			return fmt.Sprintf("%s-%d", prefix, seq)
		},
	})
	if err != nil {
		b.Fatalf("New: %v", err)
	}
	if nana, err = svc.Provision("nana", "Nana", "nana-pin", "The House"); err != nil {
		b.Fatalf("Provision: %v", err)
	}
	if alice, err = svc.CreateUser(nana, "alice", "Alice", "a-pin", users.RoleUser, true); err != nil {
		b.Fatalf("CreateUser: %v", err)
	}
	if bob, err = svc.CreateUser(nana, "bob", "Bob", "b-pin", users.RoleUser, true); err != nil {
		b.Fatalf("CreateUser: %v", err)
	}
	return svc, nana, alice, bob
}

func BenchmarkWriteAllocs(b *testing.B) {
	// A trade: two ledger records, the heaviest write in the system.
	b.Run("take-quote", func(b *testing.B) {
		svc, nana, alice, bob := benchHousehold(b)
		if _, err := svc.Issue(nana, alice.Account, 1_000_000, "coins"); err != nil {
			b.Fatal(err)
		}
		if _, err := svc.IssueUSD(nana, bob.Account, 10_000_000, "dollars"); err != nil {
			b.Fatal(err)
		}

		// One quote per iteration, posted outside the timed region so the
		// measurement is the take alone.
		ids := make([]interface{ String() string }, 0)
		_ = ids
		var scratch WriteResult
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			b.StopTimer()
			q, err := svc.PostQuote(alice, marketplace.Ask, 1, 1, 0)
			if err != nil {
				b.Fatal(err)
			}
			b.StartTimer()

			if _, _, _, err := svc.TakeQuote(bob, q.ID, &scratch); err != nil {
				b.Fatal(err)
			}
		}
	})

	// The established single-record write, as a reference point: a trade
	// should not cost dramatically more than this per leg.
	b.Run("transfer", func(b *testing.B) {
		svc, nana, alice, bob := benchHousehold(b)
		if _, err := svc.Issue(nana, alice.Account, 10_000_000, "coins"); err != nil {
			b.Fatal(err)
		}
		var scratch WriteResult
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			if _, err := svc.Transfer(alice, bob.Account, 1, "chore", &scratch); err != nil {
				b.Fatal(err)
			}
		}
	})

	// The dollar balance lookup, which derives its account name. Run for
	// every user in every household listing, so a per-call allocation here
	// multiplies by the number of members.
	b.Run("usd-balance", func(b *testing.B) {
		svc, nana, alice, _ := benchHousehold(b)
		if _, err := svc.IssueUSD(nana, alice.Account, 1000, "dollars"); err != nil {
			b.Fatal(err)
		}
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			if svc.USDBalance(alice.Account) != 1000 {
				b.Fatal("wrong balance")
			}
		}
	})
}
