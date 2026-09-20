package core

import (
	"reflect"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Every event must survive a round trip exactly.
//
// This is what makes a hand-written journal format safe. An encoder and
// decoder that disagree corrupt the journal *silently* - the CRC passes,
// because the bytes are precisely what the encoder wrote, so the damage shows
// up as a wrong balance after a reboot rather than as an error anywhere.
//
// So every type is encoded, decoded and compared field by field, including
// the awkward cases: empty strings, nil optionals, set optionals, negative
// amounts, and text with multi-byte runes.
func TestEventsRoundTrip(t *testing.T) {
	str := func(s string) *string { return &s }
	amt := func(a ledger.Amount) *ledger.Amount { return &a }
	st := func(s users.Status) *users.Status { return &s }
	role := func(r users.Role) *users.Role { return &r }
	mst := func(s marketplace.Status) *marketplace.Status { return &s }

	t.Run("userCreated", func(t *testing.T) {
		want := userCreatedEvent{
			User: users.User{
				ID: "user-abc", Username: "nana", DisplayName: "Nana",
				Role: users.RoleNana, Status: users.StatusActive,
				Account: "account-xyz", CreatedAt: 1789733512,
				Verifier: "pbkdf2-sha256$1000$b80kP/PjvkcvsyVoJF6glQ$1NCGi1JQF7ilyBsB",
			},
			Account: users.Account{
				ID: "account-xyz", UserID: "user-abc", Name: "Nana",
				Status: users.StatusActive, CreatedAt: 1789733512,
			},
		}
		if n := eventWireSize(&want); n != len(encodeUserCreated(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got userCreatedEvent
		if err := decodeUserCreated(encodeUserCreated(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("userUpdated/all set", func(t *testing.T) {
		want := userUpdatedEvent{
			ID: "user-abc", DisplayName: str("New Name"),
			Status: st(users.StatusDisabled), Role: role(users.RoleNana),
			Verifier: str("pbkdf2-sha256$1000$x$y"),
		}
		if n := eventWireSize(&want); n != len(encodeUserUpdated(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got userUpdatedEvent
		if err := decodeUserUpdated(encodeUserUpdated(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("userUpdated/all nil", func(t *testing.T) {
		want := userUpdatedEvent{ID: "user-abc"}
		if n := eventWireSize(&want); n != len(encodeUserUpdated(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got userUpdatedEvent
		if err := decodeUserUpdated(encodeUserUpdated(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("nil optionals did not survive\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("transaction", func(t *testing.T) {
		want := transactionEvent{Txn: ledger.Transaction{
			ID: "txn-7", Kind: ledger.KindTransfer, CreatedAt: 99,
			Actor: "user-abc", Description: "for mowing the lawn 🌱",
			Reference: "listing-3", Reverses: "txn-4",
			Postings: []ledger.Posting{
				{Account: "account-a", Amount: -5},
				{Account: "account-b", Amount: 5},
			},
		}}
		if n := eventWireSize(&want); n != len(encodeTransactionEvent(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got transactionEvent
		if err := decodeTransactionEvent(encodeTransactionEvent(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("transaction/empty optionals", func(t *testing.T) {
		want := transactionEvent{Txn: ledger.Transaction{
			ID: "txn-1", Kind: ledger.KindIssue, CreatedAt: 1,
			Actor: "user-abc", Description: "",
			Postings: []ledger.Posting{
				{Account: ledger.SystemIssuance, Amount: -100},
				{Account: "account-a", Amount: 100},
			},
		}}
		if n := eventWireSize(&want); n != len(encodeTransactionEvent(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got transactionEvent
		if err := decodeTransactionEvent(encodeTransactionEvent(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("listingCreated", func(t *testing.T) {
		want := listingCreatedEvent{Listing: marketplace.Listing{
			ID: "listing-1", Seller: "account-a",
			Title:       "One hour of Switch time",
			Description: "Uninterrupted, no siblings,你好",
			Price:       10, Quantity: 1, Status: marketplace.StatusActive,
			CreatedAt: 5, UpdatedAt: 6,
			Kind: "currency", Currency: "USD", MinorUnits: 500,
		}}
		if n := eventWireSize(&want); n != len(encodeListingCreated(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got listingCreatedEvent
		if err := decodeListingCreated(encodeListingCreated(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("listingUpdated", func(t *testing.T) {
		want := listingUpdatedEvent{
			ID: "listing-1", Title: str("New title"),
			Description: str("New description"), Price: amt(42),
			Status: mst(marketplace.StatusCancelled), UpdatedAt: 77,
		}
		if n := eventWireSize(&want); n != len(encodeListingUpdated(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got listingUpdatedEvent
		if err := decodeListingUpdated(encodeListingUpdated(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("listingPurchased", func(t *testing.T) {
		want := listingPurchasedEvent{
			ListingID: "listing-1", Buyer: "account-b", UpdatedAt: 88,
			Txn: ledger.Transaction{
				ID: "txn-9", Kind: ledger.KindPurchase, CreatedAt: 88,
				Actor: "user-b", Description: "One hour of Switch time",
				Reference: "listing-1",
				Postings: []ledger.Posting{
					{Account: "account-b", Amount: -10},
					{Account: "account-a", Amount: 10},
				},
			},
		}
		if n := eventWireSize(&want); n != len(encodeListingPurchased(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got listingPurchasedEvent
		if err := decodeListingPurchased(encodeListingPurchased(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("configUpdated", func(t *testing.T) {
		want := configUpdatedEvent{Config: Config{
			HouseholdName: "The House", InitialGrant: 100, Currency: "NanaCoin",
		}}
		if n := eventWireSize(&want); n != len(encodeConfigUpdated(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got configUpdatedEvent
		if err := decodeConfigUpdated(encodeConfigUpdated(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})

	t.Run("idempotency", func(t *testing.T) {
		want := idempotencyEvent{
			Key: "client-key-1", UserID: "user-abc", Endpoint: "transfer",
			Result: []byte(`{"id":"txn-7","amount":5}`), At: 123,
		}
		if n := eventWireSize(&want); n != len(encodeIdempotency(nil, &want)) {
			t.Fatalf("wire size mismatch: %d", n)
		}
		var got idempotencyEvent
		if err := decodeIdempotency(encodeIdempotency(nil, &want), &got); err != nil {
			t.Fatalf("decode: %v", err)
		}
		if !reflect.DeepEqual(want, got) {
			t.Errorf("round trip changed the event\n want %+v\n  got %+v", want, got)
		}
	})
}

// A truncated or corrupt record must fail cleanly, not read past the end or
// allocate wildly. The CRC catches most corruption, but a record whose length
// field survived a bit flip would still reach the decoder.
func TestCorruptRecordsFailCleanly(t *testing.T) {
	good := encodeUserCreated(nil, &userCreatedEvent{
		User:    users.User{ID: "user-a", Username: "a", Account: "acct-a"},
		Account: users.Account{ID: "acct-a", UserID: "user-a"},
	})

	for n := 0; n < len(good); n++ {
		var e userCreatedEvent
		if err := decodeUserCreated(good[:n], &e); err == nil {
			t.Errorf("truncating to %d bytes decoded without error", n)
		}
	}

	// Trailing bytes must be refused: they mean a version mismatch, and
	// ignoring them silently is how a format change becomes a wrong balance.
	var e userCreatedEvent
	if err := decodeUserCreated(append(good, 0xFF), &e); err == nil {
		t.Error("trailing bytes decoded without error")
	}
}

// The binary format should be materially smaller than the JSON it replaced.
// Not the reason for the change, but worth pinning: a regression here would
// mean an encoder writing something it should not.
func TestBinaryIsSmallerThanJSON(t *testing.T) {
	e := userCreatedEvent{
		User: users.User{
			ID: "user-3NnFci9iwEqF", Username: "nana", DisplayName: "Nana",
			Role: users.RoleNana, Status: users.StatusActive,
			Account: "account-THF-ubUaDsN8", CreatedAt: 1789733512,
			Verifier: "pbkdf2-sha256$1000$b80kP/PjvkcvsyVoJF6glQ$1NCGi1JQF7ilyBsBlJa6iFKHITedZX5ykSOJS9KPzWg",
		},
		Account: users.Account{
			ID: "account-THF-ubUaDsN8", UserID: "user-3NnFci9iwEqF",
			Name: "Nana", Status: users.StatusActive, CreatedAt: 1789733512,
		},
	}
	got := len(encodeUserCreated(nil, &e))
	const wasJSON = 389
	t.Logf("USER_CREATED: %d bytes binary, %d bytes as JSON (%.0f%% smaller)",
		got, wasJSON, 100*(1-float64(got)/wasJSON))
	if got >= wasJSON {
		t.Errorf("binary encoding is %d bytes, JSON was %d", got, wasJSON)
	}
}

// Encoding must not allocate when the caller supplies a buffer. That is the
// property the service relies on: one reusable buffer, no per-commit garbage.
func TestEncodingReusesTheBuffer(t *testing.T) {
	e := transactionEvent{Txn: ledger.Transaction{
		ID: "txn-1", Kind: ledger.KindTransfer, CreatedAt: 1,
		Actor: "user-a", Description: "chores",
		Postings: []ledger.Posting{
			{Account: "account-a", Amount: -1},
			{Account: "account-b", Amount: 1},
		},
	}}

	buf := make([]byte, 0, 1024)
	allocs := testing.AllocsPerRun(200, func() {
		buf = encodeTransactionEvent(buf[:0], &e)
	})
	t.Logf("allocations per encode with a warm buffer: %.0f", allocs)
	if allocs > 0 {
		t.Errorf("encoding allocated %.0f times, want 0", allocs)
	}
}
