package api

import (
	"bytes"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

// The server's own per-request cost, with the test harness subtracted.
//
// This exists to correct a measurement error that shaped several decisions.
// Benchmarks built each request with httptest.NewRequest and
// httptest.NewRecorder, which together allocate about 5.5 kB - so a "6,917
// byte request" was mostly scaffolding, and the server's real share is far
// smaller. Conclusions drawn from the larger figure, including "about fifteen
// requests fit in the free heap", were wrong.
//
// The board does not use httptest at all: internal/boardhttp builds an
// http.Request from fixed buffers and writes the response straight to the
// connection. These benchmarks reuse one request and one recorder to
// approximate that, so what they report is the handler chain rather than the
// harness.
//
// Run with:
//
//	go test ./internal/api/ -run XXX -bench BenchmarkServerCost -benchmem
func BenchmarkServerCost(b *testing.B) {
	h := newHarness(b)
	_, alice, _, _, bobAcct := h.setup()

	// The recorder is reset rather than reallocated, and the request reused,
	// so neither shows up in the numbers.
	reuse := func(method, target, token, body string) (*http.Request, *reusableWriter) {
		u, err := url.ParseRequestURI(target)
		if err != nil {
			b.Fatal(err)
		}
		r := &http.Request{
			Method: method, URL: u, Proto: "HTTP/1.1",
			ProtoMajor: 1, ProtoMinor: 1,
			Header:     make(http.Header, 4),
			Host:       "board",
			RequestURI: target,
		}
		if token != "" {
			r.Header.Set("Authorization", "Bearer "+token)
		}
		if body != "" {
			r.Header.Set("Content-Type", "application/json")
		}
		return r, &reusableWriter{header: make(http.Header, 6)}
	}

	b.Run("read-me", func(b *testing.B) {
		r, w := reuse("GET", "/api/v1/me", alice, "")
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			w.reset()
			r.Body = http.NoBody
			h.srv.ServeHTTP(w, r)
		}
	})

	b.Run("read-status", func(b *testing.B) {
		r, w := reuse("GET", "/api/v1/status", "", "")
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			w.reset()
			r.Body = http.NoBody
			h.srv.ServeHTTP(w, r)
		}
	})

	b.Run("read-listings", func(b *testing.B) {
		r, w := reuse("GET", "/api/v1/listings", alice, "")
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			w.reset()
			r.Body = http.NoBody
			h.srv.ServeHTTP(w, r)
		}
	})

	b.Run("write-transfer", func(b *testing.B) {
		body := `{"to":"` + bobAcct + `","amount":1,"memo":"chores"}`
		r, w := reuse("POST", "/api/v1/transfers", alice, body)
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			w.reset()
			r.Body = io.NopCloser(strings.NewReader(body))
			r.ContentLength = int64(len(body))
			h.srv.ServeHTTP(w, r)
		}
	})

	// For contrast: the same read built the way the other benchmarks did, so
	// the harness overhead is visible rather than assumed.
	b.Run("read-me-via-httptest", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			r := httptest.NewRequest("GET", "/api/v1/me", nil)
			r.Header.Set("Authorization", "Bearer "+alice)
			h.srv.ServeHTTP(httptest.NewRecorder(), r)
		}
	})
}

// reusableWriter is an http.ResponseWriter that can be reset between
// iterations, so a benchmark measures the handler rather than the recorder.
// It discards the body, which is what the board's streaming writer does to
// the connection anyway.
type reusableWriter struct {
	header http.Header
	status int
	body   bytes.Buffer
}

func (w *reusableWriter) Header() http.Header { return w.header }
func (w *reusableWriter) WriteHeader(s int)   { w.status = s }
func (w *reusableWriter) Write(p []byte) (int, error) {
	return w.body.Write(p)
}

func (w *reusableWriter) reset() {
	for k := range w.header {
		delete(w.header, k)
	}
	w.status = 0
	w.body.Reset()
}
