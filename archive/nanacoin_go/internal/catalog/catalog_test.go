package catalog

import "testing"

// The catalog is generated, so these do not test the data so much as the
// invariants the generator promises and Lookup depends on. A code is what the
// ledger stores: if this table is wrong, every record carrying that code is
// wrong too, and nothing else in the system would notice.

func TestCodesAreSortedAndUnique(t *testing.T) {
	// Lookup binary searches. Out-of-order entries would make it miss items
	// that are present, which reads as "unknown code" rather than as a bug.
	for i := 1; i < len(Items); i++ {
		if Items[i].Code <= Items[i-1].Code {
			t.Fatalf("Items[%d].Code = %d is not greater than Items[%d].Code = %d",
				i, Items[i].Code, i-1, Items[i-1].Code)
		}
	}
}

func TestEveryItemIsFindable(t *testing.T) {
	for i := range Items {
		got, ok := Lookup(Items[i].Code)
		if !ok {
			t.Errorf("Lookup(%d) found nothing", Items[i].Code)
			continue
		}
		if got.Name != Items[i].Name {
			t.Errorf("Lookup(%d) = %q, want %q", Items[i].Code, got.Name, Items[i].Name)
		}
	}
}

func TestUnknownCodes(t *testing.T) {
	// Zero is "no code", not a lookup failure - a listing with a custom title
	// and no catalog entry carries 0.
	if Valid(0) {
		t.Error("Valid(0) is true; zero means no code")
	}
	if _, ok := Lookup(65535); ok {
		t.Error("Lookup found an item at 65535")
	}
	if Name(9999) != "" {
		t.Errorf("Name(9999) = %q, want empty", Name(9999))
	}
}

func TestEveryItemHasAKnownCategory(t *testing.T) {
	known := make(map[uint8]bool, len(Categories))
	for i := range Categories {
		known[Categories[i].ID] = true
	}
	for i := range Items {
		if !known[Items[i].Cat] {
			t.Errorf("item %d (%q) has category %d, which does not exist",
				Items[i].Code, Items[i].Name, Items[i].Cat)
		}
	}
}

func TestNamesAreNotEmpty(t *testing.T) {
	for i := range Items {
		if Items[i].Name == "" {
			t.Errorf("item %d has no name", Items[i].Code)
		}
	}
}

// Currency entries stand for real money and are the ones whose numbers matter:
// "$5 of real cash" with minor_units 50 would misprice every such listing.
func TestCurrencyItemsAreComplete(t *testing.T) {
	for i := range Items {
		it := &Items[i]
		if it.Currency == "" && it.MinorUnits == 0 {
			continue
		}
		if it.Currency == "" {
			t.Errorf("item %d has minor units but no currency", it.Code)
		}
		if it.MinorUnits <= 0 {
			t.Errorf("item %d has currency %q but no positive minor units", it.Code, it.Currency)
		}
	}
}

// The catalog lives in flash, not RAM. This is not a memory test - it cannot
// be, from inside Go - but it does pin the count, so that a generator change
// that accidentally dropped or duplicated entries fails here rather than
// showing up as missing items in the client.
func TestExpectedSize(t *testing.T) {
	if len(Items) != 250 {
		t.Errorf("catalog has %d items, want 250", len(Items))
	}
	if len(Categories) != 10 {
		t.Errorf("catalog has %d categories, want 10", len(Categories))
	}
}
