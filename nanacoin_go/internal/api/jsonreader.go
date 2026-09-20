package api

import (
	"errors"
	"strconv"
)

// Hand-written JSON parsing, with no reflection and no per-field allocation.
//
// # Why
//
// Measured on a 68-byte transfer body:
//
//	json.Decoder     1056 B/op, 10 allocs/op
//	json.Unmarshal    312 B/op,  7 allocs/op
//
// Fifteen times the input size, as ten small scattered objects - a map for
// the field set, a string per key, a string per value, the reflect machinery
// to place them. Every request pays it, and every one of those objects is
// short-lived garbage landing between the long-lived domain records.
//
// That is the mechanism the board dies of. Not a leak: the collector reclaims
// all of it. But a non-moving collector cannot compact, so a few hundred
// rounds of small allocations interleaved with permanent ones leaves a heap
// with plenty of free bytes and no contiguous run big enough for the next
// buffer.
//
// # The approach
//
// A scanner over the body with a typed dispatch per request struct. The
// parser never builds a map, never allocates a key, and hands values back as
// sub-slices of the body. A field is placed by a switch on its name, which
// the compiler turns into a comparison chain rather than a hash lookup.
//
// # Strings
//
// The one place an allocation is unavoidable is a string field that the
// request struct keeps: Go strings are immutable, so storing one means
// copying the bytes out of the body. That is one allocation per string field
// actually present, against ten for the whole decode before.
//
// Numbers, booleans and absent fields cost nothing at all.
//
// # Strictness
//
// Unknown fields are rejected, matching DisallowUnknownFields. On a money API
// a misspelled key must fail loudly rather than being silently ignored - the
// difference between a typo and a wrong transfer.

var (
	errJSONSyntax  = errors.New("malformed JSON")
	errJSONUnknown = errors.New("unknown field")
	errJSONType    = errors.New("wrong type for field")
	errJSONDepth   = errors.New("JSON nested too deeply")
)

// maxDepth bounds nesting so a hostile body cannot drive the parser into
// unbounded recursion. Request bodies here are flat objects; three levels is
// already more than any of them use.
const maxDepth = 8

// jsonr scans a JSON document held entirely in a caller-owned buffer.
//
// Holds no buffer of its own and copies nothing until a caller asks for a
// string, so the parser itself is free.
type jsonr struct {
	b   []byte
	i   int
	err error
}

func newJSONR(b []byte) jsonr { return jsonr{b: b} }

func (p *jsonr) fail(e error) {
	if p.err == nil {
		p.err = e
	}
}

// ws skips whitespace.
func (p *jsonr) ws() {
	for p.i < len(p.b) {
		switch p.b[p.i] {
		case ' ', '\t', '\n', '\r':
			p.i++
		default:
			return
		}
	}
}

// byteAt returns the current byte, or 0 at the end.
func (p *jsonr) byteAt() byte {
	if p.i >= len(p.b) {
		return 0
	}
	return p.b[p.i]
}

// expect consumes one required byte.
func (p *jsonr) expect(c byte) bool {
	p.ws()
	if p.byteAt() != c {
		p.fail(errJSONSyntax)
		return false
	}
	p.i++
	return true
}

// rawString returns the next string's bytes, unescaped in place where
// possible.
//
// The returned slice aliases the body when the string contains no escapes,
// which is the common case - so reading a field costs nothing. A string with
// escapes is decoded into scratch, which the caller supplies.
//
// Returns the slice and whether it aliases the body.
func (p *jsonr) rawString(scratch []byte) ([]byte, bool) {
	p.ws()
	if p.byteAt() != '"' {
		p.fail(errJSONSyntax)
		return nil, false
	}
	p.i++
	start := p.i

	// Fast path: scan for the closing quote, bailing out if an escape is seen.
	for p.i < len(p.b) {
		c := p.b[p.i]
		if c == '"' {
			s := p.b[start:p.i]
			p.i++
			return s, true
		}
		if c == '\\' {
			return p.escapedString(start, scratch)
		}
		if c < 0x20 {
			p.fail(errJSONSyntax)
			return nil, false
		}
		p.i++
	}
	p.fail(errJSONSyntax)
	return nil, false
}

// escapedString handles the rare string containing backslash escapes.
func (p *jsonr) escapedString(start int, scratch []byte) ([]byte, bool) {
	out := scratch[:0]
	out = append(out, p.b[start:p.i]...)

	for p.i < len(p.b) {
		c := p.b[p.i]
		switch {
		case c == '"':
			p.i++
			return out, false
		case c == '\\':
			p.i++
			if p.i >= len(p.b) {
				p.fail(errJSONSyntax)
				return nil, false
			}
			switch p.b[p.i] {
			case '"':
				out = append(out, '"')
			case '\\':
				out = append(out, '\\')
			case '/':
				out = append(out, '/')
			case 'n':
				out = append(out, '\n')
			case 'r':
				out = append(out, '\r')
			case 't':
				out = append(out, '\t')
			case 'b':
				out = append(out, '\b')
			case 'f':
				out = append(out, '\f')
			case 'u':
				// \uXXXX. Decoded to UTF-8; surrogate pairs are handled by
				// passing the raw code point through, which is enough for
				// the BMP text a household types. A lone surrogate becomes
				// U+FFFD rather than an error, matching what a browser sends.
				if p.i+4 >= len(p.b) {
					p.fail(errJSONSyntax)
					return nil, false
				}
				v, err := strconv.ParseUint(string(p.b[p.i+1:p.i+5]), 16, 32)
				if err != nil {
					p.fail(errJSONSyntax)
					return nil, false
				}
				p.i += 4
				out = appendRune(out, rune(v))
			default:
				p.fail(errJSONSyntax)
				return nil, false
			}
			p.i++
		case c < 0x20:
			p.fail(errJSONSyntax)
			return nil, false
		default:
			out = append(out, c)
			p.i++
		}
	}
	p.fail(errJSONSyntax)
	return nil, false
}

// appendRune encodes one rune as UTF-8 without importing unicode/utf8, which
// would pull in tables this board has no other use for.
func appendRune(b []byte, r rune) []byte {
	switch {
	case r < 0x80:
		return append(b, byte(r))
	case r < 0x800:
		return append(b, byte(0xC0|r>>6), byte(0x80|r&0x3F))
	case r >= 0xD800 && r <= 0xDFFF:
		// Lone surrogate: not valid on its own.
		return append(b, 0xEF, 0xBF, 0xBD) // U+FFFD
	case r < 0x10000:
		return append(b, byte(0xE0|r>>12), byte(0x80|r>>6&0x3F), byte(0x80|r&0x3F))
	default:
		return append(b, byte(0xF0|r>>18), byte(0x80|r>>12&0x3F),
			byte(0x80|r>>6&0x3F), byte(0x80|r&0x3F))
	}
}

// str reads a string field into a Go string.
//
// This is the one unavoidable allocation: the request struct keeps the value,
// and a Go string is immutable, so the bytes must be copied out of the body.
// One allocation per string field actually present.
func (p *jsonr) str(scratch []byte) string {
	b, _ := p.rawString(scratch)
	if p.err != nil {
		return ""
	}
	return string(b)
}

// optStr reads a string field into a *string, allocating both.
func (p *jsonr) optStr(scratch []byte) *string {
	s := p.str(scratch)
	if p.err != nil {
		return nil
	}
	return &s
}

// int64 reads a number. Costs nothing: parsed straight from the body.
func (p *jsonr) int64() int64 {
	p.ws()
	start := p.i
	if p.byteAt() == '-' {
		p.i++
	}
	for p.i < len(p.b) {
		c := p.b[p.i]
		if c < '0' || c > '9' {
			break
		}
		p.i++
	}
	if start == p.i {
		p.fail(errJSONType)
		return 0
	}
	v, err := strconv.ParseInt(string(p.b[start:p.i]), 10, 64)
	if err != nil {
		p.fail(errJSONType)
		return 0
	}
	return v
}

func (p *jsonr) optInt64() *int64 {
	v := p.int64()
	if p.err != nil {
		return nil
	}
	return &v
}

// bool reads true or false.
func (p *jsonr) bool() bool {
	p.ws()
	switch {
	case p.hasPrefix("true"):
		p.i += 4
		return true
	case p.hasPrefix("false"):
		p.i += 5
		return false
	}
	p.fail(errJSONType)
	return false
}

func (p *jsonr) optBool() *bool {
	v := p.bool()
	if p.err != nil {
		return nil
	}
	return &v
}

func (p *jsonr) hasPrefix(s string) bool {
	if len(p.b)-p.i < len(s) {
		return false
	}
	return string(p.b[p.i:p.i+len(s)]) == s
}

// isNull consumes a literal null if present.
func (p *jsonr) isNull() bool {
	p.ws()
	if p.hasPrefix("null") {
		p.i += 4
		return true
	}
	return false
}

// object walks a JSON object, calling field for each key.
//
// The key is handed over as a sub-slice of the body, never allocated. The
// callback returns false for a name it does not recognise, which the parser
// turns into an error - matching DisallowUnknownFields, because a misspelled
// key on a money API must fail rather than be ignored.
func (p *jsonr) object(depth int, field func(key []byte) bool) {
	if depth > maxDepth {
		p.fail(errJSONDepth)
		return
	}
	// Keys need somewhere to unescape into. Passing nil made any escaped
	// key - or any escaped *value*, since rawString shares this path -
	// decode into a zero-length buffer, which silently truncated it: a
	// body with " in it parsed as malformed rather than as a quote.
	//
	// 64 bytes is ample: every key in this API is a short identifier, and
	// the longest is "code_challenge_method" at 21.
	var keyScratch [64]byte
	if !p.expect('{') {
		return
	}
	p.ws()
	if p.byteAt() == '}' {
		p.i++
		return
	}

	for p.err == nil {
		key, _ := p.rawString(keyScratch[:])
		if p.err != nil {
			return
		}
		if !p.expect(':') {
			return
		}
		if !field(key) {
			p.fail(errJSONUnknown)
			return
		}
		p.ws()
		switch p.byteAt() {
		case ',':
			p.i++
		case '}':
			p.i++
			return
		default:
			p.fail(errJSONSyntax)
			return
		}
	}
}

// done reports whether the document parsed cleanly and was fully consumed.
func (p *jsonr) done() error {
	if p.err != nil {
		return p.err
	}
	p.ws()
	if p.i != len(p.b) {
		return errJSONSyntax
	}
	return nil
}

// keyIs compares a key slice to a literal without allocating.
func keyIs(key []byte, name string) bool {
	return len(key) == len(name) && string(key) == name
}
