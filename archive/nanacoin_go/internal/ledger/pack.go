package ledger

import "strconv"

// TransactionIDPrefix is what every derived transaction ID starts with.
const TransactionIDPrefix = "txn-"

// TransactionIDFor derives the client-visible ID of the record at a sequence
// number.
//
// The ID used to be nine random bytes, which meant 16 bytes of string header
// plus a separate allocation per record for a value that could not be
// compressed. Deriving it from the journal sequence removes both.
//
// The trade-off, stated plainly: these IDs are guessable. That is acceptable
// here and would not be everywhere. A transaction ID is not a capability in
// NanaCoin - reading one requires being a party to it or being Nana, checked
// server-side on every request - so knowing that txn-7 exists grants nothing.
// The spec's requirement that authorization codes and tokens be random is
// unaffected; those are secrets, and these are names.
//
// The upside beyond memory: IDs become ordered and human-readable, so
// "txn-41 reverses txn-38" is legible in a way two random strings were not.
func TransactionIDFor(seq uint32) TransactionID {
	return TransactionID(TransactionIDPrefix + strconv.FormatUint(uint64(seq), 10))
}

// SeqForTransactionID parses an ID back to its sequence number, reporting
// whether it was one this system issued.
func SeqForTransactionID(id TransactionID) (uint32, bool) {
	s := string(id)
	if len(s) <= len(TransactionIDPrefix) || s[:len(TransactionIDPrefix)] != TransactionIDPrefix {
		return 0, false
	}
	n, err := strconv.ParseUint(s[len(TransactionIDPrefix):], 10, 32)
	if err != nil {
		return 0, false
	}
	return uint32(n), true
}

// Pack converts a transaction to its compact form.
//
// Reports false when the transaction cannot be represented - more postings
// than MaxInlinePostings. The caller must treat that as a refusal rather than
// dropping the extra postings, since a ledger record with a posting missing
// would no longer sum to zero and the whole model rests on that.
func Pack(t *Transaction, strs *Strings, arena *Arena, seq uint32) (PackedTransaction, bool) {
	if len(t.Postings) > MaxInlinePostings {
		return PackedTransaction{}, false
	}

	p := PackedTransaction{
		Seq:         seq,
		CreatedAt:   t.CreatedAt,
		Kind:        packKind(t.Kind),
		Actor:       strs.Intern(string(t.Actor)),
		Description: arena.Put(t.Description),
		Reference:   arena.Put(t.Reference),
	}
	p.Reverses, _ = SeqForTransactionID(t.Reverses)
	for i, post := range t.Postings {
		p.Accounts[i] = strs.Intern(string(post.Account))
		p.Amounts[i] = post.Amount
		p.Currencies[i] = post.Currency
	}
	return p, true
}

// Unpack rebuilds the public form, for serialising to a client.
//
// Allocates: a Transaction and a postings slice. That is the deliberate half
// of the trade - reads are bounded by the page size and happen a handful of
// times per view. Retained records themselves live in the fixed ring.
func Unpack(p *PackedTransaction, strs *Strings, arena *Arena) *Transaction {
	t := &Transaction{
		ID:          TransactionIDFor(p.Seq),
		Kind:        p.Kind.unpack(),
		CreatedAt:   p.CreatedAt,
		Actor:       UserID(strs.Lookup(p.Actor)),
		Description: arena.Get(p.Description),
		Reference:   arena.Get(p.Reference),
	}
	if p.Reverses != 0 {
		t.Reverses = TransactionIDFor(p.Reverses)
	}

	// Only the postings that were set: a zero account Ref with a zero amount
	// is an unused slot, not a posting against the empty account.
	n := 0
	for i := 0; i < MaxInlinePostings; i++ {
		if p.Accounts[i] != 0 || p.Amounts[i] != 0 {
			n = i + 1
		}
	}
	t.Postings = make([]Posting, n)
	for i := 0; i < n; i++ {
		t.Postings[i] = Posting{
			Currency: p.Currencies[i],
			Account:  AccountID(strs.Lookup(p.Accounts[i])),
			Amount:   p.Amounts[i],
		}
	}
	return t
}

// Affects reports whether the packed record touches an account, without
// unpacking it. This is what makes a history query cheap: it scans the packed
// records and unpacks only the page it returns.
func (p *PackedTransaction) Affects(account Ref) bool {
	for i := 0; i < MaxInlinePostings; i++ {
		if p.Accounts[i] == account {
			return true
		}
	}
	return false
}

// Sum totals the postings in one currency, for the invariant check.
//
// Per currency: see Transaction.Sum. A record whose coin legs balance and
// whose dollar legs do not is broken, and a combined total would hide it.
func (p *PackedTransaction) Sum(c Currency) Amount {
	var total Amount
	for i := 0; i < MaxInlinePostings; i++ {
		if p.Currencies[i] == c {
			total += p.Amounts[i]
		}
	}
	return total
}

// UnpackInto rebuilds a transaction into caller-owned storage.
//
// Unpack allocates a Transaction and a postings slice per call, which is
// exactly the per-record cost the walkers exist to avoid. This writes into a
// scratch Transaction and a caller-supplied postings array instead, so a walk
// over a thousand records allocates nothing beyond the strings the intern
// table already holds.
//
// The postings slice must have room for MaxInlinePostings. The resulting
// Transaction aliases it, so it is valid only until the next call.
func UnpackInto(p *PackedTransaction, strs *Strings, arena *Arena, t *Transaction, postings []Posting) {
	t.ID = TransactionIDFor(p.Seq)
	t.Kind = p.Kind.unpack()
	t.CreatedAt = p.CreatedAt
	t.Actor = UserID(strs.Lookup(p.Actor))
	t.Description = arena.Get(p.Description)
	t.Reference = arena.Get(p.Reference)
	t.Reverses = ""
	if p.Reverses != 0 {
		t.Reverses = TransactionIDFor(p.Reverses)
	}

	n := 0
	for i := 0; i < MaxInlinePostings; i++ {
		if p.Accounts[i] != 0 || p.Amounts[i] != 0 {
			n = i + 1
		}
	}
	for i := 0; i < n; i++ {
		postings[i] = Posting{
			Account: AccountID(strs.Lookup(p.Accounts[i])),
			Amount:  p.Amounts[i],
		}
	}
	t.Postings = postings[:n]
}

// TransactionIDInto writes the derived ID into a caller-owned buffer.
//
// TransactionIDFor allocates a string per call through strconv, which on a
// list of thirty transactions is thirty allocations for values written
// straight to output and discarded. This appends into a scratch array
// instead.
//
// The returned slice aliases dst.
func TransactionIDInto(dst []byte, seq uint32) []byte {
	dst = append(dst[:0], TransactionIDPrefix...)
	return strconv.AppendUint(dst, uint64(seq), 10)
}
