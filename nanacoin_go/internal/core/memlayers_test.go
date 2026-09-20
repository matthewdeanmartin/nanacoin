package core

import (
	"path/filepath"
	"runtime"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/flashlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Where the per-transaction memory actually goes, layer by layer.
//
// This exists because the wrong layer was blamed repeatedly. The packed record
// is 48 bytes; a seeded year measured 466 bytes per transaction, and the gap
// was assumed to be the ledger when almost all of it was the in-memory
// journal - which on the board is the flash file and costs no RAM at all.
//
// Measuring with the memory backend charges the RAM budget for something that
// does not live there, and that mistake is worth ~280 bytes per transaction.
// The "service, flash journal" figure is the one that governs how long the
// board lasts.
func TestWhereTheMemoryGoes(t *testing.T) {
	measure := func() int64 {
		runtime.GC()
		runtime.GC()
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		return int64(m.HeapAlloc)
	}

	// Large enough that the service's fixed cost is noise against the
	// per-transaction cost being measured.
	const n = 2000

	// 1. The ledger alone: packed records, no service, no journal.
	before := measure()
	book := ledger.NewBook()
	for i := 0; i < n; i++ {
		book.Append(&ledger.Transaction{
			Kind: ledger.KindIssue, CreatedAt: int64(i),
			Actor: "user-alice", Description: "chores",
			Postings: []ledger.Posting{
				{Account: ledger.SystemIssuance, Amount: -1},
				{Account: "account-alice", Amount: 1},
			},
		}, false)
	}
	afterBook := measure()
	runtime.KeepAlive(book)
	t.Logf("ledger only:        %5.0f bytes/txn", float64(afterBook-before)/n)

	// 2. Plus the service: the journal event, the idempotency cache, the maps.
	before2 := measure()
	svc := newService(t, memory.New())
	// Errors checked, not discarded: a short password is refused, and
	// ignoring that produced a nil-pointer panic rather than a clear failure.
	nana, err := svc.Provision("nana", "Nana", "nana-pin", "H")
	if err != nil {
		t.Fatalf("Provision: %v", err)
	}
	alice, err := svc.CreateUser(nana, "alice", "Alice", "alice-pin", users.RoleUser, false)
	if err != nil {
		t.Fatalf("CreateUser alice: %v", err)
	}
	bob, err := svc.CreateUser(nana, "bob", "Bob", "bob-pin", users.RoleUser, false)
	if err != nil {
		t.Fatalf("CreateUser bob: %v", err)
	}
	if _, err := svc.Issue(nana, alice.Account, 1_000_000, "float"); err != nil {
		t.Fatalf("Issue: %v", err)
	}
	for i := 0; i < n; i++ {
		if _, err := svc.Transfer(alice, bob.Account, 1, "chores"); err != nil {
			t.Fatalf("transfer %d: %v", i, err)
		}
	}
	afterSvc := measure()
	runtime.KeepAlive(svc)
	t.Logf("full service:       %5.0f bytes/txn", float64(afterSvc-before2)/n)

	// 3. The service with a FILE journal, which is what the board has: the
	// encoded events go to flash rather than staying in RAM. This is the
	// number that governs how long the board lasts.
	path := filepath.Join(t.TempDir(), "iso.journal")
	fj, err := flashlog.Open(path, 0)
	if err != nil {
		t.Fatalf("opening journal: %v", err)
	}
	defer fj.Close()

	before4 := measure()
	svc2 := newService(t, fj)
	nana2, err := svc2.Provision("nana", "Nana", "nana-pin", "H")
	if err != nil {
		t.Fatalf("Provision: %v", err)
	}
	a2, _ := svc2.CreateUser(nana2, "alice", "Alice", "alice-pin", users.RoleUser, false)
	b2, _ := svc2.CreateUser(nana2, "bob", "Bob", "bob-pin", users.RoleUser, false)
	svc2.Issue(nana2, a2.Account, 1_000_000, "float")
	for i := 0; i < n; i++ {
		if _, err := svc2.Transfer(a2, b2.Account, 1, "chores"); err != nil {
			t.Fatalf("transfer %d: %v", i, err)
		}
	}
	afterFile := measure()
	runtime.KeepAlive(svc2)
	t.Logf("service, flash journal: %5.0f bytes/txn  <- the board's real cost",
		float64(afterFile-before4)/n)

	// 4. The memory journal in isolation, to confirm it is what the
	// difference above consists of.
	j := memory.New()
	before3 := measure()
	for i := 0; i < n; i++ {
		j.Append(1, []byte(`{"txn":{"id":"txn-1","kind":"TRANSFER","created_at":1789000000,`+
			`"actor":"user-abcdefghijkl","description":"chores","postings":[`+
			`{"account":"account-abcdefghijkl","amount":-1},`+
			`{"account":"account-mnopqrstuvwx","amount":1}]}}`))
	}
	afterJournal := measure()
	runtime.KeepAlive(j)
	t.Logf("in-memory journal:  %5.0f bytes/txn  <- flash on the board, not RAM",
		float64(afterJournal-before3)/n)
}
