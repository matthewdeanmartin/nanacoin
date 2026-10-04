package storage

// Journal is the persistence contract. It is deliberately tiny: an append, a
// replay, and a close. Every operation NanaCoin performs is one Append; there
// is no update, no delete, and no random read.
//
// A backend must guarantee that after Append returns nil, the record survives
// power loss, and that a record whose commit marker never landed is invisible
// to Replay.
type Journal interface {
	// Append durably writes one record, assigning it the next sequence
	// number. It returns the assigned sequence.
	Append(typ RecordType, payload []byte) (uint64, error)

	// Replay calls fn for each committed record in sequence order, stopping
	// at the first incomplete or invalid tail record. An error from fn
	// aborts and is returned.
	Replay(fn func(*Record) error) error

	// Size reports bytes used and bytes available, for the status endpoint.
	// A household should be able to see the journal filling years before it
	// does.
	Size() (used, capacity int64)

	Close() error
}

// DiscardJournal is an optional volatile backend that needs only payload size.
// Persistent backends must not implement this capability: their Append must
// receive the actual encoded event. This avoids serializing bytes into oblivion.
type DiscardJournal interface {
	Journal
	DiscardsPayload() bool
	AppendDiscarded(RecordType, int) (uint64, error)
}
