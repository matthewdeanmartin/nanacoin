package core

import (
	"fmt"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Transfer moves NanaCoin between two household accounts.
//
// The source account is derived from the authenticated user rather than taken
// from the request: a client cannot name the account it is spending from
// (spec 10).
func (s *Service) Transfer(actor *users.User, to ledger.AccountID, amount ledger.Amount, memo string, scratch ...*WriteResult) (*ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, err
	}
	if err := validateAmount(amount); err != nil {
		return nil, err
	}
	if err := validateText(memo, MaxMemoLen, "memo"); err != nil {
		return nil, err
	}
	from := actor.Account
	if to == from {
		return nil, ErrSelfDeal
	}
	if err := s.checkUserAccountLocked(to); err != nil {
		return nil, err
	}

	out := writeResult(scratch)
	txn := &out.Transaction
	*txn = ledger.Transaction{
		ID:          s.book.NextID(),
		Kind:        ledger.KindTransfer,
		CreatedAt:   s.now().Unix(),
		Actor:       actor.ID,
		Description: memo,
		Postings:    out.Postings[:],
	}
	out.Postings = [ledger.MaxInlinePostings]ledger.Posting{
		{Account: from, Amount: -amount},
		{Account: to, Amount: amount},
	}
	// Validate before journalling: a transfer that would overdraw must
	// leave no record at all, or the ledger accumulates failed attempts.
	if err := s.book.Validate(txn, false); err != nil {
		return nil, err
	}
	if err := s.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		return nil, err
	}
	return txn, nil
}

// Issue creates new NanaCoin from the system issuance account. Nana only.
func (s *Service) Issue(actor *users.User, to ledger.AccountID, amount ledger.Amount, reason string, scratch ...*WriteResult) (*ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if !actor.IsNana() {
		return nil, ErrForbidden
	}
	return s.issueLocked(actor.ID, to, amount, reason, scratch...)
}

func (s *Service) issueLocked(actor ledger.UserID, to ledger.AccountID, amount ledger.Amount, reason string, scratch ...*WriteResult) (*ledger.Transaction, error) {
	if err := validateAmount(amount); err != nil {
		return nil, err
	}
	if err := validateText(reason, MaxMemoLen, "reason"); err != nil {
		return nil, err
	}
	if err := s.checkUserAccountLocked(to); err != nil {
		return nil, err
	}

	out := writeResult(scratch)
	txn := &out.Transaction
	*txn = ledger.Transaction{
		ID:          s.book.NextID(),
		Kind:        ledger.KindIssue,
		CreatedAt:   s.now().Unix(),
		Actor:       actor,
		Description: reason,
		Postings:    out.Postings[:],
	}
	out.Postings = [ledger.MaxInlinePostings]ledger.Posting{
		// Issuance is a normal balanced transaction against an account
		// allowed to go negative, not an exception to the rules. That
		// keeps "every coin is accounted for" a property of the whole
		// ledger rather than a claim about a special case (spec 9).
		{Account: ledger.SystemIssuance, Amount: -amount},
		{Account: to, Amount: amount},
	}
	if err := s.book.Validate(txn, false); err != nil {
		return nil, err
	}
	if err := s.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		return nil, err
	}
	return txn, nil
}

// Retire destroys NanaCoin, returning it to the issuance account. Nana only.
func (s *Service) Retire(actor *users.User, from ledger.AccountID, amount ledger.Amount, reason string, scratch ...*WriteResult) (*ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if !actor.IsNana() {
		return nil, ErrForbidden
	}
	if err := validateAmount(amount); err != nil {
		return nil, err
	}
	if err := validateText(reason, MaxMemoLen, "reason"); err != nil {
		return nil, err
	}
	if err := s.checkUserAccountLocked(from); err != nil {
		return nil, err
	}

	out := writeResult(scratch)
	txn := &out.Transaction
	*txn = ledger.Transaction{
		ID:          s.book.NextID(),
		Kind:        ledger.KindRetire,
		CreatedAt:   s.now().Unix(),
		Actor:       actor.ID,
		Description: reason,
		Postings:    out.Postings[:],
	}
	out.Postings = [ledger.MaxInlinePostings]ledger.Posting{
		{Account: from, Amount: -amount},
		{Account: ledger.SystemIssuance, Amount: amount},
	}
	if err := s.book.Validate(txn, false); err != nil {
		return nil, err
	}
	if err := s.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		return nil, err
	}
	return txn, nil
}

// Reverse undoes a transaction by appending its mirror image. Nana only.
//
// Overdraft is permitted here: if Bob has already spent the money Alice is
// getting back, the correction still happens and Bob's balance goes negative
// where the household can see it (spec 11). Blocking the reversal would leave
// the wrong version of events as the permanent record, which is worse.
func (s *Service) Reverse(actor *users.User, id ledger.TransactionID, reason string, scratch ...*WriteResult) (*ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if !actor.IsNana() {
		return nil, ErrForbidden
	}
	if err := validateText(reason, MaxMemoLen, "reason"); err != nil {
		return nil, err
	}

	out := writeResult(scratch)
	txn, err := s.book.BuildReversalInto(id, actor.ID, reason, s.now().Unix(), &out.Transaction, out.Postings[:])
	if err != nil {
		return nil, err
	}
	// BuildReversal leaves the ID unset, since only the book knows the next
	// one. Fill it in before journalling, so the record on flash carries the
	// same ID the client is told.
	txn.ID = s.book.NextID()
	if err := s.book.Validate(txn, true); err != nil {
		return nil, err
	}
	if err := s.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		return nil, err
	}

	// A reversed purchase leaves the listing sold. Un-selling it would be a
	// second state change that could half-land, and leaving Nana to relist
	// it by hand is the household-scale answer the spec asks for (2.4). The
	// listing SoldTx still points at the original, so the trail from
	// listing to reversal stays intact.
	return txn, nil
}

// checkUserAccountLocked rejects unknown accounts, the issuance account and
// accounts belonging to disabled users. The issuance check is what keeps
// SystemIssuance from ever being a transfer target (spec 9).
func (s *Service) checkUserAccountLocked(id ledger.AccountID) error {
	if id == ledger.SystemIssuance {
		return fmt.Errorf("%w: %s", ledger.ErrSystemAccount, id)
	}
	acct := s.accountByID(id)
	if acct == nil {
		return ErrAccountUnknown
	}
	owner, _ := s.userByID(acct.UserID)
	if owner == nil || !owner.IsActive() {
		return fmt.Errorf("%w: account %s", ErrDisabled, id)
	}
	return nil
}

func validateAmount(a ledger.Amount) error {
	if a <= 0 {
		return fmt.Errorf("%w: amount must be positive", ErrBadInput)
	}
	// An upper bound keeps a typo from creating a quantity of NanaCoin that
	// makes every later sum look absurd, and keeps sums far from int64
	// overflow.
	if a > 1_000_000_000 {
		return fmt.Errorf("%w: amount is implausibly large", ErrBadInput)
	}
	return nil
}

func validateText(v string, max int, what string) error {
	if len(v) > max {
		return fmt.Errorf("%w: %s must be at most %d characters", ErrBadInput, what, max)
	}
	for _, r := range v {
		if r < 0x20 || r == 0x7f {
			return fmt.Errorf("%w: %s contains a control character", ErrBadInput, what)
		}
	}
	return nil
}

// Idempotent deduplicates (user, endpoint, key) while its bounded receipt remains.
// A persistent journal can restore retained receipts on replay, but the money
// event and receipt are separate appends: a crash between them is not covered.
// The board currently uses a discard journal and loses receipts on reboot.
//
// An empty key means the caller did not ask for idempotency, and fn runs
// normally.
// The callback returns the encoded response body rather than a value to
// marshal. That keeps reflection out of this package entirely: the API layer
// already has a typed encoder for every shape it returns, so it hands over
// bytes and core stores them opaquely.
func (s *Service) Idempotent(userID ledger.UserID, endpoint, key string, fn func() ([]byte, error)) ([]byte, error) {
	return s.IdempotentInto(userID, endpoint, key, nil, fn)
}

// IdempotentInto copies cached receipts into dst before releasing mu. New
// responses can be encoded directly into dst by fn. A nil dst is the allocating
// compatibility API for host callers; board handlers always provide storage.
func (s *Service) IdempotentInto(userID ledger.UserID, endpoint, key string, dst []byte, fn func() ([]byte, error)) ([]byte, error) {
	if key == "" {
		return fn()
	}
	if len(key) > MaxIdempotencyKey {
		return nil, fmt.Errorf("%w: idempotency key too long", ErrBadInput)
	}

	// Fixed lock stripes serialize matching keys through execution and receipt
	// publication. Hash collisions only serialize unrelated keyed operations.
	var hash uint32 = 2166136261
	for _, part := range []string{string(userID), endpoint, key} {
		for i := 0; i < len(part); i++ {
			hash = (hash ^ uint32(part[i])) * 16777619
		}
		hash = (hash ^ 255) * 16777619
	}
	gate := &s.idemGates[hash%uint32(len(s.idemGates))]
	gate.Lock()
	defer gate.Unlock()
	s.mu.Lock()
	off, size, seen := s.idem.find(userID, endpoint, key)
	if seen {
		if dst == nil {
			dst = make([]byte, size)
		}
		if len(dst) < size {
			s.mu.Unlock()
			return nil, ErrCapacity
		}
		copy(dst, s.idem.data[off:off+size])
		s.mu.Unlock()
		return dst[:size], nil
	}
	s.mu.Unlock()

	result, err := fn()
	if err != nil {
		// Failures are not recorded. A retry of a transfer that failed
		// for lack of funds should be free to succeed once the user has
		// been paid, and recording the failure would freeze the refusal
		// in place.
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	ev := idempotencyEvent{Key: key, UserID: userID, Endpoint: endpoint, Result: result, At: s.now().Unix()}
	if err := s.commitEvent(storage.TypeIdempotency, &ev); err != nil {
		// The money moved and was journalled; only the receipt failed.
		// Returning the result is honest - telling the user it failed
		// would not be - and the worst consequence is that one retry goes
		// undeduplicated after reboot. Keep the RAM receipt for concurrent retries.
		s.idem.remember(userID, endpoint, key, result)
		return result, nil
	}
	return result, nil
}
