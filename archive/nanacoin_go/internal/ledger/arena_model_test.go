package ledger

import (
	"fmt"
	"math/rand"
	"strings"
	"testing"
)

func TestArenaChurnPreservesOtherOwners(t *testing.T) {
	a := NewArena()
	rng := rand.New(rand.NewSource(73))
	var slots [80]Slot
	var texts [80]string
	for step := 0; step < 5000; step++ {
		i := rng.Intn(len(slots))
		a.Release(slots[i])
		texts[i] = fmt.Sprintf("owner-%d-step-%d-value-%d", i, step, rng.Int63())
		slots[i] = a.Put(texts[i])
		if step%31 != 0 {
			continue
		}
		expected := 0
		for j := range slots {
			expected += len(texts[j])
			if got := a.Get(slots[j]); got != texts[j] {
				t.Fatalf("step %d owner %d corrupted: %q", step, j, got)
			}
		}
		used, _, truncated := a.Stats()
		if used != expected || truncated != 0 {
			t.Fatalf("accounting: used=%d expected=%d truncated=%d", used, expected, truncated)
		}
	}
	for i := range slots {
		a.Release(slots[i])
	}
	if used, _, _ := a.Stats(); used != 0 {
		t.Fatal("released arena leaks text", used)
	}
	if !a.CanStore(string(make([]byte, ArenaSize)), "") {
		t.Fatal("arena cannot reuse full capacity")
	}
}

// Put truncates to a block boundary when the arena is nearly full.
//
// That is the documented policy and it is right for prose: a clipped memo
// still reads. It is wrong for anything used as a key, and callers storing an
// identifier must check what came back rather than only whether it was empty.
//
// Seen on the board: an offer stored the listing ID "listing-dBibwBup", the
// 16-byte prefix of "listing-dBibwBupXqv9". The offer then referenced nothing
// - it rendered with an empty title and could never be accepted, because every
// lookup of that ID returned 404.
func TestPutTruncatesIdentifiersAtBlockBoundary(t *testing.T) {
	a := NewArena()

	// Leave less than one whole ID's worth of space.
	filler := make([]byte, ArenaSize-16)
	for i := range filler {
		filler[i] = 'x'
	}
	a.Put(string(filler))

	const id = "listing-dBibwBupXqv9"
	got := a.Get(a.Put(id))

	if got == id {
		t.Fatal("expected truncation with a nearly full arena; the policy has changed " +
			"and callers that check for it can be simplified")
	}
	if got != "listing-dBibwBup" {
		t.Errorf("truncated to %q, want the 16-byte block boundary %q", got, "listing-dBibwBup")
	}
	if !strings.HasPrefix(id, got) {
		t.Errorf("truncated value %q is not a prefix of %q", got, id)
	}
}
