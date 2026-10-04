package core

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/flashlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

func testOptions(seq *int) Options {
	return Options{
		Now: func() time.Time { return time.Unix(1_700_000_000, 0) },
		NewID: func(prefix string) string {
			*seq++
			return fmt.Sprintf("%s-%d", prefix, *seq)
		},
	}
}

func newService(t *testing.T, j storage.Journal) *Service {
	t.Helper()
	var seq int
	svc, err := New(j, testOptions(&seq))
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	return svc
}

// household provisions Nana and two members.
func household(t *testing.T, svc *Service) (nana, alice, bob *users.User) {
	t.Helper()
	nana, err := svc.Provision("nana", "Nana", "nana-pin", "The House")
	if err != nil {
		t.Fatalf("Provision: %v", err)
	}
	alice, err = svc.CreateUser(nana, "alice", "Alice", "alice-pin", users.RoleUser, true)
	if err != nil {
		t.Fatalf("CreateUser alice: %v", err)
	}
	bob, err = svc.CreateUser(nana, "bob", "Bob", "bob-pin", users.RoleUser, true)
	if err != nil {
		t.Fatalf("CreateUser bob: %v", err)
	}
	return nana, alice, bob
}

// The durability claim: everything committed before a reboot is there after
// it, reconstructed from the journal alone.
func TestStateSurvivesReboot(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nanacoin.journal")

	j1, err := flashlog.Open(path, 0)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	svc1 := newService(t, j1)
	nana, alice, bob := household(t, svc1)

	if _, err := svc1.Transfer(alice, bob.Account, 30, "chores"); err != nil {
		t.Fatalf("Transfer: %v", err)
	}
	listing, err := svc1.CreateListing(bob, ListingInput{Title: "LEGO", Price: 25})
	if err != nil {
		t.Fatalf("CreateListing: %v", err)
	}
	if _, _, err := svc1.Purchase(alice, listing.ID); err != nil {
		t.Fatalf("Purchase: %v", err)
	}
	if _, err := svc1.SetConfig(nana, Config{HouseholdName: "The House", InitialGrant: 250, Currency: "NanaCoin"}); err != nil {
		t.Fatalf("SetConfig: %v", err)
	}

	beforeAlice := svc1.Balance(alice.Account)
	beforeBob := svc1.Balance(bob.Account)
	beforeTxns := svc1.Status().Transactions
	svc1.Close()

	// Reboot: a brand new service over the same journal file.
	j2, err := flashlog.Open(path, 0)
	if err != nil {
		t.Fatalf("reopen: %v", err)
	}
	svc2 := newService(t, j2)
	defer svc2.Close()

	if got := svc2.Balance(alice.Account); got != beforeAlice {
		t.Errorf("alice has %d after reboot, want %d", got, beforeAlice)
	}
	if got := svc2.Balance(bob.Account); got != beforeBob {
		t.Errorf("bob has %d after reboot, want %d", got, beforeBob)
	}
	if got := svc2.Status().Transactions; got != beforeTxns {
		t.Errorf("%d transactions after reboot, want %d", got, beforeTxns)
	}
	if got := svc2.Config().InitialGrant; got != 250 {
		t.Errorf("initial grant is %d after reboot, want 250", got)
	}
	if l, ok := svc2.Listing(listing.ID); !ok || l.Status != "SOLD" {
		t.Errorf("listing did not survive as SOLD: %+v", l)
	}

	// Credentials must survive too, or a power cut locks the household out.
	if _, err := svc2.Authenticate("alice", "alice-pin"); err != nil {
		t.Errorf("alice cannot log in after reboot: %v", err)
	}
	if err := svc2.book.CheckInvariants(); err != nil {
		t.Errorf("invariants after reboot: %v", err)
	}
}

// If the journal write fails, the RAM model must not move. Otherwise the
// server would report a transfer that the next reboot silently undoes.
func TestFailedJournalWriteLeavesNoTrace(t *testing.T) {
	j := memory.New()
	svc := newService(t, j)
	_, alice, bob := household(t, svc)

	before := svc.Balance(alice.Account)
	beforeTxns := svc.Status().Transactions

	// Fail every append from here on, as a full or dead flash would. The
	// memory backend counts appends, so "after the ones already made" is
	// where the failures start.
	diskFull := errors.New("flash write failed")
	j.SetFailure(j.Appends(), diskFull)

	_, err := svc.Transfer(alice, bob.Account, 10, "will not land")
	if err == nil {
		t.Fatal("transfer succeeded despite a failing journal")
	}
	if !errors.Is(err, diskFull) {
		t.Errorf("error is %v, want it to wrap the journal failure", err)
	}
	if got := svc.Balance(alice.Account); got != before {
		t.Errorf("alice has %d after a failed journal write, want %d", got, before)
	}
	if got := svc.Status().Transactions; got != beforeTxns {
		t.Errorf("%d transactions after a failed write, want %d", got, beforeTxns)
	}
}

func TestProvisionOnlyOnce(t *testing.T) {
	svc := newService(t, memory.New())
	if _, err := svc.Provision("nana", "Nana", "pin1", ""); err != nil {
		t.Fatalf("Provision: %v", err)
	}
	if _, err := svc.Provision("other", "Other", "pin2", ""); !errors.Is(err, ErrForbidden) {
		t.Errorf("second provision: got %v, want ErrForbidden", err)
	}
}

func TestUsernamesAreUniqueCaseInsensitively(t *testing.T) {
	svc := newService(t, memory.New())
	nana, _, _ := household(t, svc)

	// "Alice" and "alice" being different people is a household confusion
	// waiting to happen, and a plausible impersonation route.
	if _, err := svc.CreateUser(nana, "ALICE", "Impostor", "pin1234", users.RoleUser, false); !errors.Is(err, ErrUsernameTaken) {
		t.Errorf("got %v, want ErrUsernameTaken", err)
	}
}

func TestAuthenticateRejectsWrongPasswordAndDisabledUsers(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, _ := household(t, svc)

	if _, err := svc.Authenticate("alice", "wrong"); err == nil {
		t.Error("a wrong password authenticated")
	}
	if _, err := svc.Authenticate("nobody", "anything"); err == nil {
		t.Error("an unknown user authenticated")
	}

	disabled := users.StatusDisabled
	if _, err := svc.UpdateUser(nana, alice.ID, nil, &disabled, nil, nil); err != nil {
		t.Fatalf("disable: %v", err)
	}
	if _, err := svc.Authenticate("alice", "alice-pin"); !errors.Is(err, ErrDisabled) {
		t.Errorf("disabled user: got %v, want ErrDisabled", err)
	}
}

func TestDisabledUserCannotReceiveOrSend(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, bob := household(t, svc)

	disabled := users.StatusDisabled
	if _, err := svc.UpdateUser(nana, bob.ID, nil, &disabled, nil, nil); err != nil {
		t.Fatalf("disable bob: %v", err)
	}

	if _, err := svc.Transfer(alice, bob.Account, 10, ""); !errors.Is(err, ErrDisabled) {
		t.Errorf("transfer to a disabled user: got %v, want ErrDisabled", err)
	}
	if _, err := svc.Transfer(bob, alice.Account, 10, ""); !errors.Is(err, ErrDisabled) {
		t.Errorf("transfer from a disabled user: got %v, want ErrDisabled", err)
	}
}

func TestOrdinaryUsersCannotIssueOrReverse(t *testing.T) {
	svc := newService(t, memory.New())
	_, alice, bob := household(t, svc)

	if _, err := svc.Issue(alice, alice.Account, 1000, "mine"); !errors.Is(err, ErrForbidden) {
		t.Errorf("issue by a user: got %v, want ErrForbidden", err)
	}
	if _, err := svc.Retire(alice, bob.Account, 10, "yours"); !errors.Is(err, ErrForbidden) {
		t.Errorf("retire by a user: got %v, want ErrForbidden", err)
	}

	txn, err := svc.Transfer(alice, bob.Account, 10, "payment")
	if err != nil {
		t.Fatalf("Transfer: %v", err)
	}
	if _, err := svc.Reverse(alice, txn.ID, "oops"); !errors.Is(err, ErrForbidden) {
		t.Errorf("reverse by a user: got %v, want ErrForbidden", err)
	}
}

func TestIdempotencyIsDurable(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nanacoin.journal")

	j1, _ := flashlog.Open(path, 0)
	svc1 := newService(t, j1)
	_, alice, bob := household(t, svc1)

	// The callback returns encoded bytes now, not a value to marshal - see
	// Idempotent. A test only needs them to be stable and comparable.
	first, err := svc1.Idempotent(alice.ID, "transfer", "retry-me", func() ([]byte, error) {
		txn, err := svc1.Transfer(alice, bob.Account, 40, "chores")
		if err != nil {
			return nil, err
		}
		return []byte(txn.ID), nil
	})
	if err != nil {
		t.Fatalf("first attempt: %v", err)
	}
	afterFirst := svc1.Balance(alice.Account)
	svc1.Close()

	// The retry arrives after a power cut, which is exactly the case the
	// spec calls out (20).
	j2, _ := flashlog.Open(path, 0)
	svc2 := newService(t, j2)
	defer svc2.Close()

	second, err := svc2.Idempotent(alice.ID, "transfer", "retry-me", func() ([]byte, error) {
		t.Error("the operation ran again after a reboot")
		txn, err := svc2.Transfer(alice, bob.Account, 40, "chores")
		if err != nil {
			return nil, err
		}
		return []byte(txn.ID), nil
	})
	if err != nil {
		t.Fatalf("retry: %v", err)
	}
	if string(first) != string(second) {
		t.Errorf("retry returned a different result:\n first: %s\nsecond: %s", first, second)
	}
	if got := svc2.Balance(alice.Account); got != afterFirst {
		t.Errorf("alice has %d after the retry, want %d", got, afterFirst)
	}
}

// A failed operation must not be memoised: retrying after being paid should
// work rather than replaying the refusal.
func TestIdempotencyDoesNotCacheFailures(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, bob := household(t, svc)

	if _, err := svc.Idempotent(alice.ID, "transfer", "k1", func() ([]byte, error) {
		txn, err := svc.Transfer(alice, bob.Account, 500, "too much")
		if err != nil {
			return nil, err
		}
		return []byte(txn.ID), nil
	}); err == nil {
		t.Fatal("an unaffordable transfer succeeded")
	}

	if _, err := svc.Issue(nana, alice.Account, 500, "allowance"); err != nil {
		t.Fatalf("Issue: %v", err)
	}
	if _, err := svc.Idempotent(alice.ID, "transfer", "k1", func() ([]byte, error) {
		txn, err := svc.Transfer(alice, bob.Account, 500, "now affordable")
		if err != nil {
			return nil, err
		}
		return []byte(txn.ID), nil
	}); err != nil {
		t.Errorf("retry after being funded: %v", err)
	}
	if got := svc.Balance(bob.Account); got != 600 {
		t.Errorf("bob has %d, want 600", got)
	}
}

func TestPurchaseIsOneJournalRecord(t *testing.T) {
	j := memory.New()
	svc := newService(t, j)
	_, alice, bob := household(t, svc)

	listing, err := svc.CreateListing(bob, ListingInput{Title: "LEGO", Price: 25})
	if err != nil {
		t.Fatalf("CreateListing: %v", err)
	}

	var before int
	j.Replay(func(*storage.Record) error { before++; return nil })

	if _, _, err := svc.Purchase(alice, listing.ID); err != nil {
		t.Fatalf("Purchase: %v", err)
	}

	var after int
	var purchaseRecords int
	j.Replay(func(r *storage.Record) error {
		after++
		if r.Type == storage.TypeListingPurchased {
			purchaseRecords++
		}
		return nil
	})

	// Two records would mean two chances to half-land (spec 13).
	if after-before != 1 {
		t.Errorf("a purchase wrote %d records, want 1", after-before)
	}
	if purchaseRecords != 1 {
		t.Errorf("found %d LISTING_PURCHASED records, want 1", purchaseRecords)
	}
}

func TestExternalCurrencyListingValidation(t *testing.T) {
	svc := newService(t, memory.New())
	_, alice, _ := household(t, svc)

	// A well-formed one: $5 cash offered for 8 NanaCoin (spec 14).
	if _, err := svc.CreateListing(alice, ListingInput{
		Title: "$5 USD cash", Price: 8, Kind: "currency", Currency: "USD", MinorUnits: 500,
	}); err != nil {
		t.Fatalf("valid currency listing: %v", err)
	}

	for _, bad := range []ListingInput{
		{Title: "no code", Price: 8, Kind: "currency", MinorUnits: 500},
		{Title: "no units", Price: 8, Kind: "currency", Currency: "USD"},
		{Title: "unknown kind", Price: 8, Kind: "livestock"},
	} {
		if _, err := svc.CreateListing(alice, bad); !errors.Is(err, ErrBadInput) {
			t.Errorf("%q: got %v, want ErrBadInput", bad.Title, err)
		}
	}
}

func TestTextLimitsEnforced(t *testing.T) {
	svc := newService(t, memory.New())
	_, alice, bob := household(t, svc)

	long := make([]byte, MaxMemoLen+1)
	for i := range long {
		long[i] = 'x'
	}
	if _, err := svc.Transfer(alice, bob.Account, 1, string(long)); !errors.Is(err, ErrBadInput) {
		t.Errorf("oversized memo: got %v, want ErrBadInput", err)
	}
	// Control characters would travel into the journal and the UI.
	if _, err := svc.Transfer(alice, bob.Account, 1, "line\nbreak"); !errors.Is(err, ErrBadInput) {
		t.Errorf("control character in memo: got %v, want ErrBadInput", err)
	}
}

func TestIssuanceAccountIsNotSpendable(t *testing.T) {
	svc := newService(t, memory.New())
	_, alice, _ := household(t, svc)

	if _, err := svc.Transfer(alice, ledger.SystemIssuance, 10, ""); !errors.Is(err, ledger.ErrSystemAccount) {
		t.Errorf("got %v, want ErrSystemAccount", err)
	}
}

func TestCirculationTracksIssuanceAndRetirement(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, _ := household(t, svc)

	if got := svc.Status().Circulation; got != 200 {
		t.Fatalf("circulation is %d after two grants, want 200", got)
	}
	svc.Issue(nana, alice.Account, 50, "bonus")
	if got := svc.Status().Circulation; got != 250 {
		t.Errorf("circulation is %d after issuing 50, want 250", got)
	}
	svc.Retire(nana, alice.Account, 30, "fine")
	if got := svc.Status().Circulation; got != 220 {
		t.Errorf("circulation is %d after retiring 30, want 220", got)
	}

	// A transfer moves money without changing how much exists.
	svc.Transfer(alice, svc.Users()[2].Account, 10, "")
	if got := svc.Status().Circulation; got != 220 {
		t.Errorf("a transfer changed circulation to %d", got)
	}
}

// The idempotency cache must not grow without limit. It holds a full
// marshalled response per key, and an unbounded map of those is a slow leak
// that ends in "fatal error: out of memory" on a board with ~120 KB spare.
func TestIdempotencyCacheIsBounded(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, bob := household(t, svc)

	// Fund Alice enough to make many transfers.
	if _, err := svc.Issue(nana, alice.Account, 10_000, "float"); err != nil {
		t.Fatalf("Issue: %v", err)
	}

	// More distinct keys than the cache can hold.
	const n = MaxIdempotencyEntries + 40
	for i := 0; i < n; i++ {
		key := "key-" + fmt.Sprint(i)
		if _, err := svc.Idempotent(alice.ID, "transfer", key, func() ([]byte, error) {
			txn, err := svc.Transfer(alice, bob.Account, 1, "chore")
			if err != nil {
				return nil, err
			}
			return []byte(txn.ID), nil
		}); err != nil {
			t.Fatalf("transfer %d: %v", i, err)
		}
	}

	svc.mu.Lock()
	size := svc.idem.count
	order := svc.idem.count
	svc.mu.Unlock()

	if size > MaxIdempotencyEntries {
		t.Errorf("cache holds %d entries, want at most %d", size, MaxIdempotencyEntries)
	}
	if order != size {
		t.Errorf("order list has %d entries but the map has %d - they must not drift", order, size)
	}

	// The money still moved every time: eviction must not turn a completed
	// transfer into a lost one.
	if got := svc.Balance(bob.Account); got != 100+int64(n) {
		t.Errorf("bob has %d, want %d - some transfers did not happen", got, 100+n)
	}
}

// Eviction must keep the newest keys, which are the ones a client could still
// be retrying.
func TestIdempotencyEvictsOldestFirst(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, bob := household(t, svc)
	svc.Issue(nana, alice.Account, 10_000, "float")

	first := "key-first"
	if _, err := svc.Idempotent(alice.ID, "transfer", first, func() ([]byte, error) {
		txn, err := svc.Transfer(alice, bob.Account, 1, "first")
		if err != nil {
			return nil, err
		}
		return []byte(txn.ID), nil
	}); err != nil {
		t.Fatalf("first: %v", err)
	}

	// Fill past the limit so the first key is evicted.
	for i := 0; i < MaxIdempotencyEntries; i++ {
		key := "filler-" + fmt.Sprint(i)
		svc.Idempotent(alice.ID, "transfer", key, func() ([]byte, error) {
			txn, err := svc.Transfer(alice, bob.Account, 1, "filler")
			if err != nil {
				return nil, err
			}
			return []byte(txn.ID), nil
		})
	}

	svc.mu.Lock()
	_, _, stillThere := svc.idem.find(alice.ID, "transfer", first)
	newestEntry := &svc.idem.entries[svc.idem.count-1]
	newest := string(newestEntry.key[:newestEntry.keyLen])
	svc.mu.Unlock()

	if stillThere {
		t.Error("the oldest key survived eviction")
	}
	if !strings.Contains(newest, "filler-") {
		t.Errorf("newest retained key is %q, want the most recent filler", newest)
	}
}

// The cache must be bounded by bytes, not only by count. Each entry holds a
// full marshalled response - roughly 700 bytes for a purchase - so 64 entries
// could hold 44 kB, more than twice the board's free heap.
func TestIdempotencyCacheIsBoundedByBytes(t *testing.T) {
	svc := newService(t, memory.New())
	nana, alice, bob := household(t, svc)
	svc.Issue(nana, alice.Account, 100_000, "float")

	for i := 0; i < MaxIdempotencyEntries*4; i++ {
		key := "key-" + fmt.Sprint(i)
		if _, err := svc.Idempotent(alice.ID, "transfer", key, func() ([]byte, error) {
			txn, err := svc.Transfer(alice, bob.Account, 1, "a memo of some realistic length")
			if err != nil {
				return nil, err
			}
			return []byte(txn.ID), nil
		}); err != nil {
			t.Fatalf("transfer %d: %v", i, err)
		}
	}

	svc.mu.Lock()
	bytes := svc.idem.used
	entries := svc.idem.count
	svc.mu.Unlock()

	if bytes > MaxIdempotencyBytes {
		t.Errorf("cache holds %d bytes, over the %d budget", bytes, MaxIdempotencyBytes)
	}
	if entries > MaxIdempotencyEntries {
		t.Errorf("cache holds %d entries, over the %d limit", entries, MaxIdempotencyEntries)
	}

	// The accounting must stay honest, or the budget drifts from reality.
	svc.mu.Lock()
	var actual int
	for i := 0; i < svc.idem.count; i++ {
		actual += int(svc.idem.entries[i].size)
	}
	svc.mu.Unlock()
	if actual != bytes {
		t.Errorf("tracked %d bytes but the entries total %d", bytes, actual)
	}
}
