package core

import "github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"

// Request-owned storage. Returned transactions alias it; reuse only after the
// response is encoded. Traditional callers may omit it and own their result.
//
// # Why there is a second transaction
//
// Every write used to produce exactly one ledger record, so one slot was
// enough. A currency trade produces two - the coin leg and the cash leg - and
// the first version of TakeQuote met that by declaring both as locals with
// slice literals for their postings. That put four objects on the heap per
// trade, on a board where the free heap is measured in single-digit
// kilobytes and the allocator has to find space in a fragmented arena.
//
// The second slot costs 68 bytes for the transaction plus 40 for its postings
// on the 32-bit target, and there are RecordBufferCount of these - 432 bytes
// in total, allocated once at boot. That is the trade this struct exists to
// make: a fixed cost at startup instead of a variable one per request.
type WriteResult struct {
	Transaction ledger.Transaction
	Postings    [ledger.MaxInlinePostings]ledger.Posting

	// The second leg, for writes that produce a pair of records. Untouched by
	// single-record callers, which is every endpoint but the exchange.
	Second         ledger.Transaction
	SecondPostings [ledger.MaxInlinePostings]ledger.Posting
}

//go:noinline
func writeResult(storage []*WriteResult) *WriteResult {
	if len(storage) > 0 && storage[0] != nil {
		return storage[0]
	}
	return new(WriteResult)
}
