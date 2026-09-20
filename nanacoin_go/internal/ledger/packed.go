package ledger

// Packed records are the fixed-size RAM representation. IDs and reversal
// links use sequence numbers; shared identities use interned references.
// Text slots are owned by their records and released when history is evicted.

// MaxInlinePostings is how many postings a packed record holds without
// spilling.
//
// Two, because that is what every transaction this system creates has:
// issuance and retirement move between an account and SystemIssuance,
// transfers and purchases between two accounts, and a reversal mirrors
// whichever of those it undoes. Validate enforces that postings sum to zero,
// which needs at least two.
const MaxInlinePostings = 2

// PackedTransaction is a transaction as held in RAM.
//
// Field order is deliberate: the 64-bit and 32-bit fields come first so the
// compiler needs no padding between them, then the 16-bit references, then
// the single byte. Reordering this will silently grow the struct.
type PackedTransaction struct {
	// Seq is the journal sequence number, which is also the record's
	// identity. The ID a client sees is derived from it rather than stored -
	// see TransactionIDFor - which removes a 16-byte header and a
	// per-record allocation for a value that was random and therefore
	// incompressible.
	Seq uint32

	CreatedAt int64

	// Amounts of the inline postings, in the same order as Accounts.
	Amounts [MaxInlinePostings]Amount

	// Accounts the postings touch, interned.
	Accounts [MaxInlinePostings]Ref

	// Actor is interned: a household has a handful of users and every record
	// names one of them, so the table holds each exactly once.
	Actor Ref

	// Reverses is the original transaction sequence, or zero.
	Reverses uint32

	// Description and Reference go to the text arena, not the intern table.
	//
	// This was the last unbounded growth in the system. Memos are free text
	// and References are listing IDs - both have unbounded cardinality, so
	// interning them consumed a table slot per distinct value that was never
	// reused. Measured: 119 of 512 slots gone after forty listing cycles and
	// sixty transactions, and a full table degrades silently (Intern returns
	// 0, so memos start coming back empty).
	//
	// The fixed arena recycles whole slots on eviction with no per-string
	// allocation. Long text can cause history to evict before the ring fills.
	Description Slot
	Reference   Slot

	Kind PackedKind

	// Currencies of the inline postings, in the same order as Accounts.
	//
	// Two bytes, and they cost nothing: the struct already carried 18 bytes
	// of padding, so these sit in space that was being paid for anyway.
	// Measured before and after - PackedTransaction stays 56 bytes, and the
	// 365-record ring stays 20 KB.
	//
	// Per posting rather than per transaction because a cross-currency trade
	// has legs in different currencies, and a transaction-level tag could not
	// describe one.
	Currencies [MaxInlinePostings]Currency
}

// PackedKind is a transaction kind as a single byte.
type PackedKind uint8

const (
	PackedUnknown PackedKind = iota
	PackedIssue
	PackedRetire
	PackedTransfer
	PackedPurchase
	PackedReversal
)

// packKind converts a kind to its byte form. An unrecognised kind becomes
// PackedUnknown rather than being rejected: a record from a newer format
// should be readable as "something happened" rather than break replay.
func packKind(k TransactionKind) PackedKind {
	switch k {
	case KindIssue:
		return PackedIssue
	case KindRetire:
		return PackedRetire
	case KindTransfer:
		return PackedTransfer
	case KindPurchase:
		return PackedPurchase
	case KindReversal:
		return PackedReversal
	}
	return PackedUnknown
}

// Unpack is the exported form, for callers outside this package that render
// a packed record directly.
func (k PackedKind) Unpack() TransactionKind { return k.unpack() }

func (k PackedKind) unpack() TransactionKind {
	switch k {
	case PackedIssue:
		return KindIssue
	case PackedRetire:
		return KindRetire
	case PackedTransfer:
		return KindTransfer
	case PackedPurchase:
		return KindPurchase
	case PackedReversal:
		return KindReversal
	}
	return ""
}
