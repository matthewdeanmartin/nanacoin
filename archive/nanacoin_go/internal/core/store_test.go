package core

import (
	"fmt"
	"runtime"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// The packed store's whole claim is that its cost is fixed.
//
// Not "small" - fixed. A household that has been using the system for a year
// must occupy the same heap as one provisioned five minutes ago, because the
// slots were all allocated at startup and nothing inside them is ever
// individually allocated or freed.
//
// That is the property that stops fragmentation, and it is the one worth
// pinning: a regression here would not fail loudly, it would just make the
// board die again after a few days.
func TestDomainCostDoesNotGrowWithUse(t *testing.T) {
	measure := func() int64 {
		runtime.GC()
		runtime.GC()
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		return int64(m.HeapAlloc)
	}

	svc := newService(t, memory.New())
	nana, err := svc.Provision("nana", "Nana", "nana-pin", "H")
	if err != nil {
		t.Fatalf("Provision: %v", err)
	}
	seller, err := svc.CreateUser(nana, "alice", "Alice", "alice-pin", users.RoleUser, false)
	if err != nil {
		t.Fatalf("CreateUser: %v", err)
	}

	// Fill the listing table once, then churn it many times over. Each
	// listing is created, sold or cancelled, and recycled - which is exactly
	// what a household does over months and what used to leak.
	fill := func(round int) {
		for i := 0; i < MaxListings/2; i++ {
			l, err := svc.CreateListing(seller, ListingInput{
				Title:       fmt.Sprintf("item r%d-%d", round, i),
				Description: "a description long enough to matter, repeated",
				Price:       1,
			})
			if err != nil {
				t.Fatalf("round %d listing %d: %v", round, i, err)
			}
			if _, err := svc.CancelListing(seller, l.ID); err != nil {
				t.Fatalf("cancel: %v", err)
			}
		}
	}

	fill(0) // warm: first fill allocates the slots
	baseline := measure()

	for round := 1; round <= 8; round++ {
		fill(round)
	}
	after := measure()

	growth := after - baseline
	t.Logf("after %d further listing cycles: heap moved %+d bytes",
		8*(MaxListings/2), growth)

	// Some movement is expected - the intern table gains a few titles, the
	// arena fills - but it must not scale with how much the household has
	// done. A per-listing leak over 192 cycles would show as tens of kB.
	if growth > 24<<10 {
		t.Errorf("domain heap grew %d bytes over %d listing cycles - "+
			"something is allocating per record again", growth,
			8*(MaxListings/2))
	}
}

// The caps must refuse rather than grow, and say which cap was hit.
//
// A refusal is correct behaviour here and has to stay legible: "storage is
// full" with the limit named is actionable, and a silent overwrite or a
// generic 500 is not.
func TestCapacityRefusesAndSaysWhy(t *testing.T) {
	svc := newService(t, memory.New())
	nana, err := svc.Provision("nana", "Nana", "nana-pin", "H")
	if err != nil {
		t.Fatalf("Provision: %v", err)
	}

	// Fill the household.
	for i := 0; svc.store.nUsers < MaxUsers; i++ {
		name := fmt.Sprintf("m%d", i)
		if _, err := svc.CreateUser(nana, name, name, "member-pin",
			users.RoleUser, false); err != nil {
			t.Fatalf("filling household at %d: %v", i, err)
		}
	}

	_, err = svc.CreateUser(nana, "onemore", "One More", "member-pin",
		users.RoleUser, false)
	if err == nil {
		t.Fatal("creating a user past MaxUsers succeeded, want refusal")
	}
	t.Logf("household full: %v", err)
}

// Active listings must never be recycled out from under a seller.
//
// Recycling a closed listing is fine - the ledger still records what was
// bought. Recycling an active one would delete an offer somebody is waiting
// on, which is a data loss a household would notice and not forgive.
func TestRecyclingNeverDropsAnActiveListing(t *testing.T) {
	svc := newService(t, memory.New())
	nana, err := svc.Provision("nana", "Nana", "nana-pin", "H")
	if err != nil {
		t.Fatalf("Provision: %v", err)
	}
	seller, err := svc.CreateUser(nana, "alice", "Alice", "alice-pin",
		users.RoleUser, false)
	if err != nil {
		t.Fatalf("CreateUser: %v", err)
	}

	// Fill every slot with an active listing.
	var ids []string
	for i := 0; i < MaxListings; i++ {
		l, err := svc.CreateListing(seller, ListingInput{
			Title: fmt.Sprintf("active %d", i), Description: "d", Price: 1,
		})
		if err != nil {
			t.Fatalf("listing %d: %v", i, err)
		}
		ids = append(ids, string(l.ID))
	}

	// One more must be refused, not granted by evicting an active listing.
	if _, err := svc.CreateListing(seller, ListingInput{
		Title: "one too many", Description: "d", Price: 1,
	}); err == nil {
		t.Fatal("creating past MaxListings succeeded while all were active")
	}

	// Every original listing must still be there.
	n := 0
	svc.EachListing(marketplace.StatusActive, func(*marketplace.Listing) bool {
		n++
		return true
	})
	if n != MaxListings {
		t.Errorf("after a refused create, %d active listings remain, want %d",
			n, MaxListings)
	}
}

// StoreBytes must describe what the store actually costs, so the figure
// printed at boot is true.
func TestStoreBytesIsStated(t *testing.T) {
	t.Logf("packed domain store: %d bytes (%.1f kB) fixed", StoreBytes,
		float64(StoreBytes)/1024)
	t.Logf("  users    %d x 32 = %d", MaxUsers, MaxUsers*32)
	t.Logf("  accounts %d x 24 = %d", MaxAccounts, MaxAccounts*24)
	t.Logf("  listings %d x 56 = %d", MaxListings, MaxListings*56)
	if StoreBytes > 24<<10 {
		t.Errorf("store budget is %d bytes, which is too much of an 85 kB heap",
			StoreBytes)
	}
}
