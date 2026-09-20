package ledger

import (
	"strings"
	"testing"
)

func TestArenaReplacementIsAllOrNothing(t *testing.T) {
	a := NewArena()
	old := [3]Slot{a.Put("title"), a.Put("description"), a.Put("listing-id")}
	before, _, truncated := a.Stats()
	p := TextReplacement{Old: old, Values: [3]string{"changed", strings.Repeat("x", ArenaSize), "new-id"}}
	ok := a.Replace(&p)
	used, _, trunc := a.Stats()
	if ok || used != before || trunc != truncated || a.Get(old[2]) != "listing-id" || a.Get(old[0]) != "title" {
		t.Fatal("failed replacement mutated text")
	}
	p.Values = [3]string{"new title", "new description", "new-id"}
	ok = a.Replace(&p)
	slots := p.Slots
	if !ok || a.Get(slots[2]) != "new-id" {
		t.Fatal("replacement failed")
	}
	if n := testing.AllocsPerRun(100, func() { p.Old = slots; ok = a.Replace(&p); slots = p.Slots }); n != 0 || !ok {
		t.Fatal(n, ok)
	}
}

func TestReplacementReusesFragmentedExistingSlots(t *testing.T) {
	a := NewArena()
	// Physical order differs from field order: naive first-fit puts the title
	// in the ID block, leaving two separate one-block holes for a two-block ID.
	id := a.Put(strings.Repeat("i", 20))
	a.Put("occupied spacer")
	desc := a.Put(strings.Repeat("d", 200))
	a.Put("occupied spacer")
	title := a.Put("old title")
	for {
		if a.Put("0123456789abcdef").IsEmpty() {
			break
		}
	}
	p := TextReplacement{Old: [3]Slot{title, desc, id}, Values: [3]string{"new title", strings.Repeat("e", 200), strings.Repeat("j", 20)}}
	ok := a.Replace(&p)
	slots := p.Slots
	if !ok || a.Get(slots[2]) != strings.Repeat("j", 20) {
		t.Fatal("existing equal-sized fields always fit in their own blocks")
	}
}
