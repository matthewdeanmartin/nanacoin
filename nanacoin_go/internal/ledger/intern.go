package ledger

import "sync"

// Interning: strings that repeat across every record are stored once and
// referred to by a small integer.
//
// Why this exists
//
// A Transaction held five string fields. Each costs 16 bytes of header before
// any characters, and the characters themselves are a separate allocation per
// record - so an ordinary transfer cost about 552 bytes of heap, measured on a
// running service. On a board with roughly 17 kB spare that is 31
// transactions, about three weeks of household use.
//
// The fields concerned are not arbitrary text. Account and user IDs come from
// a set the size of the household, which is two to ten people. Kinds are five
// constants. Interning them turns five string headers into three small
// integers, and stores each distinct value exactly once regardless of how
// many records mention it.
//
// What it does not change
//
// The wire format. The API still sends and receives strings; interning is an
// internal storage decision, and the JSON a client sees is identical. That is
// the point of keeping it here rather than letting compact IDs leak outward -
// a household member reading their history should not be able to tell.
//
// Concurrency
//
// The table only ever grows, and a Ref stays valid for the life of the
// process. Reads are far more common than writes - a new string appears when
// a user or account is created, a handful of times ever - so the mutex is
// uncontended in practice.

// Ref is an interned string. Zero means the empty string, which is what makes
// an unset Reference or Reverses field free rather than 16 bytes.
type Ref uint16

// MaxInterned bounds the table. A household of ten people has about a dozen
// accounts and users, five kinds and a handful of listing references, so this
// is generous by more than an order of magnitude. It exists so that a bug
// cannot turn the table into the unbounded growth interning was meant to
// remove.
const MaxInterned = 512

// Strings is the interning table. One per process; the ledger holds a
// reference so tests can use an isolated table.
type Strings struct {
	mu   sync.RWMutex
	byID []string
	toID map[string]Ref

	// bytesByID is byID as []byte, stored alongside rather than converted on
	// demand.
	//
	// []byte(s) allocates - measured at 24 bytes and one allocation per call,
	// which on a thirty-item list rendering two accounts each is sixty
	// allocations for values written straight to a socket. Interned strings
	// never change once stored, so keeping both forms costs one slice header
	// per distinct value and makes every later read free.
	bytesByID [][]byte
}

// NewStrings returns an empty table.
func NewStrings() *Strings {
	return &Strings{
		// Index 0 is the empty string, so a zero Ref needs no special case
		// at every read.
		byID:      []string{""},
		bytesByID: [][]byte{nil},
		toID:      map[string]Ref{"": 0},
	}
}

// Intern returns the Ref for s, adding it if new.
//
// Returns 0 for the empty string, and also for a string that cannot be added
// because the table is full. That degradation is deliberate: losing a
// description is survivable, and refusing the transaction that carried it is
// not.
func (t *Strings) Intern(s string) Ref {
	if s == "" {
		return 0
	}

	t.mu.RLock()
	ref, ok := t.toID[s]
	t.mu.RUnlock()
	if ok {
		return ref
	}

	t.mu.Lock()
	defer t.mu.Unlock()
	// Check again: another goroutine may have added it between the unlock
	// and the lock.
	if ref, ok := t.toID[s]; ok {
		return ref
	}
	if len(t.byID) >= MaxInterned {
		return 0
	}
	ref = Ref(len(t.byID))
	t.byID = append(t.byID, s)
	t.bytesByID = append(t.bytesByID, []byte(s))
	t.toID[s] = ref
	return ref
}

// Lookup returns the string a Ref stands for, or empty for an unknown one.
func (t *Strings) Lookup(ref Ref) string {
	t.mu.RLock()
	defer t.mu.RUnlock()
	if int(ref) >= len(t.byID) {
		return ""
	}
	return t.byID[ref]
}

// Len is how many distinct strings are interned, for the status endpoint and
// for tests asserting that the table is not growing unexpectedly.
func (t *Strings) Len() int {
	t.mu.RLock()
	defer t.mu.RUnlock()
	return len(t.byID)
}

// Find resolves an existing value without retaining misses from read requests.
func (t *Strings) Find(s string) Ref {
	t.mu.RLock()
	defer t.mu.RUnlock()
	return t.toID[s]
}

// FindBytes is Find for a name held in a buffer rather than a string.
//
// The compiler special-cases map[string(b)] so no string is built for the
// lookup. That matters for the dollar wallets: their names are derived
// ("account-x" -> "account-x-usd") rather than stored, so resolving one via
// Find means concatenating - a heap allocation on a path that runs for every
// user on every /me and every household listing.
func (t *Strings) FindBytes(b []byte) Ref {
	t.mu.RLock()
	defer t.mu.RUnlock()
	return t.toID[string(b)]
}

// LookupBytes returns an interned value's bytes without copying.
//
// Reads the parallel byte form built at intern time. Converting on demand
// with []byte(s) would allocate - 24 bytes and one allocation per call, which
// is the whole cost this exists to remove.
//
// The returned slice aliases the table and must not be modified or retained
// past the caller's immediate use.
func (t *Strings) LookupBytes(ref Ref) []byte {
	t.mu.RLock()
	defer t.mu.RUnlock()
	if int(ref) >= len(t.bytesByID) {
		return nil
	}
	return t.bytesByID[ref]
}
