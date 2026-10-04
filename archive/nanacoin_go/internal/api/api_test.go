package api

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// --- harness ----------------------------------------------------------------

type harness struct {
	t        testing.TB
	srv      http.Handler
	svc      *core.Service
	sessions *auth.Store
	seq      int
}

func newHarness(t testing.TB) *harness {
	t.Helper()
	h := &harness{t: t}

	// Deterministic IDs make failures readable: a test that fails says
	// "txn-3", not a random string.
	svc, err := core.New(memory.New(), core.Options{
		NewID: func(prefix string) string {
			h.seq++
			return fmt.Sprintf("%s-%d", prefix, h.seq)
		},
	})
	if err != nil {
		t.Fatalf("core.New: %v", err)
	}
	h.svc = svc
	h.sessions = auth.NewStore(auth.Options{TokenTTL: time.Hour})
	h.srv = NewServer(svc, h.sessions, Config{
		AllowedOrigins: []string{"https://nanacoin.example.net"},
		AllowProvision: true,
	}).Handler()
	return h
}

func (h *harness) do(method, path, token string, body any, headers ...string) *httptest.ResponseRecorder {
	h.t.Helper()
	var r *http.Request
	if body != nil {
		b, err := json.Marshal(body)
		if err != nil {
			h.t.Fatalf("marshal request: %v", err)
		}
		r = httptest.NewRequest(method, path, bytes.NewReader(b))
		r.Header.Set("Content-Type", "application/json")
	} else {
		r = httptest.NewRequest(method, path, nil)
	}
	if token != "" {
		r.Header.Set("Authorization", "Bearer "+token)
	}
	for i := 0; i+1 < len(headers); i += 2 {
		r.Header.Set(headers[i], headers[i+1])
	}
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)
	return w
}

// decodeInto unmarshals a response body, failing the test on an unexpected
// status so that a broken call reports the server's message rather than a
// confusing unmarshal error.
func (h *harness) decodeInto(w *httptest.ResponseRecorder, wantStatus int, v any) {
	h.t.Helper()
	if w.Code != wantStatus {
		h.t.Fatalf("status %d, want %d: %s", w.Code, wantStatus, w.Body.String())
	}
	if v != nil {
		if err := json.Unmarshal(w.Body.Bytes(), v); err != nil {
			h.t.Fatalf("decoding response: %v (body: %s)", err, w.Body.String())
		}
	}
}

// login runs the whole PKCE flow and returns the access token.
func (h *harness) login(username, password string) string {
	h.t.Helper()
	verifier := strings.Repeat("a", 43)
	sum := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(sum[:])

	var authz authorizeResponse
	w := h.do("POST", "/api/v1/auth/authorize", "", authorizeRequest{
		Username: username, Password: password,
		CodeChallenge: challenge, CodeChallengeMethod: "S256",
		RedirectURI: "http://localhost:5173/callback",
	})
	h.decodeInto(w, http.StatusOK, &authz)

	var tok tokenResponse
	w = h.do("POST", "/api/v1/auth/token", "", tokenRequest{
		Code: authz.Code, CodeVerifier: verifier,
		RedirectURI: "http://localhost:5173/callback",
	})
	h.decodeInto(w, http.StatusOK, &tok)
	return tok.AccessToken
}

// setup provisions Nana and creates two users, returning their tokens.
func (h *harness) setup() (nana, alice, bob string, aliceAcct, bobAcct string) {
	h.t.Helper()

	w := h.do("POST", "/api/v1/provision", "", provisionRequest{
		Username: "nana", DisplayName: "Nana", Password: "nana-pin", HouseholdName: "The House",
	})
	h.decodeInto(w, http.StatusCreated, nil)
	nana = h.login("nana", "nana-pin")

	var au, bu userView
	w = h.do("POST", "/api/v1/users", nana, createUserRequest{
		Username: "alice", DisplayName: "Alice", Password: "alice-pin",
	})
	h.decodeInto(w, http.StatusCreated, &au)
	w = h.do("POST", "/api/v1/users", nana, createUserRequest{
		Username: "bob", DisplayName: "Bob", Password: "bob-pin",
	})
	h.decodeInto(w, http.StatusCreated, &bu)

	return nana, h.login("alice", "alice-pin"), h.login("bob", "bob-pin"),
		string(au.Account), string(bu.Account)
}

func (h *harness) balance(token string) int64 {
	h.t.Helper()
	var me userView
	h.decodeInto(h.do("GET", "/api/v1/me", token, nil), http.StatusOK, &me)
	if me.Balance == nil {
		h.t.Fatal("/me returned no balance")
	}
	return *me.Balance
}

// --- tests ------------------------------------------------------------------

func TestStatusIsPublicAndReportsUnprovisioned(t *testing.T) {
	h := newHarness(t)
	var st core.Status
	h.decodeInto(h.do("GET", "/api/v1/status", "", nil), http.StatusOK, &st)
	if st.Provisioned {
		t.Error("a fresh server reports itself provisioned")
	}
	if st.TransactionCapacity != ledger.Capacity || st.RetainedTransactions != 0 || st.OldestTransaction != 0 {
		t.Fatalf("incorrect initial ledger window: %+v", st)
	}
}

func TestProvisionHappensOnce(t *testing.T) {
	h := newHarness(t)
	w := h.do("POST", "/api/v1/provision", "", provisionRequest{
		Username: "nana", Password: "nana-pin", HouseholdName: "The House",
	})
	h.decodeInto(w, http.StatusCreated, nil)

	// A second provisioning would hand anyone on the network a Nana account.
	w = h.do("POST", "/api/v1/provision", "", provisionRequest{
		Username: "intruder", Password: "hunter22",
	})
	if w.Code != http.StatusForbidden {
		t.Errorf("second provision returned %d, want 403: %s", w.Code, w.Body.String())
	}
}

func TestInitialGrantIsALedgerTransaction(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	if got := h.balance(alice); got != 100 {
		t.Errorf("alice starts with %d, want the configured 100", got)
	}

	// The grant must exist as a transaction, not as an initialised field.
	var hist struct {
		Transactions []transactionView `json:"transactions"`
	}
	var me userView
	h.decodeInto(h.do("GET", "/api/v1/me", alice, nil), http.StatusOK, &me)
	h.decodeInto(h.do("GET", "/api/v1/accounts/"+string(me.Account)+"/transactions", alice, nil), http.StatusOK, &hist)

	if len(hist.Transactions) != 1 {
		t.Fatalf("alice has %d transactions, want 1", len(hist.Transactions))
	}
	tx := hist.Transactions[0]
	if tx.Kind != "ISSUE" {
		t.Errorf("grant is kind %s, want ISSUE", tx.Kind)
	}
	if len(tx.Postings) != 2 {
		t.Fatalf("grant has %d postings, want 2", len(tx.Postings))
	}
	var sum int64
	for _, p := range tx.Postings {
		sum += p.Amount
	}
	if sum != 0 {
		t.Errorf("grant postings sum to %d, want 0", sum)
	}
}

func TestTransferMovesMoney(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, _, bobAcct := h.setup()

	w := h.do("POST", "/api/v1/transfers", alice, transferRequest{
		To: acct(bobAcct), Amount: 30, Memo: "Taking out trash",
	})
	h.decodeInto(w, http.StatusCreated, nil)

	if got := h.balance(alice); got != 70 {
		t.Errorf("alice has %d, want 70", got)
	}
	if got := h.balance(bob); got != 130 {
		t.Errorf("bob has %d, want 130", got)
	}
}

func TestTransferRejectsOverdraft(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, bobAcct := h.setup()

	w := h.do("POST", "/api/v1/transfers", alice, transferRequest{
		To: acct(bobAcct), Amount: 500,
	})
	if w.Code != http.StatusConflict {
		t.Fatalf("status %d, want 409: %s", w.Code, w.Body.String())
	}
	var e errorBody
	json.Unmarshal(w.Body.Bytes(), &e)
	if e.Error != "insufficient_funds" {
		t.Errorf("error code %q, want insufficient_funds", e.Error)
	}
	if got := h.balance(alice); got != 100 {
		t.Errorf("a refused transfer changed the balance to %d", got)
	}
}

func TestTransferRejectsSelfAndNonPositive(t *testing.T) {
	h := newHarness(t)
	_, alice, _, aliceAcct, bobAcct := h.setup()

	if w := h.do("POST", "/api/v1/transfers", alice, transferRequest{
		To: acct(aliceAcct), Amount: 5,
	}); w.Code != http.StatusBadRequest {
		t.Errorf("self-transfer returned %d, want 400", w.Code)
	}
	for _, amount := range []int64{0, -5} {
		if w := h.do("POST", "/api/v1/transfers", alice, transferRequest{
			To: acct(bobAcct), Amount: amount,
		}); w.Code != http.StatusBadRequest {
			t.Errorf("transfer of %d returned %d, want 400", amount, w.Code)
		}
	}
	// A negative transfer that succeeded would be a way to take money.
	if got := h.balance(alice); got != 100 {
		t.Errorf("alice has %d after refused transfers, want 100", got)
	}
}

// The spending account comes from the token, never from the request, so a
// client cannot name someone else's account as the source.
func TestTransferCannotSpendAnotherAccount(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, aliceAcct, _ := h.setup()

	// Bob transfers "to" Alice - the only account he can name. There is no
	// field through which he could name Alice's as the source, which is the
	// property under test: his own balance is what moves.
	w := h.do("POST", "/api/v1/transfers", bob, transferRequest{
		To: acct(aliceAcct), Amount: 10,
	})
	h.decodeInto(w, http.StatusCreated, nil)

	if got := h.balance(bob); got != 90 {
		t.Errorf("bob has %d, want 90 - his own account funded the transfer", got)
	}
	if got := h.balance(alice); got != 110 {
		t.Errorf("alice has %d, want 110", got)
	}
}

func TestIdempotentTransferMovesMoneyOnce(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, _, bobAcct := h.setup()

	req := transferRequest{To: acct(bobAcct), Amount: 25, Memo: "chores"}
	var first, second transactionView

	w := h.do("POST", "/api/v1/transfers", alice, req, "Idempotency-Key", "key-abc")
	h.decodeInto(w, http.StatusCreated, &first)

	// The retry a flaky wifi link produces: same key, same request.
	w = h.do("POST", "/api/v1/transfers", alice, req, "Idempotency-Key", "key-abc")
	h.decodeInto(w, http.StatusCreated, &second)

	if first.ID != second.ID {
		t.Errorf("retry produced a new transaction %s, want the original %s", second.ID, first.ID)
	}
	if got := h.balance(alice); got != 75 {
		t.Errorf("alice has %d, want 75 - the money moved twice", got)
	}
	if got := h.balance(bob); got != 125 {
		t.Errorf("bob has %d, want 125", got)
	}

	// A different key is a different operation and must go through.
	w = h.do("POST", "/api/v1/transfers", alice, req, "Idempotency-Key", "key-def")
	h.decodeInto(w, http.StatusCreated, nil)
	if got := h.balance(alice); got != 50 {
		t.Errorf("alice has %d, want 50", got)
	}
}

// One user's idempotency key must not satisfy another user's request.
func TestIdempotencyKeysAreScopedPerUser(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, aliceAcct, bobAcct := h.setup()

	h.decodeInto(h.do("POST", "/api/v1/transfers", alice,
		transferRequest{To: acct(bobAcct), Amount: 10}, "Idempotency-Key", "shared"),
		http.StatusCreated, nil)

	// Bob reuses the same key for his own, different transfer.
	h.decodeInto(h.do("POST", "/api/v1/transfers", bob,
		transferRequest{To: acct(aliceAcct), Amount: 5}, "Idempotency-Key", "shared"),
		http.StatusCreated, nil)

	if got := h.balance(bob); got != 105 {
		t.Errorf("bob has %d, want 105 (110 received - 5 sent); his transfer was swallowed by alice's key", got)
	}
}

func TestReversalIsNanaOnlyAndAppendsAnOpposite(t *testing.T) {
	h := newHarness(t)
	nana, alice, bob, _, bobAcct := h.setup()

	var txn transactionView
	h.decodeInto(h.do("POST", "/api/v1/transfers", alice,
		transferRequest{To: acct(bobAcct), Amount: 10, Memo: "mowing"}),
		http.StatusCreated, &txn)

	// An ordinary user cannot undo their own payment.
	if w := h.do("POST", "/api/v1/transactions/"+string(txn.ID)+"/reverse", alice,
		reverseRequest{Reason: "changed my mind"}); w.Code != http.StatusForbidden {
		t.Errorf("alice reversing returned %d, want 403", w.Code)
	}

	var rev transactionView
	h.decodeInto(h.do("POST", "/api/v1/transactions/"+string(txn.ID)+"/reverse", nana,
		reverseRequest{Reason: "Bob did not mow the lawn"}),
		http.StatusCreated, &rev)

	if rev.Reverses != txn.ID {
		t.Errorf("reversal points at %q, want %q", rev.Reverses, txn.ID)
	}
	if got := h.balance(alice); got != 100 {
		t.Errorf("alice has %d, want 100", got)
	}
	if got := h.balance(bob); got != 100 {
		t.Errorf("bob has %d, want 100", got)
	}

	// The original is still readable and now points at its reversal.
	var orig transactionView
	h.decodeInto(h.do("GET", "/api/v1/transactions/"+string(txn.ID), nana, nil), http.StatusOK, &orig)
	if orig.ReversedBy != rev.ID {
		t.Errorf("original says reversed_by=%q, want %q", orig.ReversedBy, rev.ID)
	}
	if orig.Description != "mowing" {
		t.Error("the original transaction was rewritten")
	}

	// Reversing twice would double the correction.
	if w := h.do("POST", "/api/v1/transactions/"+string(txn.ID)+"/reverse", nana,
		reverseRequest{Reason: "again"}); w.Code != http.StatusConflict {
		t.Errorf("double reversal returned %d, want 409", w.Code)
	}
}

// Nana may reverse into a negative balance, and the negative is left visible
// rather than silently blocked (spec 11).
func TestReversalMayOverdraw(t *testing.T) {
	h := newHarness(t)
	nana, alice, bob, aliceAcct, bobAcct := h.setup()

	var txn transactionView
	h.decodeInto(h.do("POST", "/api/v1/transfers", alice,
		transferRequest{To: acct(bobAcct), Amount: 100, Memo: "everything"}),
		http.StatusCreated, &txn)

	// Bob spends it all before the reversal lands.
	h.decodeInto(h.do("POST", "/api/v1/transfers", bob,
		transferRequest{To: acct(aliceAcct), Amount: 200}),
		http.StatusCreated, nil)

	h.decodeInto(h.do("POST", "/api/v1/transactions/"+string(txn.ID)+"/reverse", nana,
		reverseRequest{Reason: "not actually delivered"}),
		http.StatusCreated, nil)

	if got := h.balance(bob); got != -100 {
		t.Errorf("bob has %d, want -100 - the correction must land and be visible", got)
	}
}

func TestPurchaseIsAtomic(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, _, _ := h.setup()

	var listing listingView
	h.decodeInto(h.do("POST", "/api/v1/listings", bob, createListingRequest{
		Title: "One hour of Switch time", Description: "Uninterrupted", Price: 40,
	}), http.StatusCreated, &listing)

	var resp purchaseResponse
	h.decodeInto(h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/purchase", alice, nil),
		http.StatusCreated, &resp)

	// Money and listing state changed together, in one response.
	if resp.Listing.Status != "SOLD" {
		t.Errorf("listing status %s, want SOLD", resp.Listing.Status)
	}
	if resp.Transaction.Reference != string(listing.ID) {
		t.Errorf("transaction references %q, want the listing %q", resp.Transaction.Reference, listing.ID)
	}
	if got := h.balance(alice); got != 60 {
		t.Errorf("alice has %d, want 60", got)
	}
	if got := h.balance(bob); got != 140 {
		t.Errorf("bob has %d, want 140", got)
	}

	// A second buyer finds it closed rather than buying it again.
	if w := h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/purchase", alice, nil); w.Code != http.StatusConflict {
		t.Errorf("repeat purchase returned %d, want 409", w.Code)
	}
}

// A purchase that cannot be paid for must leave the listing on sale. If the
// listing were marked sold first, this is where the money and the goods would
// come apart.
func TestFailedPurchaseLeavesListingActive(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, _, _ := h.setup()

	var listing listingView
	h.decodeInto(h.do("POST", "/api/v1/listings", bob, createListingRequest{
		Title: "Old LEGO set", Price: 500,
	}), http.StatusCreated, &listing)

	if w := h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/purchase", alice, nil); w.Code != http.StatusConflict {
		t.Fatalf("unaffordable purchase returned %d, want 409: %s", w.Code, w.Body.String())
	}

	var after listingView
	h.decodeInto(h.do("GET", "/api/v1/listings/"+string(listing.ID), alice, nil), http.StatusOK, &after)
	if after.Status != "ACTIVE" {
		t.Errorf("listing is %s after a failed purchase, want ACTIVE", after.Status)
	}
	if got := h.balance(alice); got != 100 {
		t.Errorf("alice has %d after a failed purchase, want 100", got)
	}
	if got := h.balance(bob); got != 100 {
		t.Errorf("bob has %d after a failed purchase, want 100", got)
	}
}

func TestCannotBuyOwnListing(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	var listing listingView
	h.decodeInto(h.do("POST", "/api/v1/listings", alice, createListingRequest{
		Title: "Do your dishes", Price: 5,
	}), http.StatusCreated, &listing)

	if w := h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/purchase", alice, nil); w.Code != http.StatusBadRequest {
		t.Errorf("self-purchase returned %d, want 400", w.Code)
	}
}

func TestCancelledListingCannotBeBought(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, _, _ := h.setup()

	var listing listingView
	h.decodeInto(h.do("POST", "/api/v1/listings", bob, createListingRequest{
		Title: "Gone", Price: 5,
	}), http.StatusCreated, &listing)
	h.decodeInto(h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/cancel", bob, nil), http.StatusOK, nil)

	if w := h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/purchase", alice, nil); w.Code != http.StatusConflict {
		t.Errorf("buying a cancelled listing returned %d, want 409", w.Code)
	}
}

func TestListingEditsAreSellerOrNanaOnly(t *testing.T) {
	h := newHarness(t)
	nana, alice, bob, _, _ := h.setup()

	var listing listingView
	h.decodeInto(h.do("POST", "/api/v1/listings", bob, createListingRequest{
		Title: "Bob's thing", Price: 5,
	}), http.StatusCreated, &listing)

	newTitle := "Alice's thing now"
	if w := h.do("PATCH", "/api/v1/listings/"+string(listing.ID), alice,
		updateListingRequest{Title: &newTitle}); w.Code != http.StatusForbidden {
		t.Errorf("alice editing bob's listing returned %d, want 403", w.Code)
	}
	if w := h.do("POST", "/api/v1/listings/"+string(listing.ID)+"/cancel", alice, nil); w.Code != http.StatusForbidden {
		t.Errorf("alice cancelling bob's listing returned %d, want 403", w.Code)
	}
	// Nana may (listing:edit_all, listing:cancel_all).
	nanaTitle := "Moderated"
	h.decodeInto(h.do("PATCH", "/api/v1/listings/"+string(listing.ID), nana,
		updateListingRequest{Title: &nanaTitle}), http.StatusOK, nil)
}

// --- authorization ----------------------------------------------------------

func TestIssuanceIsNanaOnly(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, aliceAcct, _ := h.setup()

	if w := h.do("POST", "/api/v1/admin/issue", alice, issueRequest{
		To: acct(aliceAcct), Amount: 1000, Reason: "treating myself",
	}); w.Code != http.StatusForbidden {
		t.Errorf("alice issuing returned %d, want 403", w.Code)
	}
	if got := h.balance(alice); got != 100 {
		t.Errorf("alice printed herself %d NanaCoin", got-100)
	}

	h.decodeInto(h.do("POST", "/api/v1/admin/issue", nana, issueRequest{
		To: acct(aliceAcct), Amount: 50, Reason: "birthday",
	}), http.StatusCreated, nil)
	if got := h.balance(alice); got != 150 {
		t.Errorf("alice has %d, want 150", got)
	}
}

func TestUserCreationIsNanaOnly(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	if w := h.do("POST", "/api/v1/users", alice, createUserRequest{
		Username: "sockpuppet", Password: "pin1234",
	}); w.Code != http.StatusForbidden {
		t.Errorf("alice creating a user returned %d, want 403", w.Code)
	}
}

// Self-promotion to Nana is the escalation that matters most.
func TestUserCannotPromoteSelf(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	var me userView
	h.decodeInto(h.do("GET", "/api/v1/me", alice, nil), http.StatusOK, &me)

	role := roleOf("nana")
	if w := h.do("PATCH", "/api/v1/users/"+string(me.ID), alice,
		updateUserRequest{Role: &role}); w.Code != http.StatusForbidden {
		t.Errorf("self-promotion returned %d, want 403", w.Code)
	}

	var after userView
	h.decodeInto(h.do("GET", "/api/v1/me", alice, nil), http.StatusOK, &after)
	if after.Role != "user" {
		t.Errorf("alice is now %s", after.Role)
	}
}

func TestUserCannotEditAnotherUser(t *testing.T) {
	h := newHarness(t)
	_, alice, bob, _, _ := h.setup()

	var bobView userView
	h.decodeInto(h.do("GET", "/api/v1/me", bob, nil), http.StatusOK, &bobView)

	name := "Robert"
	if w := h.do("PATCH", "/api/v1/users/"+string(bobView.ID), alice,
		updateUserRequest{DisplayName: &name}); w.Code != http.StatusForbidden {
		t.Errorf("alice editing bob returned %d, want 403", w.Code)
	}
}

func TestBalancesAreNotVisibleToOtherUsers(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, bobAcct := h.setup()

	if w := h.do("GET", "/api/v1/accounts/"+bobAcct, alice, nil); w.Code != http.StatusForbidden {
		t.Errorf("alice reading bob's account returned %d, want 403", w.Code)
	}
	if w := h.do("GET", "/api/v1/accounts/"+bobAcct+"/transactions", alice, nil); w.Code != http.StatusForbidden {
		t.Errorf("alice reading bob's history returned %d, want 403", w.Code)
	}
	// Nana may (ledger:read_all).
	h.decodeInto(h.do("GET", "/api/v1/accounts/"+bobAcct, nana, nil), http.StatusOK, nil)

	// The household list shows members but hides other people's balances.
	var list struct {
		Users []userView `json:"users"`
	}
	h.decodeInto(h.do("GET", "/api/v1/users", alice, nil), http.StatusOK, &list)
	shown := 0
	for _, u := range list.Users {
		if u.Balance != nil {
			shown++
			if u.Username != "alice" {
				t.Errorf("alice can see %s's balance", u.Username)
			}
		}
	}
	if shown != 1 {
		t.Errorf("%d balances visible to alice, want 1 (her own)", shown)
	}
}

func TestFullLedgerIsNanaOnly(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, _ := h.setup()

	if w := h.do("GET", "/api/v1/transactions", alice, nil); w.Code != http.StatusForbidden {
		t.Errorf("alice reading the whole ledger returned %d, want 403", w.Code)
	}
	h.decodeInto(h.do("GET", "/api/v1/transactions", nana, nil), http.StatusOK, nil)
}

// A transaction ID must not be a way to browse other people's dealings.
func TestTransactionReadableOnlyByParties(t *testing.T) {
	h := newHarness(t)
	nana, alice, bob, aliceAcct, _ := h.setup()

	// Nana creates a third user whose transaction alice should not see.
	var carol userView
	h.decodeInto(h.do("POST", "/api/v1/users", nana, createUserRequest{
		Username: "carol", Password: "carol-pin",
	}), http.StatusCreated, &carol)
	carolToken := h.login("carol", "carol-pin")

	var txn transactionView
	h.decodeInto(h.do("POST", "/api/v1/transfers", carolToken,
		transferRequest{To: accountOf(h, bob), Amount: 5}),
		http.StatusCreated, &txn)

	if w := h.do("GET", "/api/v1/transactions/"+string(txn.ID), alice, nil); w.Code != http.StatusForbidden {
		t.Errorf("alice reading carol's transaction returned %d, want 403", w.Code)
	}
	// Both parties may read it, and so may Nana.
	h.decodeInto(h.do("GET", "/api/v1/transactions/"+string(txn.ID), bob, nil), http.StatusOK, nil)
	h.decodeInto(h.do("GET", "/api/v1/transactions/"+string(txn.ID), nana, nil), http.StatusOK, nil)
	_ = aliceAcct
}

func TestSystemIssuanceAccountIsNotATransferTarget(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	if w := h.do("POST", "/api/v1/transfers", alice, transferRequest{
		To: "account:system-issuance", Amount: 10,
	}); w.Code != http.StatusBadRequest {
		t.Errorf("transfer to the issuance account returned %d, want 400", w.Code)
	}
}

func TestDisabledUserLosesAccessImmediately(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, _ := h.setup()

	var me userView
	h.decodeInto(h.do("GET", "/api/v1/me", alice, nil), http.StatusOK, &me)

	disabled := statusOf("DISABLED")
	h.decodeInto(h.do("PATCH", "/api/v1/users/"+string(me.ID), nana,
		updateUserRequest{Status: &disabled}), http.StatusOK, nil)

	// The existing token must stop working now, not at its expiry.
	if w := h.do("GET", "/api/v1/me", alice, nil); w.Code != http.StatusUnauthorized && w.Code != http.StatusForbidden {
		t.Errorf("disabled user's token returned %d, want 401 or 403", w.Code)
	}
}

func TestLastNanaCannotBeDisabled(t *testing.T) {
	h := newHarness(t)
	nana, _, _, _, _ := h.setup()

	var me userView
	h.decodeInto(h.do("GET", "/api/v1/me", nana, nil), http.StatusOK, &me)

	disabled := statusOf("DISABLED")
	if w := h.do("PATCH", "/api/v1/users/"+string(me.ID), nana,
		updateUserRequest{Status: &disabled}); w.Code != http.StatusForbidden {
		t.Errorf("disabling the only Nana returned %d, want 403 - it would lock the household out", w.Code)
	}
}

// --- auth mechanics ---------------------------------------------------------

func TestUnauthenticatedRequestsAreRejected(t *testing.T) {
	h := newHarness(t)
	h.setup()

	for _, path := range []string{"/api/v1/me", "/api/v1/users", "/api/v1/listings", "/api/v1/transactions"} {
		if w := h.do("GET", path, "", nil); w.Code != http.StatusUnauthorized {
			t.Errorf("GET %s without a token returned %d, want 401", path, w.Code)
		}
	}
	if w := h.do("GET", "/api/v1/me", "not-a-real-token", nil); w.Code != http.StatusUnauthorized {
		t.Errorf("a garbage token returned %d, want 401", w.Code)
	}
}

func TestPKCEVerifierIsRequired(t *testing.T) {
	h := newHarness(t)
	h.setup()

	verifier := strings.Repeat("b", 43)
	sum := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(sum[:])

	issue := func() string {
		var a authorizeResponse
		w := h.do("POST", "/api/v1/auth/authorize", "", authorizeRequest{
			Username: "alice", Password: "alice-pin",
			CodeChallenge: challenge, CodeChallengeMethod: "S256",
			RedirectURI: "http://localhost:5173/callback",
		})
		h.decodeInto(w, http.StatusOK, &a)
		return a.Code
	}

	// A stolen code without the verifier is useless.
	if w := h.do("POST", "/api/v1/auth/token", "", tokenRequest{
		Code: issue(), CodeVerifier: strings.Repeat("x", 43),
		RedirectURI: "http://localhost:5173/callback",
	}); w.Code != http.StatusBadRequest {
		t.Errorf("wrong verifier returned %d, want 400", w.Code)
	}

	// A code bound to one redirect URI cannot be redeemed against another.
	if w := h.do("POST", "/api/v1/auth/token", "", tokenRequest{
		Code: issue(), CodeVerifier: verifier,
		RedirectURI: "https://attacker.example/callback",
	}); w.Code != http.StatusBadRequest {
		t.Errorf("mismatched redirect_uri returned %d, want 400", w.Code)
	}
}

func TestAuthorizationCodeIsSingleUse(t *testing.T) {
	h := newHarness(t)
	h.setup()

	verifier := strings.Repeat("c", 43)
	sum := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(sum[:])

	var a authorizeResponse
	h.decodeInto(h.do("POST", "/api/v1/auth/authorize", "", authorizeRequest{
		Username: "alice", Password: "alice-pin",
		CodeChallenge: challenge, CodeChallengeMethod: "S256",
		RedirectURI: "http://localhost:5173/callback",
	}), http.StatusOK, &a)

	req := tokenRequest{Code: a.Code, CodeVerifier: verifier, RedirectURI: "http://localhost:5173/callback"}
	h.decodeInto(h.do("POST", "/api/v1/auth/token", "", req), http.StatusOK, nil)

	if w := h.do("POST", "/api/v1/auth/token", "", req); w.Code != http.StatusBadRequest {
		t.Errorf("replayed code returned %d, want 400", w.Code)
	}
}

func TestPlainPKCEIsRefused(t *testing.T) {
	h := newHarness(t)
	h.setup()

	if w := h.do("POST", "/api/v1/auth/authorize", "", authorizeRequest{
		Username: "alice", Password: "alice-pin",
		CodeChallenge: "anything", CodeChallengeMethod: "plain",
		RedirectURI: "http://localhost:5173/callback",
	}); w.Code != http.StatusBadRequest {
		t.Errorf("plain method returned %d, want 400", w.Code)
	}
}

func TestFailedLoginsAreRateLimited(t *testing.T) {
	h := newHarness(t)
	h.setup()

	verifier := strings.Repeat("d", 43)
	sum := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(sum[:])
	bad := authorizeRequest{
		Username: "alice", Password: "wrong",
		CodeChallenge: challenge, CodeChallengeMethod: "S256",
		RedirectURI: "http://localhost:5173/callback",
	}

	limited := false
	for i := 0; i < 10; i++ {
		if h.do("POST", "/api/v1/auth/authorize", "", bad).Code == http.StatusTooManyRequests {
			limited = true
			break
		}
	}
	if !limited {
		t.Fatal("ten wrong passwords in a row were never rate limited")
	}

	// The lockout must hold even once the right password is offered, or it
	// would only be a speed bump.
	good := bad
	good.Password = "alice-pin"
	if w := h.do("POST", "/api/v1/auth/authorize", "", good); w.Code != http.StatusTooManyRequests {
		t.Errorf("correct password during lockout returned %d, want 429", w.Code)
	}
}

func TestLogoutRevokesTheToken(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	if w := h.do("POST", "/api/v1/auth/logout", alice, nil); w.Code != http.StatusNoContent {
		t.Fatalf("logout returned %d, want 204", w.Code)
	}
	if w := h.do("GET", "/api/v1/me", alice, nil); w.Code != http.StatusUnauthorized {
		t.Errorf("token still works after logout: %d", w.Code)
	}
}

// --- CORS -------------------------------------------------------------------

func TestCORSAllowsConfiguredOriginOnly(t *testing.T) {
	h := newHarness(t)

	r := httptest.NewRequest("OPTIONS", "/api/v1/status", nil)
	r.Header.Set("Origin", "https://nanacoin.example.net")
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	if got := w.Header().Get("Access-Control-Allow-Origin"); got != "https://nanacoin.example.net" {
		t.Errorf("allowed origin header is %q", got)
	}
	if w.Code != http.StatusNoContent {
		t.Errorf("preflight returned %d, want 204", w.Code)
	}

	// An origin not on the list gets no header at all - never a reflection
	// of whatever it sent.
	r = httptest.NewRequest("OPTIONS", "/api/v1/status", nil)
	r.Header.Set("Origin", "https://evil.example")
	w = httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	if got := w.Header().Get("Access-Control-Allow-Origin"); got != "" {
		t.Errorf("unlisted origin was granted %q", got)
	}
}

// The preflight test above covers OPTIONS. This covers the actual GET, which
// is where a missing origin actually bites: the server answers 200 with the
// body, the browser sees no Access-Control-Allow-Origin, discards the
// response, and the page reports the server as unreachable. curl shows 200
// throughout, which makes it a genuinely confusing failure - so both halves
// are pinned.
func TestCORSHeadersOnPlainRequests(t *testing.T) {
	h := newHarness(t)

	for _, tc := range []struct {
		name   string
		origin string
		want   string
	}{
		{"allowed origin is echoed", "https://nanacoin.example.net", "https://nanacoin.example.net"},
		{"unlisted origin gets nothing", "http://localhost:4200", ""},
		{"no origin header at all", "", ""},
	} {
		t.Run(tc.name, func(t *testing.T) {
			r := httptest.NewRequest("GET", "/api/v1/status", nil)
			if tc.origin != "" {
				r.Header.Set("Origin", tc.origin)
			}
			w := httptest.NewRecorder()
			h.srv.ServeHTTP(w, r)

			// The request itself always succeeds; only the browser's
			// permission to read the answer is in question.
			if w.Code != http.StatusOK {
				t.Fatalf("status %d, want 200", w.Code)
			}
			if got := w.Header().Get("Access-Control-Allow-Origin"); got != tc.want {
				t.Errorf("Access-Control-Allow-Origin is %q, want %q", got, tc.want)
			}
		})
	}
}

// A response that varies by origin must say so, or a shared cache could serve
// one origin the headers granted to another.
func TestCORSSetsVaryOnOrigin(t *testing.T) {
	h := newHarness(t)

	r := httptest.NewRequest("GET", "/api/v1/status", nil)
	r.Header.Set("Origin", "https://nanacoin.example.net")
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	if got := w.Header().Get("Vary"); got != "Origin" {
		t.Errorf("Vary is %q, want Origin", got)
	}
}

// The headers a browser needs for the money endpoints: Authorization carries
// the bearer token and Idempotency-Key makes a retry safe. A preflight that
// omitted either would block every transfer.
func TestCORSAllowsTheHeadersTheClientSends(t *testing.T) {
	h := newHarness(t)

	r := httptest.NewRequest("OPTIONS", "/api/v1/transfers", nil)
	r.Header.Set("Origin", "https://nanacoin.example.net")
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	allowed := w.Header().Get("Access-Control-Allow-Headers")
	for _, h := range []string{"Authorization", "Content-Type", "Idempotency-Key"} {
		if !strings.Contains(allowed, h) {
			t.Errorf("Access-Control-Allow-Headers %q is missing %s", allowed, h)
		}
	}

	methods := w.Header().Get("Access-Control-Allow-Methods")
	for _, m := range []string{"GET", "POST", "PATCH"} {
		if !strings.Contains(methods, m) {
			t.Errorf("Access-Control-Allow-Methods %q is missing %s", methods, m)
		}
	}
}

// --- input handling ---------------------------------------------------------

func TestUnknownFieldsAreRejected(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, bobAcct := h.setup()

	// A misspelled field on a money endpoint must fail loudly rather than
	// transferring a zero amount or an unintended one.
	body := fmt.Sprintf(`{"to":%q,"ammount":30}`, bobAcct)
	r := httptest.NewRequest("POST", "/api/v1/transfers", strings.NewReader(body))
	r.Header.Set("Authorization", "Bearer "+alice)
	r.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	if w.Code != http.StatusBadRequest {
		t.Errorf("misspelled field returned %d, want 400", w.Code)
	}
}

func TestOversizedBodyIsRejected(t *testing.T) {
	h := newHarness(t)
	_, alice, _, _, _ := h.setup()

	huge := fmt.Sprintf(`{"title":%q,"price":1}`, strings.Repeat("x", MaxRequestBody+1))
	r := httptest.NewRequest("POST", "/api/v1/listings", strings.NewReader(huge))
	r.Header.Set("Authorization", "Bearer "+alice)
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, r)

	if w.Code != http.StatusBadRequest && w.Code != http.StatusRequestEntityTooLarge {
		t.Errorf("oversized body returned %d, want 400 or 413", w.Code)
	}
}

func TestLedgerStaysBalancedThroughout(t *testing.T) {
	h := newHarness(t)
	nana, alice, bob, aliceAcct, bobAcct := h.setup()

	h.do("POST", "/api/v1/transfers", alice, transferRequest{To: acct(bobAcct), Amount: 10})
	h.do("POST", "/api/v1/transfers", bob, transferRequest{To: acct(aliceAcct), Amount: 3})
	h.do("POST", "/api/v1/admin/issue", nana, issueRequest{To: acct(aliceAcct), Amount: 40, Reason: "chores"})
	h.do("POST", "/api/v1/admin/retire", nana, retireRequest{From: acct(bobAcct), Amount: 5, Reason: "fine"})

	var st core.Status
	h.decodeInto(h.do("GET", "/api/v1/status", "", nil), http.StatusOK, &st)
	if !st.LedgerBalance {
		t.Error("the ledger no longer balances")
	}

	// Circulation is what was issued minus what was retired. Alice and Bob
	// each got the 100 initial grant; Nana is provisioned without one, since
	// the treasury issues rather than holds.
	want := int64(200 + 40 - 5)
	if st.Circulation != want {
		t.Errorf("circulation is %d, want %d", st.Circulation, want)
	}
}

// --- small helpers ----------------------------------------------------------

func acct(s string) ledger.AccountID { return ledger.AccountID(s) }

func roleOf(s string) users.Role { return users.Role(s) }

func statusOf(s string) users.Status { return users.Status(s) }

func accountOf(h *harness, token string) ledger.AccountID {
	h.t.Helper()
	var me userView
	h.decodeInto(h.do("GET", "/api/v1/me", token, nil), http.StatusOK, &me)
	return me.Account
}
