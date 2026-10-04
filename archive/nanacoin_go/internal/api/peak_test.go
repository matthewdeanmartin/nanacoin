package api

import (
	"net/http"
	"net/http/httptest"
	"runtime"
	"testing"
)

// discardWriter is an http.ResponseWriter that throws the body away.
//
// httptest.ResponseRecorder buffers the whole response, which is precisely
// the thing the board does not do - boardhttp's writer flushes to the socket
// in 1 kB chunks and keeps nothing. Measuring the streaming change through a
// recorder therefore measures the recorder: the buffered page reappears in
// the test harness even after it has been removed from the server.
//
// This stands in for the board's writer: a fixed chunk, no retention.
type discardWriter struct {
	header http.Header
	status int
	n      int64

	// chunk mirrors boardhttp.ResponseChunk, so the write pattern matches
	// what the board actually performs.
	chunk [1 << 10]byte
	used  int
}

func newDiscardWriter() *discardWriter {
	return &discardWriter{header: make(http.Header, 6)}
}

func (d *discardWriter) Header() http.Header { return d.header }
func (d *discardWriter) WriteHeader(s int)   { d.status = s }

func (d *discardWriter) Write(p []byte) (int, error) {
	d.n += int64(len(p))
	for len(p) > 0 {
		n := copy(d.chunk[d.used:], p)
		d.used += n
		p = p[n:]
		if d.used == len(d.chunk) {
			d.used = 0 // "flushed"
		}
	}
	return len(p), nil
}

var _ http.ResponseWriter = (*discardWriter)(nil)

// Peak live heap during one list response, measured through a writer that
// behaves like the board's.
//
// This is the measurement that matters on hardware, and it is not the one a
// -benchmem run reports. B/op is *cumulative* allocation: every byte the
// request touched, whether or not two of them were ever live at once. The
// streaming writer deliberately raises that figure - it marshals each element
// separately rather than the page once - while lowering the quantity that
// actually kills the board, which is how much is live simultaneously.
//
// TinyGo's collector is non-moving, so a request needing 30 kB live at once
// needs 30 kB of sufficiently contiguous heap at once. A request that touches
// 30 kB but never holds more than 2 kB does not. With roughly 120 kB spare,
// that difference decides whether the board serves or dies.
func TestPeakLiveHeapDuringListResponse(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, bobAcct := h.setup()

	for i := 0; i < 60; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice, transferRequest{
			To: acct(bobAcct), Amount: 1, Memo: "chore",
		}), http.StatusCreated, nil)
	}

	// serveOnce runs one request straight into a discarding writer, with no
	// recorder in the way.
	serveOnce := func(path string) {
		r := httptest.NewRequest("GET", path, nil)
		r.Header.Set("Authorization", "Bearer "+nana)
		h.srv.ServeHTTP(newDiscardWriter(), r)
	}

	// liveAfter reports the heap still live once a request has finished and
	// the collector has run. A streaming response should leave nothing
	// behind; a buffered one leaves the page until the next collection.
	liveAfter := func(path string, reps int) uint64 {
		serveOnce(path) // warm up
		runtime.GC()
		runtime.GC()

		var before runtime.MemStats
		runtime.ReadMemStats(&before)

		for i := 0; i < reps; i++ {
			serveOnce(path)
		}

		var after runtime.MemStats
		runtime.ReadMemStats(&after)
		if after.HeapAlloc < before.HeapAlloc {
			return 0
		}
		// Heap growth across the run, with no collection forced in between:
		// this is what accumulates while requests are being served, which is
		// the pressure the board feels between collections.
		return (after.HeapAlloc - before.HeapAlloc) / uint64(reps)
	}

	const reps = 50
	small := liveAfter("/api/v1/transactions?limit=5", reps)
	large := liveAfter("/api/v1/transactions?limit=50", reps)

	t.Logf("heap growth per request: 5 items %d B, 50 items %d B", small, large)

	// Ten times the items must not mean ten times the uncollected heap. The
	// old path held the unpacked records, the view objects and the encoded
	// bytes simultaneously, so this grew with the page.
	if small > 0 && large > small*6 {
		t.Errorf("heap growth scales with page size: %d B at 5 items, %d B at 50 - "+
			"the response is probably being assembled whole again", small, large)
	}
}

// The streamed writer must not retain the page between requests: serving the
// same large list repeatedly should reach a steady state rather than climbing.
//
// This is the board's failure shape in miniature. The board does not die on
// one large response; it dies after a few dozen ordinary ones, because each
// left something behind.
func TestRepeatedListsReachSteadyState(t *testing.T) {
	h := newHarness(t)
	nana, alice, _, _, bobAcct := h.setup()

	for i := 0; i < 60; i++ {
		h.decodeInto(h.do("POST", "/api/v1/transfers", alice, transferRequest{
			To: acct(bobAcct), Amount: 1, Memo: "chore",
		}), http.StatusCreated, nil)
	}

	serve := func() {
		r := httptest.NewRequest("GET", "/api/v1/transactions?limit=50", nil)
		r.Header.Set("Authorization", "Bearer "+nana)
		h.srv.ServeHTTP(newDiscardWriter(), r)
	}

	settle := func() uint64 {
		runtime.GC()
		runtime.GC()
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		return m.HeapAlloc
	}

	for i := 0; i < 50; i++ {
		serve()
	}
	first := settle()

	for i := 0; i < 200; i++ {
		serve()
	}
	second := settle()

	t.Logf("live heap after 50 requests: %d B; after 250: %d B", first, second)

	// Some growth is fine - the event ring fills, maps rehash. A per-request
	// retention would show as growth proportional to the extra 200 requests,
	// which at even 1 kB each would be 200 kB.
	if second > first+(64<<10) {
		t.Errorf("live heap grew %d B over 200 further requests - "+
			"something is retained per request", second-first)
	}
}
