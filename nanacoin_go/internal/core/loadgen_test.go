package core

import (
	"path/filepath"
	"runtime"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/flashlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

// A year of household history must be generatable and must balance. The
// second half matters more: a seeded ledger that did not balance would make
// every measurement taken against it meaningless.
func TestSeedGeneratesABalancedYear(t *testing.T) {
	svc := newService(t, memory.New())

	res, err := svc.Seed(DefaultSeed())
	if err != nil {
		t.Fatalf("Seed: %v", err)
	}

	t.Logf("seeded %d members, %d transactions, %d listings (%d sold), %d reversals",
		res.Members, res.Transactions, res.Listings, res.Sold, res.Reversals)

	if res.Transactions < 400 {
		t.Errorf("a year at 10/week produced only %d transactions", res.Transactions)
	}
	if res.Sold == 0 {
		t.Error("no listings were bought, so the purchase path is untested")
	}
	if res.Reversals == 0 {
		t.Error("no reversals were generated, so corrections are untested")
	}
	if err := svc.book.CheckInvariants(); err != nil {
		t.Errorf("seeded ledger does not balance: %v", err)
	}
}

// What a year actually costs in RAM. This is the measurement that decides
// whether the board can hold a household's history, and it runs on a laptop -
// no flash cycle spent to answer it.
func TestSeededYearMemoryCost(t *testing.T) {
	// HeapAlloc is the wrong instrument here: the allocator can end the run
	// holding less than it started with, and the difference then says nothing
	// about what the model retains. TotalAlloc only rises, so a delta over it
	// is the churn; what is wanted is what stays live, which means keeping
	// the service reachable across the second reading and forcing a
	// collection first.
	measure := func() int64 {
		runtime.GC()
		runtime.GC()
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		return int64(m.HeapAlloc)
	}

	// A file-backed journal, as the board has: the in-memory journal keeps
	// every encoded event in RAM, which on the board is the flash file and
	// costs no heap at all. Measuring with the memory backend would charge
	// the RAM budget for something that does not live there, and that
	// mistake is worth ~280 bytes per transaction - most of the total.
	path := filepath.Join(t.TempDir(), "seed.journal")
	j, err := flashlog.Open(path, 0)
	if err != nil {
		t.Fatalf("opening journal: %v", err)
	}
	defer j.Close()

	before := measure()

	svc := newService(t, j)
	res, err := svc.Seed(DefaultSeed())
	if err != nil {
		t.Fatalf("Seed: %v", err)
	}

	after := measure()
	// Keep svc alive past the reading, or the collector is entitled to have
	// freed the very thing being measured.
	runtime.KeepAlive(svc)

	used := after - before
	if used <= 0 {
		t.Fatalf("measured %d bytes for %d transactions, which cannot be right - "+
			"the service was probably collected before the second reading",
			used, res.Transactions)
	}
	perTxn := float64(used) / float64(res.Transactions)

	t.Logf("a year of history: %d transactions in %d bytes (%.1f kB)",
		res.Transactions, used, float64(used)/1024)
	t.Logf("  %.0f bytes per transaction", perTxn)

	const boardSpare = 17408
	t.Logf("  board has ~%d bytes spare -> about %d transactions, %.1f weeks at 10/week",
		boardSpare, int(float64(boardSpare)/perTxn), float64(boardSpare)/perTxn/10)
}
