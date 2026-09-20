package boardhttp

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/api"
	"testing"
)

// Body classification decides between reading a request, refusing it, and
// treating it as empty. Getting it wrong means either refusing a legitimate
// listing or reading past a fixed buffer, so it is tested here rather than
// discovered on hardware.
func TestClassifyBody(t *testing.T) {
	const buf = 4096

	for _, tc := range []struct {
		name    string
		length  int64
		present bool
		want    BodySizeVerdict
	}{
		{"no content-length", 0, false, BodyEmpty},
		{"zero length", 0, true, BodyEmpty},
		{"negative length", -1, true, BodyEmpty},
		{"a typical listing", 120, true, BodyOK},
		{"exactly the buffer", buf, true, BodyOK},
		{"one byte over", buf + 1, true, BodyTooLarge},
		{"absurd", 1 << 30, true, BodyTooLarge},
		// A Content-Length header is attacker-controlled, so a claim of a
		// huge body must be refused on the claim alone, before any read.
		{"present but huge with no body", 1 << 40, true, BodyTooLarge},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if got := ClassifyBody(tc.length, tc.present, buf); got != tc.want {
				t.Errorf("ClassifyBody(%d, %v, %d) = %d, want %d",
					tc.length, tc.present, buf, got, tc.want)
			}
		})
	}
}

// The board's body limit has to fit the largest request the client actually
// sends, or a legitimate listing is refused with a 413 the user cannot act on.
func TestBodyLimitFitsTheLargestRealRequest(t *testing.T) {
	// A listing at the API's own field limits: 80-character title,
	// 500-character description, plus JSON structure and a price.
	const largestListing = 80 + 500 + 120

	if MaxBodyBytes < largestListing {
		t.Errorf("MaxBodyBytes is %d, too small for the largest listing (~%d bytes)",
			MaxBodyBytes, largestListing)
	}
}

// The memory budget is the thing that actually killed the board: 4 kB buffers
// times an unbounded sync.Pool produced "fatal error: out of memory" on the
// first request with a body, with nothing in the console to explain it.
//
// The budget is now fixed at startup, so it can be asserted. The ceiling is
// deliberately low: the adapter shares the heap with httphi's router memory,
// the TCP pool, and the WiFi blob's arena, and it is the newest and least
// essential of those.
func TestBufferBudgetStaysSmall(t *testing.T) {
	const ceiling = 8 << 10

	if BudgetBytes > ceiling {
		t.Errorf("the adapter's buffers total %d bytes, over the %d ceiling - "+
			"this is what caused an out-of-memory on the board; shrink "+
			"MaxBodyBytes, ResponseBufferStart or Workers",
			BudgetBytes, ceiling)
	}
}

// Workers multiplies every buffer, so it is worth stating why it is small.
func TestWorkersStaysSmall(t *testing.T) {
	if Workers < 1 {
		t.Fatal("Workers must be at least 1 or nothing can be served")
	}
	if Workers > 4 {
		t.Errorf("Workers is %d: each one costs MaxBodyBytes + ResponseBufferStart, "+
			"and the configured budget covers four workers", Workers)
	}
}

// The streaming chunk is a deliberate compromise and the numbers should say
// so: small enough that it is not the buffering it replaced, large enough
// that a response is not split into many tiny TCP writes on a marginal link.
func TestResponseChunkIsSensiblySized(t *testing.T) {
	// Below this, a 3 kB history page becomes a dozen-plus writes, which on
	// this board's WiFi is slower and more fragile than a few larger ones.
	const tooSmall = 512
	// Above this it starts to be the whole-response buffer that caused the
	// out-of-memory in the first place.
	const tooLarge = 4 << 10

	if ResponseChunk < tooSmall {
		t.Errorf("ResponseChunk is %d, below the %d floor - responses would "+
			"be split into too many small writes", ResponseChunk, tooSmall)
	}
	if ResponseChunk > tooLarge {
		t.Errorf("ResponseChunk is %d, above the %d ceiling - at that size it "+
			"is the whole-response buffer this design removed", ResponseChunk, tooLarge)
	}
}

// The largest single response must not need many chunks, or the win from
// streaming is paid back in round trips.
func TestChunkCoversATypicalResponseInFewWrites(t *testing.T) {
	// Measured: a 30-transaction history is about 6.4 kB (4.5 kB at fifty,
	// 3.2 kB at fifteen).
	const largestPage = MaxPageSize * 215

	if writes := (largestPage + ResponseChunk - 1) / ResponseChunk; writes > 8 {
		t.Errorf("the largest page would take %d writes of %d bytes; "+
			"raise ResponseChunk or lower MaxPageSize", writes, ResponseChunk)
	}
}

// Four workers have adapter and record-snapshot storage fixed at startup.
func TestFixedRenderBudget(t *testing.T) {
	if Workers != api.RecordBufferCount {
		t.Fatal("worker/snapshot pool mismatch")
	}
	if BudgetBytes+api.RecordBufferCount*api.RecordBufferBytes > 18<<10 {
		t.Fatal("adapter and snapshot buffers exceeded 18 KiB (includes maximum escaped purchase responses)")
	}
}
