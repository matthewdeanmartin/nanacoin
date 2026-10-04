package core

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
)

// Rendering straight from packed records.
//
// # Why
//
// The read path went: packed record -> unpacked Transaction -> view struct ->
// JSON. Each arrow materialised strings the next one immediately discarded.
// Measured on a thirty-item list: 283 allocations, about nine per record -
// two arena copies for the memo and reference, a strconv allocation for the
// derived ID, a postings slice, and the account names.
//
// Every one of those values is written to the socket and thrown away. None
// of them needs to be a Go string, and on a board where fragmentation is the
// failure mode, a short-lived string between two long-lived records is
// exactly the thing to avoid.
//
// # What this is
//
// A view of one packed record that hands out *bytes* rather than strings:
// arena slices and interned values, aliasing storage that outlives the call.
// The API layer's encoder takes bytes, so a transaction goes from packed
// record to socket without a single string being built.
//
// # The rule
//
// Everything here aliases. A caller may write these bytes out; it must not
// keep them. That is why the type is named for rendering and why the
// accessors return []byte: a []byte that must not be retained is a much
// louder warning than a string that must not be, because strings are usually
// safe to keep.

// TxnRender is one transaction, ready to serialise, with nothing copied.
//
// Valid only until the next record is rendered or the service lock is
// released, whichever comes first.
type TxnRender struct {
	// ID is written into the caller's scratch, since it is derived from the
	// sequence rather than stored.
	ID []byte

	Kind        string // a constant from ledger, never allocated
	CreatedAt   int64
	Actor       []byte // interned
	Description []byte // arena
	Reference   []byte // arena
	Reverses    []byte // derived, or nil
	ReversedBy  []byte // derived, or nil

	// Postings is a fixed array rather than a slice, so rendering allocates
	// nothing for it. N says how many are used.
	Postings [ledger.MaxInlinePostings]PostingRender
	N        int
}

// PostingRender is one side of a transaction.
type PostingRender struct {
	Account []byte // interned
	Name    []byte // interned
	Amount  ledger.Amount
}

// txnScratch holds the derived IDs a render needs. Caller-owned, so the
// service allocates nothing per record.
type txnScratch struct {
	id         [24]byte
	reverses   [24]byte
	reversedBy [24]byte
}

// EachTxnRender walks transactions affecting an account (or all of them when
// acct is empty), newest first, handing each to fn as bytes.
//
// fn must not retain anything it is given.
// Optional send runs outside the service lock after fn prepares owned bytes.
// With send, records are individually consistent, not a whole-page snapshot.
// The starting sequence bounds exclude new appends; evicted records are skipped.
func (s *Service) EachTxnRender(acct ledger.AccountID, limit int, fn func(*TxnRender) bool, send ...func() bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	var r TxnRender
	var sc txnScratch
	oldest, newest := s.book.Oldest(), s.book.Total()
	ref := s.book.Strings().Find(string(acct))
	n := 0
	for seq := newest; seq > 0 && seq >= oldest; seq-- {
		p := s.book.Packed(seq)
		if p == nil || (acct != "" && !p.Affects(ref)) {
			continue
		}
		s.fillRender(&r, &sc, p)
		if !fn(&r) {
			return
		}
		if len(send) > 0 && !s.sendUnlocked(send[0]) {
			return
		}
		n++
		if limit > 0 && n >= limit {
			return
		}
	}
}

// Always reacquire, including on panic, for the caller's deferred Unlock.
func (s *Service) sendUnlocked(send func() bool) bool {
	s.mu.Unlock()
	defer s.mu.Lock()
	return send()
}

// fillRender populates a render from a packed record. Allocates nothing.
func (s *Service) fillRender(r *TxnRender, sc *txnScratch, p *ledger.PackedTransaction) {
	strs := s.book.Strings()
	arena := s.book.Arena()

	r.ID = ledger.TransactionIDInto(sc.id[:], p.Seq)
	r.Kind = string(p.Kind.Unpack())
	r.CreatedAt = p.CreatedAt
	r.Actor = internedBytes(strs, p.Actor)
	r.Description = arena.Bytes(p.Description)
	r.Reference = arena.Bytes(p.Reference)

	r.Reverses = nil
	if seq := s.book.ReversesSeq(p); seq != 0 {
		r.Reverses = ledger.TransactionIDInto(sc.reverses[:], seq)
	}
	r.ReversedBy = nil
	if seq := s.book.ReversalSeq(p.Seq); seq != 0 {
		r.ReversedBy = ledger.TransactionIDInto(sc.reversedBy[:], seq)
	}

	r.N = 0
	for i := 0; i < ledger.MaxInlinePostings; i++ {
		if p.Accounts[i] == 0 && p.Amounts[i] == 0 {
			continue
		}
		r.Postings[r.N] = PostingRender{
			Account: internedBytes(strs, p.Accounts[i]),
			Name:    s.accountNameBytes(p.Accounts[i]),
			Amount:  p.Amounts[i],
		}
		r.N++
	}
}

// internedBytes returns an interned value's bytes without copying.
//
// The conversion from the stored string to []byte does not allocate here:
// the compiler elides it for a value that is only read, and the intern table
// never mutates a string once stored.
func internedBytes(strs *ledger.Strings, ref ledger.Ref) []byte {
	if ref == 0 {
		return nil
	}
	return strs.LookupBytes(ref)
}

// accountNameBytes resolves an interned account to its display name.
func (s *Service) accountNameBytes(ref ledger.Ref) []byte {
	strs := s.book.Strings()
	if ref == 0 {
		return nil
	}
	if strs.Lookup(ref) == string(ledger.SystemIssuance) {
		return issuanceName
	}
	for i := range s.store.accountsArr {
		a := &s.store.accountsArr[i]
		if a.InUse && a.ID == ref {
			return strs.LookupBytes(a.Name)
		}
	}
	// Unknown account: render the ID, which is what the string path did.
	return strs.LookupBytes(ref)
}

// issuanceName is the display name of the system issuance account, as a
// package-level constant so it costs no allocation per posting.
var issuanceName = []byte("Issuance")
