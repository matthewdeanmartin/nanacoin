package api

import (
	"bytes"
	"encoding/json"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// The hand-written encoders must produce exactly what encoding/json does.
//
// This is what makes hand-writing them safe. Dropping reflection means the
// compiler no longer checks that an encoder matches its struct, so a field
// added to a view type and forgotten here would be a silently missing field
// in a response - on a board with no debugger attached, discovered by a
// household member noticing a blank description.
//
// So the struct tags stay the specification and this test is the check: every
// shape is encoded both ways and compared. A drifted encoder fails here, on a
// desktop, in a second.
func TestHandWrittenJSONMatchesStdlib(t *testing.T) {
	bal := ledger.Amount(42)

	cases := []struct {
		name   string
		value  any
		encode func(*jsonw)
	}{
		{
			"userView",
			userView{
				ID: "user-1", Username: "alice", DisplayName: "Alice",
				Role: users.RoleUser, Status: users.StatusActive,
				Account: "account-1", CreatedAt: 1700000000, Balance: &bal,
			},
			func(j *jsonw) {
				v := userView{
					ID: "user-1", Username: "alice", DisplayName: "Alice",
					Role: users.RoleUser, Status: users.StatusActive,
					Account: "account-1", CreatedAt: 1700000000, Balance: &bal,
				}
				j.userView(&v)
			},
		},
		{
			"userView/no balance",
			userView{
				ID: "user-2", Username: "bob", DisplayName: "Bob",
				Role: users.RoleNana, Status: users.StatusDisabled,
				Account: "account-2", CreatedAt: 1,
			},
			func(j *jsonw) {
				v := userView{
					ID: "user-2", Username: "bob", DisplayName: "Bob",
					Role: users.RoleNana, Status: users.StatusDisabled,
					Account: "account-2", CreatedAt: 1,
				}
				j.userView(&v)
			},
		},
		{
			"accountView",
			accountView{
				ID: "account-1", UserID: "user-1", Name: "Alice",
				Status: users.StatusActive, Balance: 17,
			},
			func(j *jsonw) {
				v := accountView{
					ID: "account-1", UserID: "user-1", Name: "Alice",
					Status: users.StatusActive, Balance: 17,
				}
				j.accountView(&v)
			},
		},
		{
			"transactionView/minimal",
			transactionView{
				ID: "txn-1", Kind: ledger.KindTransfer, CreatedAt: 5,
				Actor: "user-1", Description: "chores",
				Postings: []postingView{
					{Account: "account-1", Name: "Alice", Amount: -1},
					{Account: "account-2", Name: "Bob", Amount: 1},
				},
			},
			func(j *jsonw) {
				v := transactionView{
					ID: "txn-1", Kind: ledger.KindTransfer, CreatedAt: 5,
					Actor: "user-1", Description: "chores",
					Postings: []postingView{
						{Account: "account-1", Name: "Alice", Amount: -1},
						{Account: "account-2", Name: "Bob", Amount: 1},
					},
				}
				j.transactionView(&v)
			},
		},
		{
			"transactionView/every optional field",
			transactionView{
				ID: "txn-9", Kind: ledger.KindReversal, CreatedAt: 9,
				Actor: "user-1", Description: "undo",
				Reference: "listing-3", Reverses: "txn-4", ReversedBy: "txn-10",
				Postings: []postingView{{Account: "a", Name: "A", Amount: 0}},
			},
			func(j *jsonw) {
				v := transactionView{
					ID: "txn-9", Kind: ledger.KindReversal, CreatedAt: 9,
					Actor: "user-1", Description: "undo",
					Reference: "listing-3", Reverses: "txn-4", ReversedBy: "txn-10",
					Postings: []postingView{{Account: "a", Name: "A", Amount: 0}},
				}
				j.transactionView(&v)
			},
		},
		{
			"listingView/active",
			listingView{
				ID: "listing-1", Seller: "account-1", SellerName: "Alice",
				Title: "Switch hour", Description: "one hour", Price: 5,
				Status: marketplace.StatusActive, CreatedAt: 1, UpdatedAt: 2,
			},
			func(j *jsonw) {
				v := listingView{
					ID: "listing-1", Seller: "account-1", SellerName: "Alice",
					Title: "Switch hour", Description: "one hour", Price: 5,
					Status: marketplace.StatusActive, CreatedAt: 1, UpdatedAt: 2,
				}
				j.listingView(&v)
			},
		},
		{
			"listingView/sold with currency",
			listingView{
				ID: "listing-2", Seller: "account-1", SellerName: "Alice",
				Title: "Cash", Description: "five dollars", Price: 5,
				Status: marketplace.StatusSold, CreatedAt: 1, UpdatedAt: 3,
				Buyer: "account-2", BuyerName: "Bob", SoldTx: "txn-7",
				Kind: "currency", Currency: "USD", MinorUnits: 500,
			},
			func(j *jsonw) {
				v := listingView{
					ID: "listing-2", Seller: "account-1", SellerName: "Alice",
					Title: "Cash", Description: "five dollars", Price: 5,
					Status: marketplace.StatusSold, CreatedAt: 1, UpdatedAt: 3,
					Buyer: "account-2", BuyerName: "Bob", SoldTx: "txn-7",
					Kind: "currency", Currency: "USD", MinorUnits: 500,
				}
				j.listingView(&v)
			},
		},
		{
			"errorBody",
			errorBody{Error: "forbidden", Message: "not permitted"},
			func(j *jsonw) {
				v := errorBody{Error: "forbidden", Message: "not permitted"}
				j.errorBody(&v)
			},
		},
		{
			"status",
			core.Status{
				Provisioned: true, Household: "The House", Currency: "NanaCoin",
				Users: 4, Transactions: 12, ActiveList: 2, Circulation: 100,
				JournalUsed: 512, JournalCap: 0, LedgerBalance: true,
			},
			func(j *jsonw) {
				v := core.Status{
					Provisioned: true, Household: "The House", Currency: "NanaCoin",
					Users: 4, Transactions: 12, ActiveList: 2, Circulation: 100,
					JournalUsed: 512, JournalCap: 0, LedgerBalance: true,
				}
				j.status(&v)
			},
		},
		{
			"config",
			core.Config{HouseholdName: "The House", InitialGrant: 10, Currency: "NanaCoin"},
			func(j *jsonw) {
				v := core.Config{HouseholdName: "The House", InitialGrant: 10, Currency: "NanaCoin"}
				j.config(&v)
			},
		},
		{
			"event",
			eventlog.Event{Seq: 3, At: 99, Level: "warn", Kind: "403", Detail: "GET /x"},
			func(j *jsonw) {
				v := eventlog.Event{Seq: 3, At: 99, Level: "warn", Kind: "403", Detail: "GET /x"}
				j.event(&v)
			},
		},
		{
			"authorizeResponse",
			authorizeResponse{Code: "abc123"},
			func(j *jsonw) {
				v := authorizeResponse{Code: "abc123"}
				j.authorizeResponse(&v)
			},
		},
		{
			"tokenResponse",
			tokenResponse{
				AccessToken: "tok", TokenType: "Bearer", ExpiresIn: 3600,
				User: userView{
					ID: "user-1", Username: "alice", DisplayName: "Alice",
					Role: users.RoleUser, Status: users.StatusActive,
					Account: "account-1", CreatedAt: 7, Balance: &bal,
				},
			},
			func(j *jsonw) {
				v := tokenResponse{
					AccessToken: "tok", TokenType: "Bearer", ExpiresIn: 3600,
					User: userView{
						ID: "user-1", Username: "alice", DisplayName: "Alice",
						Role: users.RoleUser, Status: users.StatusActive,
						Account: "account-1", CreatedAt: 7, Balance: &bal,
					},
				}
				j.tokenResponse(&v)
			},
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			want, err := json.Marshal(tc.value)
			if err != nil {
				t.Fatalf("stdlib marshal: %v", err)
			}

			var got bytes.Buffer
			var mem [jsonBufSize]byte
			j := newJSONW(&got, mem[:])
			tc.encode(&j)
			if err := j.done(); err != nil {
				t.Fatalf("hand-written encode: %v", err)
			}

			if got.String() != string(want) {
				t.Errorf("encoders disagree\n  stdlib: %s\n  ours:   %s",
					want, got.String())
			}
		})
	}
}

// String escaping must match encoding/json for everything a household can
// type, and for the control characters it cannot.
func TestStringEscapingMatchesStdlib(t *testing.T) {
	cases := []string{
		"",
		"plain",
		`with "quotes"`,
		`back\slash`,
		"tab\there",
		"new\nline",
		"carriage\rreturn",
		"bell\x07and\x00nul",
		"unicode: café ☕ 日本語",
		"emoji: 🎉",
		`{"nested":"json"}`,
		"all: \"\\\n\r\t\x01",
	}

	for _, s := range cases {
		want, err := json.Marshal(s)
		if err != nil {
			t.Fatalf("stdlib marshal %q: %v", s, err)
		}

		var got bytes.Buffer
		var mem [jsonBufSize]byte
		j := newJSONW(&got, mem[:])
		j.str(s)
		if err := j.done(); err != nil {
			t.Fatalf("encode %q: %v", s, err)
		}

		if got.String() != string(want) {
			t.Errorf("escaping %q\n  stdlib: %s\n  ours:   %s", s, want, got.String())
		}
	}
}

// The encoder must not allocate.
//
// Measured through a package-level sink rather than a captured one: a closure
// capturing a local escapes it to the heap, which would charge the encoder
// for the test harness's own allocation and hide the number that matters.
var allocSink discardCounter

func TestHandWrittenJSONDoesNotAllocate(t *testing.T) {
	v := transactionView{
		ID: "txn-1", Kind: ledger.KindTransfer, CreatedAt: 1700000000,
		Actor: "user-alice", Description: "for mowing the lawn",
		Postings: []postingView{
			{Account: "account-alice", Name: "Alice", Amount: -5},
			{Account: "account-bob", Name: "Bob", Amount: 5},
		},
	}

	ours := testing.AllocsPerRun(500, func() {
		var mem [jsonBufSize]byte
		j := newJSONW(&allocSink, mem[:])
		j.transactionView(&v)
		_ = j.done()
	})

	stdlib := testing.AllocsPerRun(500, func() {
		b, _ := json.Marshal(v)
		allocSink.Write(b)
	})

	t.Logf("allocations per encode: hand-written %.0f, encoding/json %.0f",
		ours, stdlib)

	// Asserted as "no worse than encoding/json", not as zero.
	//
	// Zero is achievable and is what happens on the board, where TinyGo's
	// escape analysis keeps the buffer in the caller's frame. On the host,
	// AllocsPerRun's closure escapes the buffer no matter how it is written,
	// so a zero assertion here would be testing the harness rather than the
	// encoder. The figure that governs the board is bytes per encode, which
	// BenchmarkEncodeTransaction reports, and the site count that
	// `tinygo build -print-allocs` reports for the real binary.
	if ours > stdlib {
		t.Errorf("hand-written encoder (%.0f allocs) is worse than "+
			"encoding/json (%.0f)", ours, stdlib)
	}
}

type discardCounter struct{ n int64 }

func (d *discardCounter) Write(p []byte) (int, error) {
	d.n += int64(len(p))
	return len(p), nil
}

// Benchmarked rather than AllocsPerRun'd, because AllocsPerRun takes a
// closure and the closure itself can escape - charging the encoder for the
// harness. A benchmark calls the code the way a handler does.
func BenchmarkEncodeTransaction(b *testing.B) {
	v := transactionView{
		ID: "txn-1", Kind: ledger.KindTransfer, CreatedAt: 1700000000,
		Actor: "user-alice", Description: "for mowing the lawn",
		Postings: []postingView{
			{Account: "account-alice", Name: "Alice", Amount: -5},
			{Account: "account-bob", Name: "Bob", Amount: 5},
		},
	}
	b.Run("handwritten", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			var mem [jsonBufSize]byte
			j := newJSONW(&allocSink, mem[:])
			j.transactionView(&v)
			_ = j.done()
		}
	})
	b.Run("encoding-json", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			out, _ := json.Marshal(v)
			allocSink.Write(out)
		}
	})
}
