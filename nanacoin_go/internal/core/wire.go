package core

import (
	"encoding/binary"
	"errors"
)

// A hand-written binary format for journal events.
//
// # Why this replaced JSON
//
// Not for the bytes. The journal lives on flash, of which this board has
// 16 MB, and a year of household use was about 200 kB of JSON - 1.2% of it.
// Space was never the problem and the old format's readability under
// `strings` was a real benefit.
//
// The problem was reflection. Every commit called json.Marshal and every
// replay called json.Unmarshal, which kept encoding/json and reflectlite -
// 138 allocation sites between them, measured with
// `tinygo build -print-allocs` - linked into the binary and running on the
// write path. Converting the API layer's responses alone moved that count
// from 138 to 138, because one reflective caller anywhere keeps the whole
// machinery reachable.
//
// So this is the write half of the same job: typed encoders, no reflection,
// nothing discovered at runtime.
//
// # The shape
//
// Length-prefixed fields in a fixed order per event type. Strings carry a
// uint16 length; numbers are little-endian; optional fields carry a presence
// byte. No field names on the wire - the order is the schema, and the schema
// is this file.
//
// # What it gives up
//
// Reading a dead board's journal with `strings`. That was worth something and
// it is genuinely lost. The replacement is a `dump` subcommand on the desktop
// build that decodes a flash image, which is better than `strings` anyway
// because it can resolve interned values and check CRCs.
//
// # Versioning
//
// storage.FormatVersion already prefixes every record and is bumped for
// changes older readers cannot tolerate. A field added to an event here is
// exactly such a change: the order is the schema, so there is no way for an
// old reader to skip a new field. Bump the version when that happens.

var (
	errWireShort   = errors.New("wire: buffer too short")
	errWireTooLong = errors.New("wire: string exceeds 64 kB")
)

// --- encoding ---------------------------------------------------------------
//
// Every encoder appends to a caller-supplied slice and returns it, the same
// contract as strconv.Append* and the API layer's jsonw. The caller owns the
// storage, so a fixed buffer reused across commits allocates nothing.

func putU8(b []byte, v uint8) []byte { return append(b, v) }

func putU16(b []byte, v uint16) []byte {
	return append(b, byte(v), byte(v>>8))
}

func putU32(b []byte, v uint32) []byte {
	return append(b, byte(v), byte(v>>8), byte(v>>16), byte(v>>24))
}

func putU64(b []byte, v uint64) []byte {
	var tmp [8]byte
	binary.LittleEndian.PutUint64(tmp[:], v)
	return append(b, tmp[:]...)
}

func putI64(b []byte, v int64) []byte { return putU64(b, uint64(v)) }

func putBool(b []byte, v bool) []byte {
	if v {
		return append(b, 1)
	}
	return append(b, 0)
}

// putStr writes a uint16 length followed by the bytes.
//
// 64 kB is far beyond anything this system stores - the longest field is a
// 500-character description - so a string that overflows the length is a bug
// rather than a condition, and truncating quietly would corrupt a record.
// Encoders check with strFits before writing.
func putStr(b []byte, s string) []byte {
	if len(s) > 0xFFFF {
		s = s[:0xFFFF]
	}
	b = putU16(b, uint16(len(s)))
	return append(b, s...)
}

func strFits(s string) bool { return len(s) <= 0xFFFF }

// putOptStr writes a presence byte then the string, for a *string field.
func putOptStr(b []byte, s *string) []byte {
	if s == nil {
		return append(b, 0)
	}
	b = append(b, 1)
	return putStr(b, *s)
}

// putOptI64 writes a presence byte then the number.
func putOptI64(b []byte, v *int64) []byte {
	if v == nil {
		return append(b, 0)
	}
	b = append(b, 1)
	return putI64(b, *v)
}

// putOptU8 writes a presence byte then one byte, for optional enums.
func putOptU8(b []byte, v *uint8) []byte {
	if v == nil {
		return append(b, 0)
	}
	return append(b, 1, *v)
}

// putBytes writes a uint16 length followed by raw bytes.
func putBytes(b []byte, v []byte) []byte {
	if len(v) > 0xFFFF {
		v = v[:0xFFFF]
	}
	b = putU16(b, uint16(len(v)))
	return append(b, v...)
}

// --- decoding ---------------------------------------------------------------
//
// A cursor over the payload. Every read checks the remaining length and sets
// a sticky error, so a truncated or corrupt record fails at the first bad
// field rather than reading past the end - the CRC catches most corruption,
// but a record whose length field survived a flip would still reach here.

type reader struct {
	b   []byte
	i   int
	err error
}

func newReader(b []byte) *reader { return &reader{b: b} }

func (r *reader) need(n int) bool {
	if r.err != nil {
		return false
	}
	if len(r.b)-r.i < n {
		r.err = errWireShort
		return false
	}
	return true
}

func (r *reader) u8() uint8 {
	if !r.need(1) {
		return 0
	}
	v := r.b[r.i]
	r.i++
	return v
}

func (r *reader) u16() uint16 {
	if !r.need(2) {
		return 0
	}
	v := uint16(r.b[r.i]) | uint16(r.b[r.i+1])<<8
	r.i += 2
	return v
}

func (r *reader) u32() uint32 {
	if !r.need(4) {
		return 0
	}
	v := binary.LittleEndian.Uint32(r.b[r.i:])
	r.i += 4
	return v
}

func (r *reader) u64() uint64 {
	if !r.need(8) {
		return 0
	}
	v := binary.LittleEndian.Uint64(r.b[r.i:])
	r.i += 8
	return v
}

func (r *reader) i64() int64 { return int64(r.u64()) }

func (r *reader) bool() bool { return r.u8() == 1 }

// str reads a length-prefixed string.
//
// Allocates, unavoidably: the caller gets a Go string it holds. That is the
// one allocation per string on the replay path, which happens at boot and
// once per commit - not per request - so it is the right side of the trade.
// The packed store interns or arenas it immediately afterwards.
func (r *reader) str() string {
	n := int(r.u16())
	if !r.need(n) {
		return ""
	}
	s := string(r.b[r.i : r.i+n])
	r.i += n
	return s
}

// optStr reads a presence byte then a string.
func (r *reader) optStr() *string {
	if r.u8() == 0 {
		return nil
	}
	s := r.str()
	return &s
}

func (r *reader) optI64() *int64 {
	if r.u8() == 0 {
		return nil
	}
	v := r.i64()
	return &v
}

func (r *reader) optU8() *uint8 {
	if r.u8() == 0 {
		return nil
	}
	v := r.u8()
	return &v
}

// bytes reads a length-prefixed byte slice, copied so the caller may retain
// it after the payload buffer is reused.
func (r *reader) bytes() []byte {
	n := int(r.u16())
	if !r.need(n) {
		return nil
	}
	out := make([]byte, n)
	copy(out, r.b[r.i:r.i+n])
	r.i += n
	return out
}

// done reports whether the whole payload was consumed without error.
//
// Trailing bytes are an error, not a tolerated condition: they mean the
// record was written by a different version of this file, and silently
// ignoring them is how a format mismatch becomes a wrong balance.
func (r *reader) done() error {
	if r.err != nil {
		return r.err
	}
	if r.i != len(r.b) {
		return errors.New("wire: trailing bytes in record")
	}
	return nil
}
