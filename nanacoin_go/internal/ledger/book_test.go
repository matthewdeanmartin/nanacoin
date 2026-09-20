package ledger

import (
	"errors"
	"testing"
)

const (
	alice AccountID = "account:alice"
	bob   AccountID = "account:bob"
)

// Transactions no longer carry a caller-chosen ID: the book derives one from
// the record's position, so the IDs in these tests are the derived ones -
// txn-1, txn-2 and so on, in append order.
func transfer(from, to AccountID, amount Amount) *Transaction {
	return &Transaction{
		Kind:      KindTransfer,
		CreatedAt: 1000,
		Postings: []Posting{
			{Account: from, Amount: -amount},
			{Account: to, Amount: amount},
		},
	}
}

func issuance(to AccountID, amount Amount) *Transaction {
	return &Transaction{
		Kind:      KindIssue,
		CreatedAt: 1000,
		Postings: []Posting{
			{Account: SystemIssuance, Amount: -amount},
			{Account: to, Amount: amount},
		},
	}
}

func mustIssue(t *testing.T, b *Book, to AccountID, amount Amount) TransactionID {
	t.Helper()
	id, err := b.Append(issuance(to, amount), false)
	if err != nil {
		t.Fatalf("issue %d to %s: %v", amount, to, err)
	}
	return id
}

func TestBalanceIsAFoldOverPostings(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 100)

	if got := b.Balance(alice); got != 100 {
		t.Errorf("alice has %d, want 100", got)
	}
	// Issuance carries the negative of everything in circulation.
	if got := b.Balance(SystemIssuance); got != -100 {
		t.Errorf("issuance is %d, want -100", got)
	}
	if got := b.Circulation(); got != 100 {
		t.Errorf("circulation is %d, want 100", got)
	}

	if _, err := b.Append(transfer(alice, bob, 30), false); err != nil {
		t.Fatalf("transfer: %v", err)
	}

	if got := b.Balance(alice); got != 70 {
		t.Errorf("alice has %d, want 70", got)
	}
	if got := b.Balance(bob); got != 30 {
		t.Errorf("bob has %d, want 30", got)
	}
	// A transfer moves money; it does not create or destroy it.
	if got := b.Circulation(); got != 100 {
		t.Errorf("circulation changed to %d after a transfer", got)
	}
	if err := b.CheckInvariants(); err != nil {
		t.Errorf("invariants: %v", err)
	}
}

// IDs are derived from position, so they are sequential and predictable.
func TestAppendReturnsDerivedIDs(t *testing.T) {
	b := NewBook()

	first := mustIssue(t, b, alice, 100)
	if first != "txn-1" {
		t.Errorf("first record got ID %q, want txn-1", first)
	}

	second, err := b.Append(transfer(alice, bob, 5), false)
	if err != nil {
		t.Fatalf("transfer: %v", err)
	}
	if second != "txn-2" {
		t.Errorf("second record got ID %q, want txn-2", second)
	}

	// And NextID predicts the next one, which is what lets the service
	// journal a record before applying it.
	if got := b.NextID(); got != "txn-3" {
		t.Errorf("NextID is %q, want txn-3", got)
	}
}

func TestUnbalancedTransactionRejected(t *testing.T) {
	b := NewBook()
	_, err := b.Append(&Transaction{
		Kind:      KindTransfer,
		CreatedAt: 1000,
		Postings: []Posting{
			{Account: alice, Amount: -5},
			{Account: bob, Amount: 7}, // money from nowhere
		},
	}, false)

	if !errors.Is(err, ErrUnbalanced) {
		t.Errorf("got %v, want ErrUnbalanced", err)
	}
	if b.Len() != 0 {
		t.Error("rejected transaction was appended anyway")
	}
}

func TestOverdraftRejectedForNormalAccounts(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 10)

	_, err := b.Append(transfer(alice, bob, 25), false)

	var insuf *InsufficientFundsError
	if !errors.As(err, &insuf) {
		t.Fatalf("got %v, want InsufficientFundsError", err)
	}
	if insuf.Account != alice || insuf.Balance != 10 || insuf.Requested != 25 {
		t.Errorf("error says account=%s balance=%d requested=%d",
			insuf.Account, insuf.Balance, insuf.Requested)
	}
	if !errors.Is(err, ErrInsufficient) {
		t.Error("InsufficientFundsError does not unwrap to ErrInsufficient")
	}
	if b.Balance(alice) != 10 {
		t.Error("rejected transfer changed a balance")
	}
}

// Overdraft is permitted when Nana is making a correction, and the resulting
// negative balance is left visible rather than hidden (spec 11).
func TestOverdraftAllowedWithOverride(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 10)

	if _, err := b.Append(&Transaction{
		Kind:      KindReversal,
		CreatedAt: 2000,
		Postings: []Posting{
			{Account: alice, Amount: -25},
			{Account: bob, Amount: 25},
		},
	}, true); err != nil {
		t.Fatalf("override append: %v", err)
	}

	if got := b.Balance(alice); got != -15 {
		t.Errorf("alice has %d, want -15", got)
	}
	if err := b.CheckInvariants(); err != nil {
		t.Errorf("a negative balance should not break invariants: %v", err)
	}
}

// The issuance account is exempt from the overdraft rule by construction: that
// exemption is what lets money be created at all.
func TestIssuanceAccountMayGoNegative(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 1000)
	if got := b.Balance(SystemIssuance); got != -1000 {
		t.Errorf("issuance is %d, want -1000", got)
	}
}

func TestZeroAmountPostingRejected(t *testing.T) {
	b := NewBook()
	_, err := b.Append(&Transaction{
		Kind:      KindTransfer,
		CreatedAt: 1000,
		Postings: []Posting{
			{Account: alice, Amount: 0},
			{Account: bob, Amount: 0},
		},
	}, false)
	if !errors.Is(err, ErrNonPositive) {
		t.Errorf("got %v, want ErrNonPositive", err)
	}
}

func TestEmptyTransactionRejected(t *testing.T) {
	_, err := NewBook().Append(&Transaction{Kind: KindTransfer, CreatedAt: 1000}, false)
	if !errors.Is(err, ErrEmpty) {
		t.Errorf("got %v, want ErrEmpty", err)
	}
}

// More postings than the packed record holds inline must be refused rather
// than truncated: a record with a posting missing would not sum to zero, and
// the whole model rests on that.
func TestTooManyPostingsRejected(t *testing.T) {
	b := NewBook()
	_, err := b.Append(&Transaction{
		Kind:      KindTransfer,
		CreatedAt: 1000,
		Postings: []Posting{
			{Account: alice, Amount: -10},
			{Account: bob, Amount: 5},
			{Account: "account:carol", Amount: 5},
		},
	}, false)
	if !errors.Is(err, ErrTooManyPostings) {
		t.Errorf("got %v, want ErrTooManyPostings", err)
	}
	if b.Len() != 0 {
		t.Error("a record that could not be packed was appended anyway")
	}
}

func TestReversalMirrorsOriginal(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 100)

	origID, err := b.Append(transfer(alice, bob, 10), false)
	if err != nil {
		t.Fatalf("transfer: %v", err)
	}

	rev, err := b.BuildReversal(origID, "user:nana", "Bob did not mow the lawn", 2000)
	if err != nil {
		t.Fatalf("BuildReversal: %v", err)
	}
	revID, err := b.Append(rev, true)
	if err != nil {
		t.Fatalf("append reversal: %v", err)
	}

	if got := b.Balance(alice); got != 100 {
		t.Errorf("alice has %d, want 100 (back where she started)", got)
	}
	if got := b.Balance(bob); got != 0 {
		t.Errorf("bob has %d, want 0", got)
	}
	if rev.Reverses != origID {
		t.Errorf("reversal points at %q, want %q", rev.Reverses, origID)
	}

	// The original is still there, unedited. That is the audit trail.
	if b.Len() != 3 {
		t.Errorf("book has %d transactions, want 3 - nothing should be deleted", b.Len())
	}
	got, ok := b.Get(origID)
	if !ok {
		t.Fatalf("the original %q is gone", origID)
	}
	if got.Postings[0].Amount != -10 {
		t.Error("the original transaction was mutated")
	}
	if id, ok := b.ReversalOf(origID); !ok || id != revID {
		t.Errorf("reversal index says %q/%v, want %q/true", id, ok, revID)
	}
}

func TestDoubleReversalRejected(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 100)

	origID, _ := b.Append(transfer(alice, bob, 10), false)
	rev, _ := b.BuildReversal(origID, "user:nana", "mistake", 2000)
	revID, _ := b.Append(rev, true)

	if _, err := b.BuildReversal(origID, "user:nana", "again", 3000); !errors.Is(err, ErrAlreadyReversed) {
		t.Errorf("got %v, want ErrAlreadyReversed", err)
	}
	// And a reversal is not itself reversible - undoing an undo is better
	// expressed as a fresh transfer.
	if _, err := b.BuildReversal(revID, "user:nana", "undo the undo", 4000); !errors.Is(err, ErrReverseReversal) {
		t.Errorf("got %v, want ErrReverseReversal", err)
	}
}

func TestReverseUnknownTransaction(t *testing.T) {
	if _, err := NewBook().BuildReversal("txn-999", "user:nana", "", 0); !errors.Is(err, ErrNotFound) {
		t.Errorf("got %v, want ErrNotFound", err)
	}
	// A malformed ID is not found either, rather than panicking on the parse.
	if _, err := NewBook().BuildReversal("not-an-id", "user:nana", "", 0); !errors.Is(err, ErrNotFound) {
		t.Errorf("got %v, want ErrNotFound", err)
	}
}

func TestHistoryIsNewestFirstAndFiltered(t *testing.T) {
	b := NewBook()
	aliceGrant := mustIssue(t, b, alice, 100)
	mustIssue(t, b, bob, 100) // not alice's
	transferID, _ := b.Append(transfer(alice, bob, 5), false)

	h := b.History(alice, 0)
	if len(h) != 2 {
		t.Fatalf("alice has %d transactions, want 2 (bob's grant is not hers)", len(h))
	}
	if h[0].ID != transferID || h[1].ID != aliceGrant {
		t.Errorf("history order is %s, %s; want %s, %s",
			h[0].ID, h[1].ID, transferID, aliceGrant)
	}

	if got := b.History(alice, 1); len(got) != 1 || got[0].ID != transferID {
		t.Errorf("limited history is wrong: %v", got)
	}
}

// A transaction that both debits and credits an account is judged on its net
// effect, not on the intermediate state of each posting.
//
// With two inline posting slots the only way to express this is two postings
// against the same account, which must balance to zero between them - so the
// case this covers is narrower than it was when postings were a slice. That
// is a real consequence of packing: a three-posting transaction, netting or
// otherwise, is now refused rather than stored. See TestTooManyPostingsRejected.
func TestNettingWithinATransaction(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 10)

	// Both postings against alice, netting zero. Validate must judge the net
	// rather than refusing the -100 leg against a balance of 10.
	if _, err := b.Append(&Transaction{
		Kind:      KindTransfer,
		CreatedAt: 1000,
		Postings: []Posting{
			{Account: alice, Amount: -100},
			{Account: alice, Amount: 100},
		},
	}, false); err != nil {
		t.Fatalf("a net-zero pair against a balance of 10 should be fine: %v", err)
	}
	if got := b.Balance(alice); got != 10 {
		t.Errorf("alice has %d, want 10 - the pair nets to nothing", got)
	}
}

func TestCheckInvariantsDetectsCacheDrift(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 100)
	if err := b.CheckInvariants(); err != nil {
		t.Fatalf("clean book: %v", err)
	}

	// Corrupt the cache the way a bug that updated a balance without a
	// posting would.
	b.balanceSlot(b.strs.Intern(string(alice))).balance = 999
	if err := b.CheckInvariants(); err == nil {
		t.Error("CheckInvariants missed a drifted cached balance")
	}
}

// Replay must not re-validate: a reversal that legitimately leaves an account
// negative has to survive a reboot.
func TestReplayAcceptsNegativeBalances(t *testing.T) {
	b := NewBook()

	if _, err := b.Replay(issuance(alice, 10)); err != nil {
		t.Fatalf("replaying issuance: %v", err)
	}
	if _, err := b.Replay(&Transaction{
		Kind:      KindReversal,
		CreatedAt: 2000,
		Postings: []Posting{
			{Account: alice, Amount: -25},
			{Account: bob, Amount: 25},
		},
	}); err != nil {
		t.Fatalf("replaying reversal: %v", err)
	}

	if got := b.Balance(alice); got != -15 {
		t.Errorf("alice has %d, want -15", got)
	}
	if err := b.CheckInvariants(); err != nil {
		t.Errorf("invariants after replay: %v", err)
	}
}

// An unknown or malformed ID must read as absent rather than panicking or
// returning someone else's record.
func TestGetRejectsBadIDs(t *testing.T) {
	b := NewBook()
	mustIssue(t, b, alice, 100)

	for _, id := range []TransactionID{"", "txn-0", "txn-2", "txn-99", "nope", "txn-abc"} {
		if _, ok := b.Get(id); ok {
			t.Errorf("Get(%q) returned a record", id)
		}
	}
	if _, ok := b.Get("txn-1"); !ok {
		t.Error("Get(txn-1) did not find the record that exists")
	}
}

// Currencies must balance separately, or the book can be broken in a way that
// looks fine.
//
// The failure this guards: 100 coins leave an account and 100 cents arrive in
// another. A single grand total is zero, so a combined check calls that
// balanced - and the household has lost 100 coins and gained a dollar from
// nowhere. "Every coin is accounted for" and "every dollar is accounted for"
// have to be two statements.
func TestCrossCurrencyCannotCancel(t *testing.T) {
	txn := &Transaction{
		ID:        TransactionIDFor(1),
		Kind:      KindTransfer,
		CreatedAt: 1,
		Actor:     "user-1",
		Postings: []Posting{
			{Account: "account-a", Amount: -100, Currency: NANA},
			{Account: "account-b", Amount: 100, Currency: USD},
		},
	}

	// The naive combined total is zero...
	var combined Amount
	for _, p := range txn.Postings {
		combined += p.Amount
	}
	if combined != 0 {
		t.Fatalf("test setup is wrong: combined total is %d, wanted a deceptive zero", combined)
	}

	// ...but neither currency balances on its own.
	if txn.Sum(NANA) == 0 {
		t.Error("NanaCoin side sums to zero; the currencies were allowed to cancel")
	}
	if txn.Sum(USD) == 0 {
		t.Error("USD side sums to zero; the currencies were allowed to cancel")
	}

	b := NewBook()
	if err := b.Validate(txn, false); err == nil {
		t.Fatal("Validate accepted a transaction whose currencies cancel each other")
	}
}

// A single-currency transaction still balances, and a USD one balances in USD.
func TestSingleCurrencyTransactionsBalance(t *testing.T) {
	for _, c := range []Currency{NANA, USD} {
		txn := &Transaction{
			ID:        TransactionIDFor(1),
			Kind:      KindTransfer,
			CreatedAt: 1,
			Actor:     "user-1",
			Postings: []Posting{
				{Account: "account-a", Amount: -50, Currency: c},
				{Account: "account-b", Amount: 50, Currency: c},
			},
		}
		if got := txn.Sum(c); got != 0 {
			t.Errorf("%s transaction sums to %d, want 0", c, got)
		}
		// And contributes nothing to the other currency's total.
		other := NANA
		if c == NANA {
			other = USD
		}
		if got := txn.Sum(other); got != 0 {
			t.Errorf("%s transaction contributed %d to %s", c, got, other)
		}
	}
}

// Currency survives the pack/unpack round trip, which is what replay depends
// on: a USD posting that comes back as NanaCoin would silently rewrite the
// household's money after a reboot.
func TestCurrencySurvivesPacking(t *testing.T) {
	strs, arena := NewStrings(), NewArena()
	original := &Transaction{
		ID:        TransactionIDFor(1),
		Kind:      KindTransfer,
		CreatedAt: 1,
		Actor:     "user-1",
		Postings: []Posting{
			{Account: "account-a", Amount: -500, Currency: USD},
			{Account: "account-b", Amount: 500, Currency: USD},
		},
	}

	packed, ok := Pack(original, strs, arena, 1)
	if !ok {
		t.Fatal("Pack refused a two-posting USD transaction")
	}
	back := Unpack(&packed, strs, arena)

	if len(back.Postings) != 2 {
		t.Fatalf("got %d postings back, want 2", len(back.Postings))
	}
	for i, p := range back.Postings {
		if p.Currency != USD {
			t.Errorf("posting %d came back as %s, want USD", i, p.Currency)
		}
	}
}
