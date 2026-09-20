package ledger

import (
	"testing"
	"unsafe"
)

// The whole point of packing is the size, so it is asserted rather than
// assumed. Field order in PackedTransaction matters: reordering it introduces
// padding and silently grows the struct.
func TestPackedTransactionIsSmall(t *testing.T) {
	var p PackedTransaction
	var old Transaction

	size := int(unsafe.Sizeof(p))
	t.Logf("PackedTransaction %d bytes (was %d for Transaction, before its "+
		"five string bodies and postings slice)", size, unsafe.Sizeof(old))

	// A ceiling, not an exact figure: a new field is allowed, a careless one
	// that doubles the struct is not.
	const ceiling = 64
	if size > ceiling {
		t.Errorf("PackedTransaction is %d bytes, over the %d ceiling - check "+
			"for padding from field reordering", size, ceiling)
	}
}

func TestPackUnpackRoundTrip(t *testing.T) {
	strs := NewStrings()
	arena := NewArena()

	original := &Transaction{
		ID:          "ignored-derived-from-seq",
		Kind:        KindTransfer,
		CreatedAt:   1789000000,
		Actor:       "user-alice",
		Description: "Taking out the trash",
		Postings: []Posting{
			{Account: "account-alice", Amount: -5},
			{Account: "account-bob", Amount: 5},
		},
	}

	packed, ok := Pack(original, strs, arena, 42)
	if !ok {
		t.Fatal("Pack refused an ordinary transfer")
	}

	got := Unpack(&packed, strs, arena)

	if got.ID != "txn-42" {
		t.Errorf("ID is %q, want txn-42 - it is derived from the sequence", got.ID)
	}
	if got.Kind != original.Kind {
		t.Errorf("Kind is %q, want %q", got.Kind, original.Kind)
	}
	if got.CreatedAt != original.CreatedAt {
		t.Errorf("CreatedAt is %d, want %d", got.CreatedAt, original.CreatedAt)
	}
	if got.Actor != original.Actor {
		t.Errorf("Actor is %q, want %q", got.Actor, original.Actor)
	}
	if got.Description != original.Description {
		t.Errorf("Description is %q, want %q", got.Description, original.Description)
	}
	if len(got.Postings) != 2 {
		t.Fatalf("got %d postings, want 2", len(got.Postings))
	}
	for i := range original.Postings {
		if got.Postings[i] != original.Postings[i] {
			t.Errorf("posting %d is %+v, want %+v", i, got.Postings[i], original.Postings[i])
		}
	}
}

// An empty Reference or Reverses must cost nothing, which is most of the
// saving on an ordinary transfer.
func TestEmptyFieldsInternToZero(t *testing.T) {
	strs := NewStrings()
	arena := NewArena()

	packed, ok := Pack(&Transaction{
		Kind:      KindTransfer,
		CreatedAt: 1,
		Postings:  []Posting{{Account: "a", Amount: -1}, {Account: "b", Amount: 1}},
	}, strs, arena, 1)
	if !ok {
		t.Fatal("Pack refused")
	}

	// Reverses still interns (almost always empty, so a zero Ref is free);
	// Reference and Description go to the arena, where an empty string is
	// the zero Slot and costs no arena bytes.
	if packed.Reverses != 0 {
		t.Errorf("empty Reverses interned to %d, want 0", packed.Reverses)
	}
	if !packed.Reference.IsEmpty() || !packed.Description.IsEmpty() {
		t.Errorf("empty text took arena slots %v/%v, want empty",
			packed.Reference, packed.Description)
	}
	got := Unpack(&packed, strs, arena)
	if got.Reference != "" || got.Reverses != "" || got.Description != "" {
		t.Errorf("a zero Ref unpacked to %q/%q/%q", got.Reference, got.Reverses, got.Description)
	}
}

// Repeated values must be stored once - that is what interning buys.
func TestInterningDeduplicates(t *testing.T) {
	strs := NewStrings()
	arena := NewArena()

	for i := 0; i < 100; i++ {
		if _, ok := Pack(&Transaction{
			Kind: KindTransfer, CreatedAt: int64(i), Actor: "user-alice",
			Description: "chores",
			Postings: []Posting{
				{Account: "account-alice", Amount: -1},
				{Account: "account-bob", Amount: 1},
			},
		}, strs, arena, uint32(i)); !ok {
			t.Fatal("Pack refused")
		}
	}

	// The empty string, plus alice, bob and the actor. The description is
	// no longer interned - free text has unbounded cardinality, so it lives
	// in the arena instead. See PackedTransaction.Description.
	if got := strs.Len(); got != 4 {
		t.Errorf("100 identical-shaped records interned %d strings, want 5", got)
	}
}

// More postings than fit must be refused, not silently truncated: a record
// missing a posting would not sum to zero, and the whole model rests on that.
func TestPackRefusesTooManyPostings(t *testing.T) {
	strs := NewStrings()
	arena := NewArena()

	if _, ok := Pack(&Transaction{
		Kind: KindTransfer, CreatedAt: 1,
		Postings: []Posting{
			{Account: "a", Amount: -10},
			{Account: "b", Amount: 5},
			{Account: "c", Amount: 5},
		},
	}, strs, arena, 1); ok {
		t.Error("Pack accepted three postings, which would lose one silently")
	}
}

func TestTransactionIDRoundTrip(t *testing.T) {
	for _, seq := range []uint32{0, 1, 42, 65535, 4294967295} {
		id := TransactionIDFor(seq)
		got, ok := SeqForTransactionID(id)
		if !ok {
			t.Errorf("%q did not parse back", id)
			continue
		}
		if got != seq {
			t.Errorf("%q parsed to %d, want %d", id, got, seq)
		}
	}

	for _, bad := range []TransactionID{"", "txn-", "txn-abc", "other-1", "1", "txn--1"} {
		if _, ok := SeqForTransactionID(bad); ok {
			t.Errorf("%q parsed as valid", bad)
		}
	}
}

// The table must not grow without limit, or interning reintroduces the
// unbounded growth it was meant to remove.
func TestInterningIsBounded(t *testing.T) {
	strs := NewStrings()

	for i := 0; i < MaxInterned*2; i++ {
		strs.Intern("unique-" + string(rune('a'+i%26)) + itoa(i))
	}
	if got := strs.Len(); got > MaxInterned {
		t.Errorf("table holds %d strings, over the %d limit", got, MaxInterned)
	}

	// A string that could not be added interns to zero, and a caller reading
	// it back gets the empty string rather than a wrong value.
	if got := strs.Lookup(0); got != "" {
		t.Errorf("Ref 0 is %q, want the empty string", got)
	}
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var b []byte
	for n > 0 {
		b = append([]byte{byte('0' + n%10)}, b...)
		n /= 10
	}
	return string(b)
}
