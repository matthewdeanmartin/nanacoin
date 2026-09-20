package auth

import (
	"testing"
	"time"
)

// A store with a deliberately tiny session table, so "full" is reachable in a
// test the way it is reachable on a board with 32 slots and an evening's use.
func newTestStore(t *testing.T, slots int) *Store {
	t.Helper()
	now := time.Unix(1_700_000_000, 0)
	return NewStore(Options{
		Now:             func() time.Time { return now },
		SessionCapacity: slots,
	})
}

// Logging in repeatedly must not fill the table with sessions nobody holds.
//
// This is the bug behind a demo seed run stopping a few minutes in with
// "invalid or expired token". Sessions are RAM-only, last eight hours, and
// nothing logs out; the seeder signs in as every household member on each
// run. Once the table was full the board refused every further login for the
// rest of the day - and the user never saw that refusal, because the client
// had already dropped the account it could not re-authenticate and the *next*
// request failed with a dead token instead.
func TestRepeatedLoginRecyclesYourOwnSession(t *testing.T) {
	store := newTestStore(t, 4)

	// Far more logins than there are slots. Every one must succeed.
	var newest string
	for i := 0; i < 40; i++ {
		token, _, err := store.NewSession("user:alice")
		if err != nil {
			t.Fatalf("login %d refused: %v", i+1, err)
		}
		newest = token
	}

	// The newest token works, which is the thing the person is holding.
	if _, err := store.Lookup(newest); err != nil {
		t.Fatalf("the newest session does not work: %v", err)
	}

	// Every one of Alice's 40 logins worked, and the table never grew past
	// its four slots - which is the whole point. What this does *not* claim
	// is that other people are unaffected: Alice's first four logins do fill
	// a four-slot table, and a fifth person then has no stale session of
	// their own to reclaim. That limit is real and is asserted separately in
	// TestAFullTableOfOtherPeopleStillRefuses.
	//
	// The bug being fixed is unbounded accumulation across a long run, not
	// contention between users on a table sized for the household.
	var used int
	for i := range store.sessions {
		if store.sessions[i].used {
			used++
		}
	}
	if used > 4 {
		t.Errorf("40 logins left %d sessions in a 4-slot table", used)
	}
}

// The flip side: eviction takes the caller's own stalest session, never
// somebody else's live one. A parent's session must not be ended because a
// child logged in.
func TestLoginNeverEvictsAnotherUser(t *testing.T) {
	store := newTestStore(t, 3)

	nana, _, err := store.NewSession("user:nana")
	if err != nil {
		t.Fatalf("nana: %v", err)
	}
	// Fill what is left, then keep going as one user.
	if _, _, err := store.NewSession("user:kid"); err != nil {
		t.Fatalf("kid: %v", err)
	}
	for i := 0; i < 10; i++ {
		if _, _, err := store.NewSession("user:kid"); err != nil {
			t.Fatalf("kid login %d: %v", i+1, err)
		}
	}

	if _, err := store.Lookup(nana); err != nil {
		t.Errorf("nana's session was evicted by another user's logins: %v", err)
	}
}

// With every slot held by a different user there is no stale session of the
// caller's to reclaim, and refusing is the honest answer.
func TestAFullTableOfOtherPeopleStillRefuses(t *testing.T) {
	store := newTestStore(t, 2)
	if _, _, err := store.NewSession("user:a"); err != nil {
		t.Fatal(err)
	}
	if _, _, err := store.NewSession("user:b"); err != nil {
		t.Fatal(err)
	}
	if _, _, err := store.NewSession("user:c"); err != ErrTooManySessions {
		t.Errorf("got %v, want ErrTooManySessions", err)
	}
}
