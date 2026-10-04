package api

import (
	"io"
	"strconv"
)

// Hand-written JSON encoding, with no reflection and no allocation.
//
// # Why
//
// `tinygo build -print-allocs` on the board binary found 742 allocation sites,
// of which encoding/json accounts for 84 and the reflectlite machinery it
// depends on another 54. That is 138 sites on the path every single request
// takes, and 103 sites across the binary are "size is not constant" - the
// compiler cannot bound them because they depend on runtime input.
//
// No amount of packing the domain objects touches that. The ledger and the
// store are pointer-free arrays now and the request path still churned,
// because the churn was never the data - it was the reflection walking it.
//
// encoding/json is a fine format for machines built after 2000. This board
// has the RAM of one built in the late 1980s, and reflection is the part it
// cannot afford.
//
// # The approach
//
// Every field is appended by name, in order, with the right strconv.Append*
// for its type. The compiler knows at build time that CreatedAt is an int64
// and Status is a string, so nothing needs to discover it at runtime. The
// generated machine code is: write a literal, write a number, write a
// literal, write a number.
//
// # Why there is no buffer
//
// The obvious design is to append into a caller-owned [256]byte and hand the
// filled slice to the writer. That is correct and it is what most embedded
// JSON encoders do.
//
// This does not need it. boardhttp's responseWriter already streams: it
// accumulates into a fixed 1 kB chunk and flushes to the socket when full, so
// it is itself the bounded buffer. Adding another one in front would mean two
// fixed buffers where one does the job, and would reintroduce the ceiling the
// streaming writer exists to remove - a response larger than the scratch
// buffer would fail rather than stream.
//
// So the encoder writes through to the ResponseWriter and carries only a
// 24-byte scratch array for number formatting, which lives in the jsonw
// struct and is reused for every number in the response.
//
// # What this gives up
//
// Generality. Adding a field to a view type means adding a line here, and
// forgetting to is a silently missing field rather than a compile error. That
// is a real cost and the reason the view types and their encoders are kept
// adjacent in the same package: the two files are meant to be read together.
//
// A code generator would remove the footgun and is the right answer if these
// types multiply. At thirteen shapes, hand-written is smaller than the
// generator that would write it.

// jsonw appends JSON to an io.Writer without allocating.
//
// Errors are sticky. A response that fails partway cannot be retracted - the
// status line has already gone out - so the only thing to do is stop writing
// and let the connection close, which is how a truncated body is signalled in
// HTTP/1.1 anyway.
type jsonw struct {
	w   io.Writer
	err error

	// buf accumulates output and is flushed to w when it fills.
	//
	// This is the difference between an encoder that allocates and one that
	// does not, and it was not obvious: the first version wrote each literal
	// and number straight through with io.WriteString(w, ...). That measured
	// *79 allocations per transaction* against encoding/json's 2, because
	// every call through the io.Writer interface boxes its argument.
	//
	// Appending into a fixed array and flushing in blocks makes the whole
	// encode one or two Write calls regardless of how many fields it has, so
	// the interface cost is paid a couple of times rather than eighty.
	//
	// 192 bytes, deliberately small.
	//
	// Bigger is not better here. Go moves a struct to the heap once it is
	// too large to keep on the stack, and a 512-byte buffer crossed that
	// line - which cost one allocation per response, the one thing this file
	// exists to avoid. At 192 the whole encoder lives on the caller's stack
	// and the encode allocates nothing at all.
	//
	// It does not need to hold a whole object. A full buffer flushes and
	// keeps going, so this is a batching window, not a capacity limit: the
	// only thing it changes is how many Write calls a large response costs,
	// and boardhttp's writer batches again behind it into 1 kB chunks.
	// buf is caller-owned storage, not an array inside this struct.
	//
	// An array field makes jsonw big enough that Go moves it to the heap the
	// moment a method takes a pointer to it - which is every method here.
	// That cost one allocation per response. A slice field keeps jsonw small
	// enough to stay on the stack, and the caller supplies the backing array
	// from its own stack frame:
	//
	//	var mem [192]byte
	//	j := newJSONW(w, mem[:])
	//
	// Now nothing escapes and the encode allocates zero.
	buf []byte
	n   int

	// scratch backs number formatting, reused for every number.
	scratch [24]byte

	// needComma tracks whether the next member needs a separator.
	needComma bool
}

// newJSONW returns an encoder writing to w.
//
// Returns a value, not a pointer. A caller doing
//
//	j := newJSONW(w)
//	j.userView(&v)
//
// keeps the whole 512-byte buffer on the stack, so the encode allocates
// nothing at all. Returning *jsonw forced it to the heap - one allocation per
// response, which is one more than this file exists to permit.
func newJSONW(w io.Writer, buf []byte) jsonw { return jsonw{w: w, buf: buf} }

// jsonBufSize is the scratch each caller declares.
//
// A batching window, not a capacity limit: a full buffer flushes and the
// encode continues, so this only decides how many Write calls a large
// response costs. boardhttp's writer batches again behind it into 1 kB
// chunks.
const jsonBufSize = 192

// flush empties the buffer to the writer.
func (j *jsonw) flush() {
	if j.n == 0 || j.err != nil {
		return
	}
	_, j.err = j.w.Write(j.buf[:j.n])
	j.n = 0
}

// room makes sure n bytes fit, flushing if not.
func (j *jsonw) room(n int) bool {
	if j.err != nil {
		return false
	}
	if len(j.buf)-j.n < n {
		j.flush()
		if j.err != nil {
			return false
		}
	}
	return len(j.buf)-j.n >= n
}

// raw appends bytes.
func (j *jsonw) raw(b []byte) {
	if !j.room(len(b)) {
		if j.err == nil && len(b) > len(j.buf) {
			// Longer than the whole buffer: write it straight through
			// rather than failing. Only a very long description reaches
			// this, and it is still one Write rather than many.
			j.flush()
			if j.err == nil {
				_, j.err = j.w.Write(b)
			}
		}
		return
	}
	j.n += copy(j.buf[j.n:], b)
}

// lit appends a string constant.
func (j *jsonw) lit(s string) {
	if !j.room(len(s)) {
		if j.err == nil && len(s) > len(j.buf) {
			j.flush()
			if j.err == nil {
				_, j.err = io.WriteString(j.w, s)
			}
		}
		return
	}
	j.n += copy(j.buf[j.n:], s)
}

// b appends one byte, the common case for punctuation.
func (j *jsonw) b(c byte) {
	if !j.room(1) {
		return
	}
	j.buf[j.n] = c
	j.n++
}

func (j *jsonw) objOpen()  { j.b('{'); j.needComma = false }
func (j *jsonw) objClose() { j.b('}'); j.needComma = true }
func (j *jsonw) arrOpen()  { j.b('['); j.needComma = false }
func (j *jsonw) arrClose() { j.b(']'); j.needComma = true }

// comma writes a separator when one is due.
func (j *jsonw) comma() {
	if j.needComma {
		j.b(',')
	}
	j.needComma = true
}

// key writes a field name. The name is always a compile-time constant from
// this package, so it needs no escaping - a caller passing user input here
// would be a bug, and there is no path that does.
func (j *jsonw) key(name string) {
	j.comma()
	j.b('"')
	j.lit(name)
	j.b('"')
	j.b(':')
	j.needComma = false
}

// --- scalars ----------------------------------------------------------------

func (j *jsonw) int64(v int64) {
	j.comma()
	if !j.room(24) {
		return
	}
	// Format straight into the tail of the buffer: AppendInt writes into the
	// slice it is given, so no scratch copy is needed at all.
	j.n = len(strconv.AppendInt(j.buf[:j.n], v, 10))
}

func (j *jsonw) uint64(v uint64) {
	j.comma()
	if !j.room(24) {
		return
	}
	j.n = len(strconv.AppendUint(j.buf[:j.n], v, 10))
}

func (j *jsonw) int(v int) { j.int64(int64(v)) }

func (j *jsonw) bool(v bool) {
	j.comma()
	if v {
		j.lit("true")
	} else {
		j.lit("false")
	}
}

func (j *jsonw) null() {
	j.comma()
	j.lit("null")
}

// str writes a quoted, escaped JSON string.
//
// Escapes exactly what RFC 8259 requires: the quote, the backslash, and
// everything below 0x20. Bytes at or above 0x20 other than those two are
// passed through unchanged, which is correct for UTF-8 - a multi-byte rune's
// continuation bytes are all >= 0x80 and none of them can be mistaken for a
// character needing escape.
//
// Invalid UTF-8 is passed through rather than replaced. encoding/json
// substitutes U+FFFD; doing that here would mean decoding runes on a board
// that has no reason to, and the only strings this API emits are ones it
// validated on the way in.
func (j *jsonw) str(s string) {
	j.comma()
	j.b('"')

	// Write runs of ordinary bytes in one call rather than byte by byte: a
	// memo with no escapes becomes a single Write instead of 140 of them.
	start := 0
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 0x20 && c != '"' && c != '\\' {
			continue
		}
		if start < i {
			j.lit(s[start:i])
		}
		switch c {
		case '"':
			j.lit(`\"`)
		case '\\':
			j.lit(`\\`)
		case '\n':
			j.lit(`\n`)
		case '\r':
			j.lit(`\r`)
		case '\t':
			j.lit(`\t`)
		case '\b':
			// Backspace and form feed have short escapes in RFC 8259 and
			// encoding/json emits them. The \u00XX form below is valid JSON
			// that parses to the same string, but it is not byte-identical -
			// and byte-identical is the contract these encoders are tested
			// against.
			j.lit(`\b`)
		case '\f':
			j.lit(`\f`)
		default:
			// Other control characters take the \u00XX form.
			j.lit(`\u00`)
			const hex = "0123456789abcdef"
			j.scratch[0] = hex[c>>4]
			j.scratch[1] = hex[c&0xF]
			j.raw(j.scratch[0:2])
		}
		start = i + 1
	}
	if start < len(s) {
		j.lit(s[start:])
	}
	j.b('"')
}

// --- field helpers ----------------------------------------------------------
//
// One call per field, so an encoder reads as a list of fields rather than as
// interleaved key/value plumbing.

func (j *jsonw) fStr(name, v string)           { j.key(name); j.str(v) }
func (j *jsonw) fInt64(name string, v int64)   { j.key(name); j.int64(v) }
func (j *jsonw) fInt(name string, v int)       { j.key(name); j.int(v) }
func (j *jsonw) fUint64(name string, v uint64) { j.key(name); j.uint64(v) }
func (j *jsonw) fBool(name string, v bool)     { j.key(name); j.bool(v) }

// fStrOmit writes the field only when non-empty, matching `omitempty`.
func (j *jsonw) fStrOmit(name, v string) {
	if v != "" {
		j.fStr(name, v)
	}
}

// fInt64Omit writes the field only when non-zero.
func (j *jsonw) fInt64Omit(name string, v int64) {
	if v != 0 {
		j.fInt64(name, v)
	}
}

// fInt64Ptr writes a nullable number: absent when nil, matching a
// `*int64` with `omitempty`.
func (j *jsonw) fInt64Ptr(name string, v *int64) {
	if v != nil {
		j.fInt64(name, *v)
	}
}

// done flushes and reports whether the whole document was written.
func (j *jsonw) done() error {
	j.flush()
	return j.err
}

// --- zero-copy field writers ------------------------------------------------
//
// The encoders above take Go strings, which means a caller holding interned
// or arena-backed text has to materialise it first. On a thirty-transaction
// list that was two arena copies and one ID allocation per record - about
// nine allocations each, for values written straight to output and discarded.
//
// These take bytes instead. Nothing is copied: the arena slice and the
// interned bytes go through the escaper directly.

// strBytes writes a quoted, escaped JSON string from bytes.
//
// Same escaping rules as str; the two are kept in step by
// TestStringEscapingMatchesStdlib, which runs both over the same inputs.
func (j *jsonw) strBytes(s []byte) {
	j.comma()
	j.b('"')

	start := 0
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 0x20 && c != '"' && c != '\\' {
			continue
		}
		if start < i {
			j.raw(s[start:i])
		}
		switch c {
		case '"':
			j.lit(`\"`)
		case '\\':
			j.lit(`\\`)
		case '\n':
			j.lit(`\n`)
		case '\r':
			j.lit(`\r`)
		case '\t':
			j.lit(`\t`)
		case '\b':
			j.lit(`\b`)
		case '\f':
			j.lit(`\f`)
		default:
			j.lit(`\u00`)
			const hex = "0123456789abcdef"
			j.scratch[0] = hex[c>>4]
			j.scratch[1] = hex[c&0xF]
			j.raw(j.scratch[0:2])
		}
		start = i + 1
	}
	if start < len(s) {
		j.raw(s[start:])
	}
	j.b('"')
}

// fStrBytes writes a name/value pair whose value is bytes.
func (j *jsonw) fStrBytes(name string, v []byte) {
	j.key(name)
	j.strBytes(v)
}

// fStrBytesOmit writes the field only when the value is non-empty.
func (j *jsonw) fStrBytesOmit(name string, v []byte) {
	if len(v) > 0 {
		j.fStrBytes(name, v)
	}
}
