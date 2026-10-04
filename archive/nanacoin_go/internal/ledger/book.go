package ledger

import (
	"errors"
	"fmt"
)

var (
	ErrUnbalanced      = errors.New("postings do not sum to zero")
	ErrEmpty           = errors.New("transaction has no postings")
	ErrNonPositive     = errors.New("amount must be positive")
	ErrInsufficient    = errors.New("insufficient funds")
	ErrDuplicateID     = errors.New("transaction id already in book")
	ErrNotFound        = errors.New("transaction not found")
	ErrAlreadyReversed = errors.New("transaction already reversed")
	ErrReverseReversal = errors.New("cannot reverse a reversal")
	ErrSystemAccount   = errors.New("system account is not a valid participant here")
	ErrTooManyPostings = errors.New("transaction has more postings than the ledger stores inline")
	ErrSequenceFull    = errors.New("transaction sequence exhausted")
	ErrAccountCapacity = errors.New("ledger account capacity exhausted")
)

// Capacity is the maximum recent history retained in RAM. Balances and IDs
// cover the entire lifetime, including records that have left this ring.
// Modulo indexing supports arbitrary capacities; RAM headroom matters more
// than using a power of two on this board.
const Capacity = 365

// InsufficientFundsError says which account fell short and by how much, so the
// API can tell a user "you have 12, you need 20" rather than a bare refusal.
type InsufficientFundsError struct {
	Account   AccountID
	Balance   Amount
	Requested Amount
}

func (e *InsufficientFundsError) Error() string {
	return fmt.Sprintf("account %s has %d, needs %d", e.Account, e.Balance, e.Requested)
}
func (e *InsufficientFundsError) Unwrap() error { return ErrInsufficient }

// Book retains a fixed ring of recent packed transactions. Balances and
// sequence numbers cover all applied transactions; opening balances summarize
// evicted postings. Only retained transactions can be read or reversed.
//
// Book is not safe for concurrent use. Its owner holds the service lock
// across writes and reads, including rendering references into the text arena.
type Book struct {
	// strs interns the strings the records share. Held here rather than
	// globally so tests get an isolated table.
	strs *Strings

	// arena holds the strings that do not repeat - memos and references.
	// Interning those consumed a table slot per distinct value that was
	// never reused, which was the last unbounded growth in the system.
	arena *Arena

	// txns uses (sequence-1) modulo Capacity; count tracks live entries.
	txns  *[Capacity]PackedTransaction
	count int
	total uint32

	// revBy links retained originals to reversals; clear it when reusing a slot.
	revBy *[Capacity]uint32

	// Account state is indexed by account, not by every interned name/title.
	balances     []balanceRecord
	balanceCount int
	check        *[32]Amount
}

func NewBook() *Book {
	return NewBookWithStrings(NewStrings(), NewArena())
}

// NewBookWithStrings builds a book over an existing intern table, so that a
// service can share one table across its ledger, users and listings.
type balanceRecord struct {
	balance, opening Amount
	ref              Ref
}

func NewBookWithStrings(strs *Strings, arena *Arena) *Book {
	return NewBookWithAccountCapacity(strs, arena, MaxInterned)
}

// NewBookWithAccountCapacity reserves lifetime account state once. The service
// uses its existing account ceiling plus issuance; generic ledger users retain
// the full intern-table ceiling through NewBookWithStrings.
func NewBookWithAccountCapacity(strs *Strings, arena *Arena, capacity int) *Book {
	if capacity < 1 || capacity > MaxInterned {
		panic("invalid ledger account capacity")
	}

	return &Book{strs: strs, arena: arena,
		txns: new([Capacity]PackedTransaction), revBy: new([Capacity]uint32),
		balances: make([]balanceRecord, capacity), check: new([32]Amount),
	}
}

// Strings exposes the intern table, for callers that store the same account
// and user IDs and should share it.
func (b *Book) Strings() *Strings { return b.strs }

// Arena exposes the text store, shared with the domain store so that one
// fixed buffer serves both.
func (b *Book) Arena() *Arena { return b.arena }

// balanceOf reads the cached balance for an interned account.
func (b *Book) balanceIndex(ref Ref) int {
	for i := 0; i < b.balanceCount; i++ {
		if b.balances[i].ref == ref {
			return i
		}
	}
	return -1
}
func (b *Book) balanceOf(ref Ref) Amount {
	if i := b.balanceIndex(ref); i >= 0 {
		return b.balances[i].balance
	}
	return 0
}
func (b *Book) openingOf(ref Ref) Amount {
	if i := b.balanceIndex(ref); i >= 0 {
		return b.balances[i].opening
	}
	return 0
}
func (b *Book) balanceSlot(ref Ref) *balanceRecord {
	if i := b.balanceIndex(ref); i >= 0 {
		return &b.balances[i]
	}
	if b.balanceCount == len(b.balances) {
		panic("ledger capacity was not validated")
	}
	slot := &b.balances[b.balanceCount]
	slot.ref = ref
	b.balanceCount++
	return slot
}
func (b *Book) addBalance(ref Ref, delta Amount) { b.balanceSlot(ref).balance += delta }

// Check before evicting history or committing a journal record. Replay calls
// this too: a journal larger than the configured account pool must fail cleanly.
func (b *Book) checkAccountCapacity(t *Transaction) error {
	if len(t.Postings) > MaxInlinePostings {
		return ErrTooManyPostings
	}
	var newRefs [MaxInlinePostings]Ref
	n := 0
	for _, p := range t.Postings {
		ref := b.strs.Intern(string(p.Account))
		if b.balanceIndex(ref) >= 0 {
			continue
		}
		duplicate := false
		for i := 0; i < n; i++ {
			if newRefs[i] == ref {
				duplicate = true
				break
			}
		}
		if !duplicate {
			newRefs[n] = ref
			n++
		}
	}
	if b.balanceCount+n > len(b.balances) {
		return ErrAccountCapacity
	}
	return nil
}

// Balance is the fold over every posting touching the account. It reads the
// cache; CheckInvariants reconciles opening balances and retained postings.
func (b *Book) Balance(acct AccountID) Amount {
	return b.balanceOf(b.strs.Find(string(acct)))
}

// BalanceIn is the balance of an account in one currency.
//
// Dollars live in a separate account - see USDAccount - so this resolves to
// the right one rather than filtering postings. That keeps a balance a fold
// over one account, which is what the whole balance cache assumes.
func (b *Book) BalanceIn(acct AccountID, c Currency) Amount {
	if c != USD {
		return b.Balance(acct)
	}
	// The dollar wallet's name is derived, not stored, so resolving it via
	// Balance would concatenate a string - one heap allocation per call, on a
	// path that runs for every user in every household listing. Built in a
	// stack buffer instead and looked up as bytes.
	if len(acct)+len(USDSuffix) > MaxAccountIDLen {
		return 0
	}
	var buf [MaxAccountIDLen]byte
	n := copy(buf[:], acct)
	n += copy(buf[n:], USDSuffix)
	return b.balanceOf(b.strs.FindBytes(buf[:n]))
}

// Circulation is the total NanaCoin in existence: the negation of the issuance
// account's balance.
func (b *Book) Circulation() Amount {
	return -b.Balance(SystemIssuance)
}

// USDHeld is the total dollars the household holds, in cents.
//
// The same construction as Circulation: dollars enter through USDIssuance, so
// the negation of its balance is what everyone else holds between them. That
// is the number that answers "does Nana have dollars to sell" for the whole
// household rather than one account.
func (b *Book) USDHeld() Amount {
	return -b.Balance(USDIssuance)
}

// Get returns a transaction in its public form, allocating it.
func (b *Book) Get(id TransactionID) (*Transaction, bool) {
	i, ok := b.indexOf(id)
	if !ok {
		return nil, false
	}
	return Unpack(&b.txns[i], b.strs, b.arena), true
}

// indexOf turns a client-visible ID into a slice index.
func (b *Book) indexOf(id TransactionID) (int, bool) {
	seq, ok := SeqForTransactionID(id)
	if !ok || seq == 0 || seq > b.total || b.total-seq >= uint32(b.count) {
		return 0, false
	}
	i := int((seq - 1) % Capacity)
	return i, b.txns[i].Seq == seq
}

func (b *Book) Len() int      { return b.count }
func (b *Book) Total() uint32 { return b.total }
func (b *Book) Oldest() uint32 {
	if b.count == 0 {
		return 0
	}
	return b.total - uint32(b.count) + 1
}
func (b *Book) slot(position int) int {
	return int((b.Oldest() - 1 + uint32(position)) % Capacity)
}

// NextID is the ID the next appended record will be given.
//
// Needed because the journal is written before the record is applied - that
// ordering is what keeps a reported success from outliving a flash failure -
// but the ID is derived from the position the record will occupy. So the
// caller asks for the ID first, journals the event carrying it, and then
// appends.
//
// Safe only under the service's lock, which is the only place it is called:
// two callers taking the same ID would both write it, and the second append
// would be rejected as a duplicate.
func (b *Book) NextID() TransactionID {
	return TransactionIDFor(b.total + 1)
}

// All returns retained transactions in append order, in public form.
//
// Allocates one Transaction per record, so it is for callers that genuinely
// need all of them - CheckInvariants uses the packed records directly instead.
func (b *Book) All() []*Transaction {
	out := make([]*Transaction, b.count)
	for i := range out {
		out[i] = Unpack(&b.txns[b.slot(i)], b.strs, b.arena)
	}
	return out
}

// History returns transactions affecting an account, newest first, capped at
// limit (<=0 means all).
//
// Scans the packed records and unpacks only those it returns, so the cost is
// proportional to the page rather than to the history.
func (b *Book) History(acct AccountID, limit int) []*Transaction {
	ref := b.strs.Find(string(acct))

	var out []*Transaction
	for pos := b.count - 1; pos >= 0; pos-- {
		i := b.slot(pos)
		if !b.txns[i].Affects(ref) {
			continue
		}
		out = append(out, Unpack(&b.txns[i], b.strs, b.arena))
		if limit > 0 && len(out) == limit {
			break
		}
	}
	return out
}

// ReversalOf returns the transaction that reversed id, if any.
func (b *Book) ReversalOf(id TransactionID) (TransactionID, bool) {
	i, ok := b.indexOf(id)
	if !ok || i >= len(b.revBy) || b.revBy[i] == 0 {
		return "", false
	}
	return TransactionIDFor(b.revBy[i]), true
}

// Validate checks a transaction against the book without applying it. Append
// calls this first, so a rejected transaction leaves no trace.
//
// allowOverdraft lets Nana push an account negative during an administrative
// correction (spec 11): the correction is more important than the invariant,
// and the resulting negative balance is deliberately left visible.
func (b *Book) Validate(t *Transaction, allowOverdraft bool) error {
	if b.total == ^uint32(0) {
		return ErrSequenceFull
	}
	if len(t.Postings) == 0 {
		return ErrEmpty
	}
	if len(t.Postings) > MaxInlinePostings {
		return ErrTooManyPostings
	}
	if seq, valid := SeqForTransactionID(t.ID); valid && seq > 0 && seq <= b.total {
		return ErrDuplicateID
	}
	// Every currency the transaction touches must balance on its own.
	if t.Sum(NANA) != 0 || t.Sum(USD) != 0 {
		return ErrUnbalanced
	}

	// Project the postings onto current balances before deciding, so that a
	// transaction which both debits and credits the same account is judged
	// on its net effect.
	var (
		refs  [MaxInlinePostings]Ref
		nets  [MaxInlinePostings]Amount
		count int
	)
	for _, p := range t.Postings {
		if p.Amount == 0 {
			return ErrNonPositive
		}
		ref := b.strs.Intern(string(p.Account))
		found := false
		for i := 0; i < count; i++ {
			if refs[i] == ref {
				nets[i] += p.Amount
				found = true
				break
			}
		}
		if !found {
			refs[count] = ref
			nets[count] = p.Amount
			count++
		}
	}
	if err := b.checkAccountCapacity(t); err != nil {
		return err
	}
	if allowOverdraft {
		return nil
	}

	// Both issuance accounts are unbounded below, by construction: their
	// negative balance IS the amount in circulation. USDIssuance is the same
	// idea for dollars - without the exemption no dollar could ever enter the
	// household, because the first issuance would overdraw an empty account.
	// Find, not Intern: this runs on every validate, and Intern would add a
	// slot for USDIssuance in a household that has never touched a dollar.
	// That is the unbounded growth the intern table exists to avoid - and a
	// test caught it, which is why the table has one.
	//
	// SystemIssuance is already interned by then (every transaction names it
	// or an account beside it), so Find returns the same ref Intern would.
	// An unknown name returns the zero ref, which matches no posting.
	issuance := b.strs.Find(string(SystemIssuance))
	usdIssuance := b.strs.Find(string(USDIssuance))
	for i := 0; i < count; i++ {
		if refs[i] == issuance || refs[i] == usdIssuance {
			continue
		}
		if after := b.balanceOf(refs[i]) + nets[i]; after < 0 {
			return &InsufficientFundsError{
				Account:   AccountID(b.strs.Lookup(refs[i])),
				Balance:   b.balanceOf(refs[i]),
				Requested: -nets[i],
			}
		}
	}
	return nil
}

// Append validates and commits, returning the sequence number the record was
// given and therefore its client-visible ID.
//
// The caller is responsible for having durably journalled the transaction
// first; Book is the RAM half of that pair.
func (b *Book) Append(t *Transaction, allowOverdraft bool) (TransactionID, error) {
	if err := b.Validate(t, allowOverdraft); err != nil {
		return "", err
	}
	return b.apply(t)
}

// Replay applies a transaction read back from the journal. It skips
// validation: the record was valid when it was written, and re-judging it
// against a partially rebuilt book would reject legitimate history (a
// reversal that leaves an account negative, say). A corrupt record is caught
// by the journal's CRC, not here.
func (b *Book) Replay(t *Transaction) (TransactionID, error) {
	return b.apply(t)
}

func (b *Book) apply(t *Transaction) (TransactionID, error) {
	if b.total == ^uint32(0) {
		return "", ErrSequenceFull
	}
	if len(t.Postings) > MaxInlinePostings {
		return "", ErrTooManyPostings
	}
	if err := b.checkAccountCapacity(t); err != nil {
		return "", err
	}
	seq := b.total + 1
	// IDs from a journal must remain monotonic; a retained suffix alone is
	// not a replayable ledger without an opening checkpoint.
	if t.ID != "" && t.ID != TransactionIDFor(seq) {
		return "", ErrDuplicateID
	}
	if b.count == Capacity {
		b.evictOldest()
	}
	// Reclaim old text too. Long memos may shorten the retained window below
	// Capacity rather than permanently exhausting the shared text arena.
	for b.count > 0 && !b.arena.CanStore(t.Description, t.Reference) {
		b.evictOldest()
	}

	packed, ok := Pack(t, b.strs, b.arena, seq)
	if !ok {
		return "", ErrTooManyPostings
	}

	i := int((seq - 1) % Capacity)
	b.txns[i] = packed
	b.revBy[i] = 0 // an old reversal link must never attach to a reused slot
	b.total = seq
	b.count++

	for i := 0; i < MaxInlinePostings; i++ {
		if packed.Accounts[i] != 0 || packed.Amounts[i] != 0 {
			b.addBalance(packed.Accounts[i], packed.Amounts[i])
		}
	}

	// Record the reversal link on the original, so ReversalOf needs no scan.
	if packed.Kind == PackedReversal && t.Reverses != "" {
		if i, found := b.indexOf(t.Reverses); found {
			b.revBy[i] = seq
		}
	}
	return TransactionIDFor(seq), nil
}

func (b *Book) evictOldest() {
	i := b.slot(0)
	p := &b.txns[i]
	for j, ref := range p.Accounts {
		if p.Amounts[j] != 0 {
			b.balanceSlot(ref).opening += p.Amounts[j]
		}
	}
	b.arena.Release(p.Description)
	b.arena.Release(p.Reference)
	b.txns[i] = PackedTransaction{}
	b.revBy[i] = 0
	b.count--
}

// BuildReversal constructs the transaction that undoes id: the same postings
// with opposite signs. It does not append it.
//
// The new ID is assigned by Append, so it is not a parameter: derived IDs come
// from the sequence, which only the book knows.
func (b *Book) BuildReversal(id TransactionID, actor UserID, reason string, now int64) (*Transaction, error) {
	return b.BuildReversalInto(id, actor, reason, now, new(Transaction), make([]Posting, MaxInlinePostings))
}

func (b *Book) BuildReversalInto(id TransactionID, actor UserID, reason string, now int64, dst *Transaction, postings []Posting) (*Transaction, error) {
	if len(postings) < MaxInlinePostings {
		return nil, ErrTooManyPostings
	}

	i, ok := b.indexOf(id)
	if !ok {
		return nil, ErrNotFound
	}
	orig := &b.txns[i]

	if orig.Kind == PackedReversal {
		// Reversing a reversal is re-doing the original, which is a
		// confusing way to express an intent that is better served by a
		// fresh transfer. Refusing keeps the audit trail readable.
		return nil, ErrReverseReversal
	}
	if i < len(b.revBy) && b.revBy[i] != 0 {
		return nil, ErrAlreadyReversed
	}

	n := 0
	for j := 0; j < MaxInlinePostings; j++ {
		if orig.Accounts[j] != 0 || orig.Amounts[j] != 0 {
			n = j + 1
		}
		postings[j] = Posting{Account: AccountID(b.strs.Lookup(orig.Accounts[j])), Amount: -orig.Amounts[j]}
	}
	*dst = Transaction{Kind: KindReversal, CreatedAt: now, Actor: actor, Description: reason, Reference: b.arena.Get(orig.Reference), Reverses: id, Postings: postings[:n]}
	return dst, nil
}

// CheckInvariants verifies the two properties the whole design rests on: every
// stored transaction balances, and the cached balances equal the fold over
// postings. Called on boot after replay and available to tests.
//
// Reads the packed records directly rather than unpacking them, so checking a
// year of history allocates nothing.
func (b *Book) CheckInvariants() error {
	// Two totals, not one. A single grand total would let 100 coins out
	// cancel 100 cents in and call a broken book balanced.
	var grand, grandUSD Amount
	for i := 0; i < b.balanceCount; i++ {
		grand += b.balances[i].opening
	}
	for pos := 0; pos < b.count; pos++ {
		t := &b.txns[b.slot(pos)]
		if sum := t.Sum(NANA); sum != 0 {
			return fmt.Errorf("transaction %s sums to %d NanaCoin, not zero",
				TransactionIDFor(t.Seq), sum)
		}
		if sum := t.Sum(USD); sum != 0 {
			return fmt.Errorf("transaction %s sums to %d cents, not zero",
				TransactionIDFor(t.Seq), sum)
		}
		for j := 0; j < MaxInlinePostings; j++ {
			if int(t.Accounts[j]) >= MaxInterned {
				return fmt.Errorf("invalid account reference")
			}
			if t.Currencies[j] == USD {
				grandUSD += t.Amounts[j]
			} else {
				grand += t.Amounts[j]
			}
		}
	}
	if grand != 0 {
		return fmt.Errorf("ledger total is %d NanaCoin, not zero", grand)
	}
	if grandUSD != 0 {
		return fmt.Errorf("ledger total is %d cents, not zero", grandUSD)
	}
	// Reconcile 32 references at a time. Sixteen bounded passes trade a little
	// CPU for 3,840 bytes of permanent RAM, without reducing account/history
	// capacity or allocating scratch on the small ESP32 goroutine stack.
	for base := 0; base < MaxInterned; base += len(b.check) {
		for i := range b.check {
			b.check[i] = b.openingOf(Ref(base + i))
		}
		for pos := 0; pos < b.count; pos++ {
			t := &b.txns[b.slot(pos)]
			for j := 0; j < MaxInlinePostings; j++ {
				index := int(t.Accounts[j]) - base
				if index >= 0 && index < len(b.check) {
					b.check[index] += t.Amounts[j]
				}
			}
		}
		for i := range b.check {
			ref := base + i
			if got := b.balanceOf(Ref(ref)); got != b.check[i] {
				return fmt.Errorf("cached balance for %s is %d, ledger says %d", b.strs.Lookup(Ref(ref)), got, b.check[i])
			}
		}
	}
	return nil
}

// Incremental readers.
//
// History and All allocate a Transaction per record and return them all at
// once. That is fine for a desktop and is what exhausted the board: All in
// particular unpacked the entire ledger so that the caller could discard all
// but the newest page of it.
//
// The walkers below unpack one record, hand it over, and move on. The
// Transaction handed to fn is only valid until fn returns - the caller must
// copy anything it keeps - which is what lets the same storage be reused
// instead of allocated per record.

// EachHistory calls fn for each transaction affecting acct, newest first,
// stopping after limit records (<=0 means all) or when fn returns false.
//
// The *Transaction passed to fn must not be retained: it is rebuilt in place
// for each record.
func (b *Book) EachHistory(acct AccountID, limit int, fn func(*Transaction) bool) {
	ref := b.strs.Find(string(acct))

	var scratch Transaction
	var postings [MaxInlinePostings]Posting

	n := 0
	for pos := b.count - 1; pos >= 0; pos-- {
		i := b.slot(pos)
		if !b.txns[i].Affects(ref) {
			continue
		}
		UnpackInto(&b.txns[i], b.strs, b.arena, &scratch, postings[:])
		if !fn(&scratch) {
			return
		}
		n++
		if limit > 0 && n == limit {
			return
		}
	}
}

// EachRecent walks the whole ledger newest first, up to limit.
//
// This is what the full-ledger endpoint needs. The old path was All() plus a
// reversal plus a truncation, which unpacked every record in the book to
// return thirty of them.
func (b *Book) EachRecent(limit int, fn func(*Transaction) bool) {
	var scratch Transaction
	var postings [MaxInlinePostings]Posting

	n := 0
	for pos := b.count - 1; pos >= 0; pos-- {
		i := b.slot(pos)
		UnpackInto(&b.txns[i], b.strs, b.arena, &scratch, postings[:])
		if !fn(&scratch) {
			return
		}
		n++
		if limit > 0 && n == limit {
			return
		}
	}
}

// EachPackedHistory walks the packed records affecting an account, newest
// first, without unpacking them.
//
// The render path uses this instead of EachHistory: a caller that only
// serialises a record does not need the Transaction struct, and building one
// meant copying every string out of the arena and intern table for values
// written straight to the socket.
func (b *Book) EachPackedHistory(acct AccountID, limit int, fn func(*PackedTransaction) bool) {
	ref := b.strs.Find(string(acct))
	n := 0
	for pos := b.count - 1; pos >= 0; pos-- {
		i := b.slot(pos)
		if !b.txns[i].Affects(ref) {
			continue
		}
		if !fn(&b.txns[i]) {
			return
		}
		n++
		if limit > 0 && n == limit {
			return
		}
	}
}

// EachPackedRecent walks every packed record, newest first.
func (b *Book) EachPackedRecent(limit int, fn func(*PackedTransaction) bool) {
	n := 0
	for pos := b.count - 1; pos >= 0; pos-- {
		i := b.slot(pos)
		if !fn(&b.txns[i]) {
			return
		}
		n++
		if limit > 0 && n == limit {
			return
		}
	}
}

// ReversesSeq is the sequence number a reversal undoes, or 0.
//
// Numeric links survive eviction of the original without interning unique IDs.
func (b *Book) ReversesSeq(p *PackedTransaction) uint32 {
	return p.Reverses
}

// ReversalSeq is the sequence of the reversal that undid the given record,
// or 0 if it was never reversed.
func (b *Book) ReversalSeq(seq uint32) uint32 {
	if seq == 0 || seq > b.total || b.total-seq >= uint32(b.count) {
		return 0
	}
	return b.revBy[(seq-1)%Capacity]
}

// Packed returns a retained record. Only valid while the owning service is locked.
func (b *Book) Packed(seq uint32) *PackedTransaction {
	if seq == 0 || seq > b.total || b.total-seq >= uint32(b.count) {
		return nil
	}
	return &b.txns[(seq-1)%Capacity]
}
