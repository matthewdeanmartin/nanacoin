package ledger

import (
	"errors"
	"fmt"
	"math/rand"
	"testing"
)

// Compare the ring with an independent balance model over many wraps. Include
// rejected overdrafts: rejection must change neither balances nor sequence.
func TestRandomTransfersAgainstModel(t *testing.T) {
	for _, seed := range []int64{1, 7, 42} {
		b := NewBook()
		mustIssue(t, b, alice, 1000)
		mustIssue(t, b, bob, 1000)
		balances := map[AccountID]Amount{alice: 1000, bob: 1000}
		rng := rand.New(rand.NewSource(seed))
		for n := 0; n < Capacity*6; n++ {
			from, to := alice, bob
			if rng.Intn(2) == 0 {
				from, to = to, from
			}
			amount := Amount(rng.Intn(1500) + 1)
			total := b.Total()
			oldest := b.Oldest()
			_, err := b.Append(transfer(from, to, amount), false)
			if amount > balances[from] {
				if err == nil || b.Total() != total || b.Oldest() != oldest {
					t.Fatalf("seed %d step %d: rejected mutation", seed, n)
				}
			} else {
				if err != nil {
					t.Fatal(err)
				}
				balances[from] -= amount
				balances[to] += amount
			}
			if b.Balance(alice) != balances[alice] || b.Balance(bob) != balances[bob] || b.Circulation() != 2000 {
				t.Fatalf("seed %d step %d: model mismatch", seed, n)
			}
			if n%127 == 0 {
				if err := b.CheckInvariants(); err != nil {
					t.Fatal(err)
				}
			}
		}
	}
}

func TestInvariantScratchWindowsCoverAllReferences(t *testing.T) {
	b := NewBook()
	for i := 0; i < MaxInterned-1; i++ {
		b.strs.Intern(fmt.Sprintf("filler-%d", i))
	}
	// Exercise boundaries and the last reference after many history wraps.
	for i := 0; i < Capacity*3; i++ {
		from := Ref(1)
		to := Ref(31 + i%(MaxInterned-31))
		tx := &Transaction{Kind: KindTransfer, Postings: []Posting{{Account: AccountID(b.strs.Lookup(from)), Amount: -1}, {Account: AccountID(b.strs.Lookup(to)), Amount: 1}}}
		if _, e := b.Append(tx, true); e != nil {
			t.Fatal(e)
		}
	}
	if e := b.CheckInvariants(); e != nil {
		t.Fatal(e)
	}
	for _, ref := range []int{0, 31, 32, 63, 64, 255, 256, 511} {
		b.balanceSlot(Ref(ref)).balance++
		if b.CheckInvariants() == nil {
			t.Fatalf("missed corruption at ref %d", ref)
		}
		b.balanceSlot(Ref(ref)).balance--
	}
	p := &b.txns[b.slot(0)]
	old := p.Accounts[0]
	p.Accounts[0] = MaxInterned
	if b.CheckInvariants() == nil {
		t.Fatal("invalid reference accepted")
	}
	p.Accounts[0] = old
}

func TestCompactAccountBookMatchesDenseThroughWraps(t *testing.T) {
	dense := NewBook()
	strs := NewStrings()
	for i := 0; i < 450; i++ {
		strs.Intern(fmt.Sprintf("unrelated-name-%d", i))
	}
	compact := NewBookWithAccountCapacity(strs, NewArena(), 3)
	for _, b := range []*Book{dense, compact} {
		mustIssue(t, b, alice, 10000)
		mustIssue(t, b, bob, 10000)
	}
	for i := 0; i < Capacity*3; i++ {
		from, to := alice, bob
		if i%2 == 0 {
			from, to = to, from
		}
		tx := transfer(from, to, Amount(i%17+1))
		for _, b := range []*Book{dense, compact} {
			if _, e := b.Append(tx, false); e != nil {
				t.Fatal(e)
			}
		}
		if compact.Balance(alice) != dense.Balance(alice) || compact.Balance(bob) != dense.Balance(bob) || compact.Circulation() != dense.Circulation() {
			t.Fatal("compact account mapping changed balances")
		}
	}
	if e := compact.CheckInvariants(); e != nil {
		t.Fatal(e)
	}
	if compact.Total() != dense.Total() || compact.Oldest() != dense.Oldest() {
		t.Fatal("history changed")
	}
}
func TestAccountCapacityFailureDoesNotEvictOrChangeMoney(t *testing.T) {
	b := NewBookWithAccountCapacity(NewStrings(), NewArena(), 2)
	for i := 0; i < Capacity; i++ {
		mustIssue(t, b, alice, 1)
	}
	total, oldest, balance := b.Total(), b.Oldest(), b.Balance(alice)
	tx := transfer(alice, bob, 1)
	for _, apply := range []func(*Transaction) (TransactionID, error){func(tx *Transaction) (TransactionID, error) { return b.Append(tx, false) }, b.Replay} {
		if _, e := apply(tx); !errors.Is(e, ErrAccountCapacity) {
			t.Fatal("capacity not checked before apply", e)
		}
		if b.Total() != total || b.Oldest() != oldest || b.Balance(alice) != balance || b.Balance(bob) != 0 {
			t.Fatal("rejected account consumed history or money")
		}
	}
	if e := b.CheckInvariants(); e != nil {
		t.Fatal(e)
	}
}
