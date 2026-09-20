package ledger

import (
	"errors"
	"strings"
	"testing"
)

func TestRingWrapPreservesBalancesAndOrdering(t *testing.T) {
	b := NewBook()
	const total = Capacity*4 + 17
	for n := 1; n <= total; n++ {
		id := mustIssue(t, b, alice, 1)
		if id != TransactionIDFor(uint32(n)) {
			t.Fatalf("id %s at %d", id, n)
		}
	}
	if b.Len() != Capacity || b.Total() != total || b.Oldest() != total-Capacity+1 {
		t.Fatal("incorrect retained window")
	}
	if b.Balance(alice) != total || b.Circulation() != total {
		t.Fatal("eviction changed balances")
	}
	if _, ok := b.Get("txn-1"); ok {
		t.Fatal("evicted ID aliases a new slot")
	}
	if _, err := b.BuildReversal("txn-1", "nana", "old", 0); !errors.Is(err, ErrNotFound) {
		t.Fatalf("old reversal: %v", err)
	}
	for i, tx := range b.All() {
		if tx.ID != TransactionIDFor(b.Oldest()+uint32(i)) {
			t.Fatal("chronological order")
		}
	}
	seen := 0
	b.EachPackedHistory(alice, 0, func(p *PackedTransaction) bool {
		if p.Seq != total-uint32(seen) {
			t.Fatal("history order")
		}
		seen++
		return true
	})
	if seen != Capacity {
		t.Fatal("history count")
	}
	if err := b.CheckInvariants(); err != nil {
		t.Fatal(err)
	}
	oldest := b.Oldest()
	if _, err := b.Append(transfer(alice, bob, total+1), false); !errors.Is(err, ErrInsufficient) {
		t.Fatal(err)
	}
	if b.Oldest() != oldest || b.Total() != total {
		t.Fatal("rejection evicted history")
	}
	b.balanceSlot(b.strs.Find(string(alice))).balance++
	if b.CheckInvariants() == nil {
		t.Fatal("missed corrupt balance after wrapping")
	}
}

func TestRingReversalAtOverwriteBoundary(t *testing.T) {
	b := NewBook()
	for i := 0; i < Capacity; i++ {
		mustIssue(t, b, alice, 1)
	}
	r, err := b.BuildReversal("txn-1", "nana", "undo", 0)
	if err != nil {
		t.Fatal(err)
	}
	id, err := b.Append(r, true)
	if err != nil {
		t.Fatal(err)
	}
	tx, ok := b.Get(id)
	if !ok || tx.Reverses != "txn-1" {
		t.Fatal("lost original reversal ID")
	}
	if _, ok := b.ReversalOf(id); ok {
		t.Fatal("reused slot inherited reversal link")
	}
	if b.Balance(alice) != Capacity-1 {
		t.Fatal("incorrect reversal balance")
	}
	if err := b.CheckInvariants(); err != nil {
		t.Fatal(err)
	}
}

func TestRingReclaimsTextAndReplaysLongJournal(t *testing.T) {
	b, replayed := NewBook(), NewBook()
	memo := strings.Repeat("m", 140)
	for i := 1; i <= Capacity*3; i++ {
		tx := issuance(alice, 1)
		tx.Description = memo
		id, err := b.Append(tx, false)
		if err != nil {
			t.Fatal(err)
		}
		tx.ID = id
		if _, err := replayed.Replay(tx); err != nil {
			t.Fatal(err)
		}
		got, _ := b.Get(id)
		if got.Description != memo {
			t.Fatal("memo was truncated")
		}
	}
	if b.Len() >= Capacity {
		t.Fatal("text pressure did not bound retained history")
	}
	if b.Balance(alice) != Capacity*3 || replayed.Balance(alice) != b.Balance(alice) || replayed.Oldest() != b.Oldest() {
		t.Fatal("replay mismatch")
	}
	_, _, truncated := b.arena.Stats()
	if truncated != 0 {
		t.Fatalf("text not reclaimed: %d truncations", truncated)
	}
	for _, tx := range b.All() {
		if tx.Description != memo {
			t.Fatal("live text overwritten")
		}
	}
	if err := b.CheckInvariants(); err != nil {
		t.Fatal(err)
	}
}

func TestArenaReleasePreservesOtherOwners(t *testing.T) {
	a := NewArena()
	keep := a.Put("domain text must survive")
	for i := 0; i < 2000; i++ {
		s := a.Put(strings.Repeat("x", 140))
		a.Release(s)
		if a.Get(keep) != "domain text must survive" {
			t.Fatal("corrupted live text")
		}
	}
	used, _, truncated := a.Stats()
	if used != len("domain text must survive") || truncated != 0 {
		t.Fatal("arena leaked")
	}
}

func TestReversalLinksDoNotFillInternTable(t *testing.T) {
	b := NewBook()
	for i := 0; i < Capacity*2; i++ {
		id := mustIssue(t, b, alice, 1)
		r, err := b.BuildReversal(id, "nana", "undo", 0)
		if err != nil {
			t.Fatal(err)
		}
		rid, err := b.Append(r, true)
		if err != nil {
			t.Fatal(err)
		}
		if got, ok := b.ReversalOf(id); !ok || got != rid {
			t.Fatal("missing reversal link")
		}
	}
	if b.strs.Len() > 4 {
		t.Fatalf("unique reversal IDs were interned: %d", b.strs.Len())
	}
	if b.Balance(alice) != 0 {
		t.Fatal("reversal balance drift")
	}
	if err := b.CheckInvariants(); err != nil {
		t.Fatal(err)
	}
}
