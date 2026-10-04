// Package boardhttp serves NanaCoin's existing http.Handler over espradio's
// allocation-free HTTP server.
//
// # Why this exists
//
// The board ran on net/http first, which was the fastest way to get the same
// handler working on both targets. Measured on an ESP32-S3, that board answers
// about twenty requests and then stops accepting TCP connections: no panic, no
// log line, WiFi still up, ARP still resolving, only the listener gone.
// net/http allocates roughly 10 kB per connection and TinyGo's conservative
// collector handles the resulting fragmentation badly, which is exactly what
// espradio's own documentation warns about.
//
// httphi is the answer to that: it allocates its request and response buffers
// and its worker goroutines once, at Configure time, and serving costs nothing
// afterwards. What it does not offer is net/http's interfaces.
//
// So rather than write NanaCoin's twenty-five endpoints a second time - two
// routers to keep in step, with the drift landing on the money-handling code -
// this package adapts one httphi.Exchange into the http.ResponseWriter and
// *http.Request pair the existing handlers already take. internal/api stays
// the single definition of the API on both targets.
//
// # What the adaptation costs
//
// A request body and output chunk use fixed per-worker buffers, as do request/
// response objects and header value slots. Request strings are still copied for
// safe ownership; unusual URLs can allocate in the standard parser. The limits are explicit and small - see MaxBodyBytes
// and the buffer sizes in Config - because a household API sends a handful of
// short JSON fields per request, and a bound that is stated is better than a
// heap that runs out.
package boardhttp

import (
	"errors"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/soypat/lneto/http/httphi"
)

// Observer is notified as a request enters and leaves the adapter, so the
// host can record what was in flight.
//
// Exists because the board cannot report its own death: an out-of-memory
// calls abort(), so no deferred function and no recover ever runs. The only
// way to know what killed it is to have written that down beforehand. See
// cmd/nanacoin-esp32/blackbox.go.
type Observer interface {
	// BeginRequest is called with the request path before the handler runs.
	//
	// Takes bytes rather than a string so the adapter need not copy the
	// target on every request. The slice is only valid for the duration of
	// the call; an implementation that keeps anything must copy it itself.
	BeginRequest(path []byte)
	// EndRequest is called after the response has been written, with whether
	// writing it succeeded.
	EndRequest(ok bool)
}

// Handler adapts an http.Handler onto httphi. One Handler serves every route:
// the wrapped handler does its own routing, which is what keeps internal/api
// authoritative about the API shape on both targets.
type Handler struct {
	h http.Handler

	// obs, when set, is told what is in flight. Optional.
	obs Observer

	// origins is the CORS allow list, needed for requests the wrapped
	// handler never sees - it cannot report a CORS decision for a request it
	// was never given. See stageCORSForRejected.
	origins []string

	// free hands out a fixed number of buffer sets, allocated once in New.
	//
	// A sync.Pool would be wrong here: it allocates a new set whenever all
	// the existing ones are busy, so its memory cost is decided by load
	// rather than by configuration. On a board with a few hundred KB that is
	// how you get "fatal error: out of memory" under a burst, which is
	// exactly what happened with a pool and 4 kB buffers.
	//
	// A buffered channel of pre-made sets gives the opposite property: the
	// cost is fixed at startup, and a request that arrives with none free
	// waits for one instead of allocating.
	free chan *buffers
}

// New wraps an http.Handler for serving over httphi.
//
// allowedOrigins must match what the handler was configured with; it is used
// only for requests the handler never sees, which it cannot answer itself.
//
// Allocates all its buffers here and none afterwards. See BudgetBytes for
// what that costs.
func New(h http.Handler, allowedOrigins []string) *Handler {
	bh := &Handler{
		h:       h,
		origins: allowedOrigins,
		free:    make(chan *buffers, Workers),
	}
	for i := 0; i < Workers; i++ {
		b := &buffers{req: make([]byte, MaxBodyBytes), chunk: make([]byte, ResponseChunk)}
		b.init()
		bh.free <- b
	}
	return bh
}

// Observe registers an Observer. Call before serving.
func (bh *Handler) Observe(o Observer) { bh.obs = o }

type buffers struct {
	req []byte
	// chunk is the streaming write buffer. A response is flushed to the
	// connection in pieces this size, so nothing ever holds a whole one.
	chunk           []byte
	request         http.Request
	uri             url.URL
	body            bodyReader
	requestHeaders  reusableHeaders
	responseHeaders reusableHeaders
	writer          responseWriter
	served          bool
}

// Register attaches the handler to every route NanaCoin serves, and an
// OPTIONS route for every path so that preflights work.
//
// The route lists live in routes.go, which has no build constraint, so they
// can be checked by a test running on a normal machine. See PreflightPaths
// for why GET-only paths need OPTIONS too.
func (bh *Handler) Register(mux *httphi.MuxSlice) {
	for _, pattern := range MethodRoutes() {
		mux.Handle(pattern, bh.serve)
	}
	for _, path := range PreflightPaths() {
		mux.Handle("OPTIONS "+path, bh.serve)
	}
}

// serve is the adaptation itself: build a Request, collect the response, send
// it. Handlers see nothing unusual.
func (bh *Handler) serve(exch *httphi.Exchange) {
	// Wait briefly for a buffer set, then refuse.
	//
	// The old code blocked forever on this channel. That is correct for
	// memory - it never allocates another set - but it is a poor answer for
	// the peer: a request that waits indefinitely is indistinguishable from
	// a board that has crashed, which is exactly the confusion this project
	// spent a long time in. A client cannot tell "busy" from "dead", so it
	// reports "dead".
	//
	// So: wait a little, because the board is slow and a short queue is
	// normal here, then say so explicitly with 503 and Retry-After. A stated
	// refusal is a much better failure than a silent one, and it is what
	// lets the probe distinguish backpressure from a crash.
	var bufs *buffers
	select {
	case bufs = <-bh.free:
	default:
		// None free right now. Give it a moment before refusing - the
		// common case is another worker finishing mid-request, and
		// refusing instantly would turn ordinary queuing into errors.
		timer := time.NewTimer(BusyWait)
		select {
		case bufs = <-bh.free:
			timer.Stop()
		case <-timer.C:
			bh.refuseBusy(exch)
			return
		}
	}

	// A method defer avoids heap-escaping closure captures in TinyGo.
	// Cleanup runs on every exit, before this worker becomes available again.
	defer bh.finishRequest(bufs)

	if bh.obs != nil {
		// Before the request is even parsed: a body too large to read is
		// still a request that could be the last thing this board does.
		bh.obs.BeginRequest(exch.RequestTarget())
	}

	req, err := bufs.buildRequest(exch)
	if err != nil {
		// A request that cannot be represented is answered directly: the
		// wrapped handler never sees it, so it cannot report it.
		//
		// The CORS headers have to be staged even here. Without them the
		// browser cannot read the error, reports a missing
		// Access-Control-Allow-Origin, and the real 400 or 413 never reaches
		// the user - so a request too large to parse looks like a CORS
		// misconfiguration.
		status := http.StatusBadRequest
		if errors.Is(err, errBodyTooLarge) {
			status = http.StatusRequestEntityTooLarge
		}
		bh.stageCORSForRejected(exch)
		exch.RespondString(status, "application/json; charset=utf-8",
			`{"error":"bad_request","message":"the board could not read this request"}`)
		return
	}

	w := &bufs.writer
	w.exch = exch
	bh.h.ServeHTTP(w, req)

	// A handler that wrote nothing at all - and there are none, but a future
	// one might - still owes the peer a response.
	err = w.finish()
	if err != nil {
		println("boardhttp: response failed:", err.Error())
	}
	bufs.served = err == nil
}

func (bh *Handler) finishRequest(b *buffers) {
	served := b.served
	b.release()
	bh.free <- b
	if bh.obs != nil {
		bh.obs.EndRequest(served)
	}
}

// BusyWait is how long a request waits for a free buffer set before being
// refused.
//
// Generous by modern standards and deliberately so. This board answers a
// simple read in tens of milliseconds and a write in a few hundred, on a
// link several floors from its access point; a client that gives up after
// two seconds is applying a desktop's expectations to hardware with a 1980s
// memory budget. Two seconds of queuing is a much better outcome than a
// refusal.
const BusyWait = 2 * time.Second

// refuseBusy answers a request the board has no capacity for.
//
// 503 with Retry-After, not a dropped connection. A closed socket is
// indistinguishable from a crash from the outside, which is how backpressure
// and failure got conflated for most of this project's debugging.
//
// The CORS headers are staged first for the same reason they are everywhere
// else: without them a browser cannot read the 503 and reports a CORS error
// instead, sending whoever is debugging it somewhere useless.
func (bh *Handler) refuseBusy(exch *httphi.Exchange) {
	bh.stageCORSForRejected(exch)
	exch.StageHeader("Retry-After", "1")
	exch.RespondString(http.StatusServiceUnavailable,
		"application/json; charset=utf-8",
		`{"error":"busy","message":"the board is serving other requests; retry shortly"}`)
	println("boardhttp: refused a request, all workers busy")
}

var errBodyTooLarge = errors.New("request body exceeds the board limit")

// corsHeaders are the fields a browser must receive to read a response at all,
// staged before anything else.
var corsHeaders = []string{
	"Access-Control-Allow-Origin",
	"Vary",
	"Access-Control-Allow-Methods",
	"Access-Control-Allow-Headers",
	"Access-Control-Expose-Headers",
	"Access-Control-Max-Age",
	// Staged with the CORS fields rather than in the general loop, because
	// this is the reading that survives a crash: once the board is out of
	// memory it cannot serve /logs, so the last successful response's headers
	// are the final diagnostic. Losing it to a full staging buffer would lose
	// exactly the evidence it exists to carry.
	"X-Nanacoin-Health",
}

// skipHeader reports fields the general loop must not stage: the two httphi
// writes itself, and the CORS fields already staged.
func skipHeader(key string) bool {
	if strings.EqualFold(key, "Content-Type") || strings.EqualFold(key, "Content-Length") {
		return true
	}
	for _, h := range corsHeaders {
		if strings.EqualFold(key, h) {
			return true
		}
	}
	return false
}

// stageCORSFrom stages the CORS and diagnostic fields a handler produced,
// returning how many did not fit.
func stageCORSFrom(exch *httphi.Exchange, header http.Header) int {
	if header == nil {
		return 0
	}
	dropped := 0
	for _, key := range corsHeaders {
		for _, v := range header.Values(key) {
			if !exch.StageHeader(key, v) {
				dropped++
			}
		}
	}
	return dropped
}

// stageCORSForRejected answers a request the wrapped handler never saw -
// one the board could not even parse.
//
// It has to consult the allow list rather than echo the request's Origin:
// reflecting an arbitrary origin would hand any page on the internet
// permission to read the board's responses, which is the one thing the CORS
// policy exists to prevent. So the origin is checked against the same list
// the handler uses, and only then echoed.
//
// Without this the browser cannot read the 400 or 413, reports a missing
// Access-Control-Allow-Origin, and sends whoever is debugging it after a CORS
// problem that does not exist.
func (bh *Handler) stageCORSForRejected(exch *httphi.Exchange) {
	origin := string(exch.RequestHeader("Origin"))
	if origin == "" {
		return
	}
	for _, allowed := range bh.origins {
		if allowed == origin {
			exch.StageHeader("Access-Control-Allow-Origin", origin)
			exch.StageHeader("Vary", "Origin")
			return
		}
	}
}

// buildRequest turns an exchange into an *http.Request backed by fixed
// buffers. The body is read fully up front - it is at most MaxBodyBytes of
// JSON - which keeps the handler from holding the connection while it decodes.
func (b *buffers) buildRequest(exch *httphi.Exchange) (*http.Request, error) {
	target := string(exch.RequestTarget())
	if err := parseTarget(&b.uri, target); err != nil {
		return nil, err
	}
	body, err := readBody(exch, b.req)
	if err != nil {
		return nil, err
	}
	b.body.Reset(body)
	r := &b.request
	r.Method = string(exch.RequestHeaderV1Raw().Method())
	r.URL = &b.uri
	r.Proto = "HTTP/1.1"
	r.ProtoMajor, r.ProtoMinor = 1, 1
	r.Header = b.requestHeaders.header
	r.Body = &b.body
	r.ContentLength = int64(len(body))
	r.RequestURI = target
	r.Host = string(exch.RequestHeader("Host"))
	for _, key := range requestHeaderKeys {
		if v := exch.RequestHeader(key); len(v) > 0 {
			values := r.Header[key][:1]
			values[0] = string(v)
			r.Header[key] = values
		}
	}
	return r, nil
}

// readBody reads the request body into buf, refusing anything larger.
func readBody(exch *httphi.Exchange, buf []byte) ([]byte, error) {
	length, present, err := exch.RequestContentLength()
	if err != nil {
		return nil, err
	}
	switch ClassifyBody(length, present, len(buf)) {
	case BodyEmpty:
		return nil, nil
	case BodyTooLarge:
		return nil, errBodyTooLarge
	}

	var n int
	for n < int(length) {
		read, err := exch.ReadBody(buf[n:int(length)])
		n += read
		if err != nil {
			if errors.Is(err, io.EOF) {
				break
			}
			return nil, err
		}
		if read == 0 {
			// No progress and no error: the peer stopped sending. The
			// connection deadline is what limits how long this can last.
			break
		}
	}
	if n != int(length) {
		return nil, io.ErrUnexpectedEOF
	}
	return buf[:n], nil
}

// responseWriter streams a handler's response to the connection.
//
// Nothing here holds a whole response. Writes accumulate in a fixed chunk and
// flush when it fills, so the memory cost is the chunk - not the response -
// however long the response turns out to be. That is the point: buffering
// whole responses is what put the board 4x over its response size in
// allocations and eventually out of memory.
//
// The cost of streaming is that Content-Length cannot be known when the
// header goes out. The response is framed by connection close instead, which
// is valid HTTP/1.1 (RFC 9112 section 6.3) and costs nothing here because
// httphi already sends Connection: close - it serves one exchange per
// connection regardless.
type responseWriter struct {
	exch  *httphi.Exchange
	chunk []byte
	n     int

	header      http.Header
	status      int
	wroteHeader bool

	// err is sticky. Once a write to the connection fails the response is
	// unrecoverable, and every later write has to stop rather than pretend.
	err error
}

func (w *responseWriter) Header() http.Header { return w.header }

// WriteHeader stages the handler's headers and sends the status line. Only the
// first call reaches the wire, as in net/http.
func (w *responseWriter) WriteHeader(status int) {
	if w.wroteHeader || w.err != nil {
		return
	}
	w.wroteHeader = true
	w.status = status

	// CORS first and explicitly. A staged field that does not fit is dropped,
	// and Go map iteration order is random - so relying on the loop below to
	// reach these would mean a full staging buffer sometimes drops the one
	// header without which the browser cannot read the response at all.
	dropped := stageCORSFrom(w.exch, w.header)

	if ct := w.header.Get("Content-Type"); ct != "" {
		if !w.exch.StageHeader("Content-Type", ct) {
			dropped++
		}
	}
	for key, values := range w.header {
		if skipHeader(key) {
			continue
		}
		for _, v := range values {
			if !w.exch.StageHeader(key, v) {
				dropped++
			}
		}
	}

	// Say so. A silently dropped header is how the health reading vanished
	// from seven consecutive responses - the seven leading up to a crash,
	// when it was the only diagnostic that mattered. StageHeader reports
	// whether the field fit, and ignoring that return is what made the
	// failure invisible.
	if dropped > 0 {
		println("boardhttp: dropped", dropped, "response headers - raise responseHeaderBuffer")
	}

	// No Content-Length: the length is not known yet, and will not be until
	// the handler has finished writing. Connection: close is what frames the
	// body instead.
	w.exch.StageHeader("Connection", "close")
	w.exch.StageStatus(status)

	if _, err := w.exch.FlushHeader(); err != nil {
		w.err = err
	}
}

func (w *responseWriter) Write(p []byte) (int, error) {
	if w.err != nil {
		return 0, w.err
	}
	if !w.wroteHeader {
		// A handler that writes without setting a status has sent 200, as in
		// net/http.
		w.WriteHeader(http.StatusOK)
		if w.err != nil {
			return 0, w.err
		}
	}

	written := 0
	for len(p) > 0 {
		// Copy into the chunk, flushing whenever it fills. A write larger
		// than the chunk is therefore split across several, rather than
		// needing a buffer its own size.
		n := copy(w.chunk[w.n:], p)
		w.n += n
		p = p[n:]
		written += n

		if w.n == len(w.chunk) {
			if err := w.flush(); err != nil {
				return written, err
			}
		}
	}
	return written, nil
}

// io.WriteString otherwise converts every streamed JSON punctuation fragment
// to a temporary []byte through the ResponseWriter interface.
func (w *responseWriter) WriteString(s string) (int, error) {
	if w.err != nil {
		return 0, w.err
	}
	if !w.wroteHeader {
		w.WriteHeader(http.StatusOK)
	}
	if w.err != nil {
		return 0, w.err
	}
	written := 0
	for len(s) > 0 {
		n := copy(w.chunk[w.n:], s)
		w.n += n
		written += n
		s = s[n:]
		if w.n == len(w.chunk) {
			if err := w.flush(); err != nil {
				return written, err
			}
		}
	}
	return written, nil
}

// flush sends what is in the chunk and empties it.
func (w *responseWriter) flush() error {
	if w.n == 0 || w.err != nil {
		return w.err
	}
	if _, err := w.exch.WriteBody(w.chunk[:w.n]); err != nil {
		w.err = err
		return err
	}
	w.n = 0
	return nil
}

// Flush implements http.Flusher, so a handler that streams deliberately gets
// what it asked for rather than silently buffering to the end.
func (w *responseWriter) Flush() {
	_ = w.flush()
}

// finish sends whatever is left, and the header if the handler wrote nothing.
func (w *responseWriter) finish() error {
	if !w.wroteHeader {
		// No handler here does this, but a response with no body still needs
		// a status line or the peer waits for one.
		w.WriteHeader(http.StatusOK)
	}
	if err := w.flush(); err != nil {
		return err
	}
	return w.err
}

var (
	_ http.ResponseWriter = (*responseWriter)(nil)
	_ http.Flusher        = (*responseWriter)(nil)
)
