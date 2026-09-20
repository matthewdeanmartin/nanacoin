package api

import (
	"io"
	"net/http"
)

// Incremental JSON output.
//
// # Why this exists
//
// json.Encoder streams its *writes*, not its *encoding*: Encode marshals the
// whole value into an internal buffer and then writes that buffer out. So
// handing it a fully built []transactionView held two copies of the page at
// once - the view objects and the encoded bytes - on top of the unpacked
// ledger records the views were built from.
//
// Measured on the board, a thirty-item history therefore peaked at roughly
// four times what the response itself is worth. That peak, not any leak, is
// what met a fragmented heap and produced "fatal error: out of memory".
//
// The writer below emits a list one element at a time: unpack a record,
// encode it, let it go, take the next. One item is live at a time instead of
// a page of them, so the peak stops scaling with the page size and the
// MaxPageSize cap stops being load-bearing.
//
// # What it does not do
//
// It is not a general JSON serialiser and should not grow into one. Handlers
// that return a single object still use writeJSON, because one object has no
// peak worth avoiding. This is only for the list responses, which are the
// only place the count is unbounded by the shape of the API.

// streamWriter emits a JSON object whose values may be incrementally produced
// arrays. It writes straight through to the ResponseWriter - on the board that
// is boardhttp's chunked responseWriter, so bytes reach the connection as they
// are produced. Service-backed lists first stage one encoded record in a
// fixed recordBuffer so socket writes happen outside the service lock.
//
// Errors are sticky and checked once at the end. A response that fails partway
// cannot be retracted - the status line has already gone out - so the only
// thing to do is stop writing and let the connection close, which is how a
// truncated body is signalled in HTTP/1.1 anyway.
type streamWriter struct {
	w   io.Writer
	err error

	// needComma tracks whether the next element in the current array or
	// object needs a separator before it.
	needComma bool
}

// beginStream starts a JSON response, writing the status and headers first.
//
// The caller must call end() to close the object. The header is sent before
// any body byte, which means a handler that fails partway cannot change the
// status - so everything that can fail (authorization, lookups) must happen
// before this is called.
func beginStream(w http.ResponseWriter, status int) *streamWriter {
	setHeader(w.Header(), "Content-Type", "application/json; charset=utf-8")
	setHeader(w.Header(), "Cache-Control", "no-store")
	w.WriteHeader(status)

	sw := &streamWriter{w: w}
	sw.write("{")
	return sw
}

func (s *streamWriter) write(str string) {
	if s.err != nil {
		return
	}
	_, s.err = io.WriteString(s.w, str)
}

// key writes a field name, with a separator if it is not the first.
func (s *streamWriter) key(name string) {
	if s.needComma {
		s.write(",")
	}
	// Field names are compile-time constants from this package, so they need
	// no escaping and strconv.Quote's allocation can go.
	s.write(`"`)
	s.write(name)
	s.write(`":`)
	s.needComma = true
}

// fieldStr, fieldInt64 and fieldUint64 write a complete name/value pair for
// the small scalar fields a list response carries.
func (s *streamWriter) fieldStr(name, v string) {
	s.key(name)
	s.emitStr(v)
}

func (s *streamWriter) fieldInt64(name string, v int64) {
	s.key(name)
	s.emitInt64(v)
}

func (s *streamWriter) fieldUint64(name string, v uint64) {
	s.key(name)
	s.emitUint64(v)
}

// value encodes one value with a hand-written encoder.
//
// This was json.Marshal, which meant every element of every list response
// walked encoding/json's reflection - the single hottest reflective path in
// the program, since list endpoints are what a page load actually fetches.
// The emit* methods below replace it with typed encoders that allocate
// nothing; see jsonwriter.go.
func (s *streamWriter) value(fn func(*jsonw)) {
	if s.err != nil {
		return
	}
	var mem [jsonBufSize]byte
	j := newJSONW(s.w, mem[:])
	fn(&j)
	if err := j.done(); err != nil {
		s.err = err
	}
}

// emitStr, emitInt64 and emitRaw write the small scalar fields a list
// response carries alongside its array.
func (s *streamWriter) emitStr(v string) {
	s.value(func(j *jsonw) { j.needComma = false; j.str(v) })
}

func (s *streamWriter) emitInt64(v int64) {
	s.value(func(j *jsonw) { j.needComma = false; j.int64(v) })
}

func (s *streamWriter) emitUint64(v uint64) {
	s.value(func(j *jsonw) { j.needComma = false; j.uint64(v) })
}

// array is the whole point: it calls emit with a sink that encodes one element
// at a time, so the caller never builds the slice.
//
// The sink takes an encoder function rather than an `any`, which is what
// keeps the element path free of reflection: the caller names the encoder for
// the type it holds, and the compiler checks it.
//
// The sink is only valid for the duration of the call.
func (s *streamWriter) array(name string, emit func(add func(fn func(*jsonw)))) {
	s.key(name)
	s.write("[")

	first := true
	add := func(fn func(*jsonw)) {
		if !first {
			s.write(",")
		}
		first = false
		s.value(fn)
	}
	emit(add)

	s.write("]")
}

// end closes the object and reports whether the whole response was written.
//
// A non-nil error here means the peer got a truncated body. There is nothing
// to do about it at this point but say so, which the board prints to the
// console - a silent truncation is how a header going missing went unnoticed
// for a long time.
func (s *streamWriter) end() error {
	s.write("}")
	return s.err
}
