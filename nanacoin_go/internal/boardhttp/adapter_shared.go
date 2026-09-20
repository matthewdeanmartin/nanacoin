// Body-size accounting shared with the TinyGo-only adapter.
//
// This file has no build constraint so that the limits and the decision
// function can be tested on a normal machine. The adapter that uses them only
// builds for the board, which is precisely why the parts that can be tested
// here should be.
package boardhttp

// MaxBodyBytes bounds a request body on the board.
//
// Sized to the largest request the client actually sends, not to a round
// number: a listing with an 80-character title and a 500-character
// description is about 700 bytes of JSON, so 1 kB covers it with room to
// spare. The API layer independently caps bodies at 16 kB, which is the
// desktop's limit; the board cannot afford that.
//
// This was 4 kB and the board died with "fatal error: out of memory" on the
// first request carrying a body. Every byte here is multiplied by Workers.
const MaxBodyBytes = 1 << 10

// ResponseChunk is the streaming write buffer: a response is written to the
// connection in pieces this size rather than assembled whole.
//
// There is no MaxResponseBytes any more, because there is no longer a buffer
// holding a whole response - that ceiling existed only to bound something
// that no longer exists.
//
// Sized deliberately, not minimally. Too small and every response becomes
// many small TCP writes, which on a marginal WiFi link is slower and more
// fragile than a few larger ones. Too large and it is the very thing being
// eliminated. 1 kB covers most responses in one or two writes - status is
// 197 bytes, /me is 149, a fifteen-transaction history is about 3.2 kB - and
// costs Workers x 1 kB, which is a price worth paying for whole-packet
// writes.
const ResponseChunk = 1 << 10

// MaxPageSize caps how many items a list response may contain on the board.
//
// With streaming this no longer bounds a buffer - nothing holds the whole
// response - so it is now about the cost of *building* each item rather than
// of storing them all. encoding/json allocates intermediate buffers per
// value, measured at roughly 18 kB for a fifty-transaction render against
// ~120 kB of spare heap, so a cap still earns its place.
//
// Thirty is a useful page and about half the measured peak.
const MaxPageSize = 30

// Workers is the fixed request concurrency. Router and adapter use the same
// count so each admitted worker has startup-allocated buffers.
const Workers = 4

// BudgetBytes is what the adapter's buffers cost in total: a request buffer
// and a streaming chunk per worker, allocated once at startup and never
// again.
//
// Present so a test can assert it, since the failure mode is a hard
// out-of-memory at runtime rather than a build error.
const BudgetBytes = Workers * (MaxBodyBytes + ResponseChunk)

// BodySizeVerdict says what to do with a request whose Content-Length is
// known.
type BodySizeVerdict uint8

const (
	// BodyOK fits and should be read.
	BodyOK BodySizeVerdict = iota
	// BodyEmpty has nothing to read.
	BodyEmpty
	// BodyTooLarge must be refused with 413 rather than read into a buffer
	// that cannot hold it.
	BodyTooLarge
)

// ClassifyBody decides how to handle a body of the given length.
//
// Separate from the reading so it can be tested: getting this wrong means
// either refusing a legitimate listing or reading past the end of a fixed
// buffer, and neither is something to discover on hardware.
func ClassifyBody(contentLength int64, present bool, bufLen int) BodySizeVerdict {
	if !present || contentLength <= 0 {
		return BodyEmpty
	}
	if contentLength > int64(bufLen) {
		return BodyTooLarge
	}
	return BodyOK
}
