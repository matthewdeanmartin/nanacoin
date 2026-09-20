// Package storage defines NanaCoin's persistence contract: an append-only
// journal of typed records.
//
// Nothing above this package knows what the records are stored on. That is the
// point - the economic logic must be able to run against a desktop file, an
// ESP32 flash partition, an SD card or a test double without changing (spec 4).
package storage

import (
	"encoding/binary"
	"errors"
	"fmt"
	"hash/crc32"
)

// Magic marks the start of a record. A scan that does not find it at the
// expected offset has reached unwritten flash or garbage, and stops.
const Magic uint32 = 0x4E414E41 // "NANA"

// FormatVersion is the on-disk record format, bumped only for changes that
// older readers cannot tolerate.
const FormatVersion uint16 = 1

// HeaderSize is the fixed prefix before each payload:
//
//	magic u32 | version u16 | type u16 | length u32 | sequence u64 | crc u32
//
// The CRC covers the header bytes before it plus the payload, so a torn write
// anywhere in the record is detected.
const HeaderSize = 4 + 2 + 2 + 4 + 8 + 4

// CommitSize is the trailing marker, written after the header and payload have
// landed. Its presence is what makes a record real: a power failure between
// the payload and the marker leaves a record that replay discards, so a
// partially written transaction can never become an economic transaction
// (spec 24).
const CommitSize = 4

// CommitMarker is a bit pattern that erased flash (all 1s) and zeroed storage
// cannot accidentally produce.
const CommitMarker uint32 = 0x5A5AC0DE

// MaxPayload bounds a single record. It exists so that a corrupt length field
// cannot make a reader try to allocate gigabytes on a device with a few
// hundred KB of RAM.
const MaxPayload = 8 * 1024

type RecordType uint16

const (
	TypeUserCreated RecordType = 1
	TypeUserUpdated RecordType = 2

	TypeAccountCreated RecordType = 10

	TypeTransactionCreated RecordType = 20

	TypeListingCreated   RecordType = 30
	TypeListingUpdated   RecordType = 31
	TypeListingPurchased RecordType = 32
	TypeListingCancelled RecordType = 33

	TypeOfferCreated  RecordType = 34
	TypeOfferAccepted RecordType = 35
	TypeOfferUpdated  RecordType = 36

	TypeQuoteCreated RecordType = 37
	TypeQuoteTaken   RecordType = 38
	TypeQuoteSettled RecordType = 39
	TypeQuoteUpdated RecordType = 41

	TypeConfigUpdated RecordType = 40

	TypeIdempotency RecordType = 50

	TypeSnapshot RecordType = 90
)

func (t RecordType) String() string {
	switch t {
	case TypeUserCreated:
		return "USER_CREATED"
	case TypeUserUpdated:
		return "USER_UPDATED"
	case TypeAccountCreated:
		return "ACCOUNT_CREATED"
	case TypeTransactionCreated:
		return "TRANSACTION_CREATED"
	case TypeListingCreated:
		return "LISTING_CREATED"
	case TypeListingUpdated:
		return "LISTING_UPDATED"
	case TypeListingPurchased:
		return "LISTING_PURCHASED"
	case TypeListingCancelled:
		return "LISTING_CANCELLED"
	case TypeOfferCreated:
		return "OFFER_CREATED"
	case TypeOfferAccepted:
		return "OFFER_ACCEPTED"
	case TypeOfferUpdated:
		return "OFFER_UPDATED"
	case TypeQuoteCreated:
		return "QUOTE_CREATED"
	case TypeQuoteTaken:
		return "QUOTE_TAKEN"
	case TypeQuoteSettled:
		return "QUOTE_SETTLED"
	case TypeQuoteUpdated:
		return "QUOTE_UPDATED"
	case TypeConfigUpdated:
		return "CONFIG_UPDATED"
	case TypeIdempotency:
		return "IDEMPOTENCY"
	case TypeSnapshot:
		return "SNAPSHOT"
	}
	return fmt.Sprintf("UNKNOWN(%d)", uint16(t))
}

// Record is one journal entry. Sequence is assigned by the journal and is
// strictly increasing with no gaps; a gap means a record was lost and replay
// stops rather than silently skipping money.
type Record struct {
	Type     RecordType
	Sequence uint64
	Payload  []byte
}

var (
	ErrBadMagic   = errors.New("record magic mismatch")
	ErrBadCRC     = errors.New("record crc mismatch")
	ErrNoCommit   = errors.New("record has no commit marker")
	ErrTooLarge   = errors.New("record payload exceeds maximum")
	ErrShortRead  = errors.New("record truncated")
	ErrBadVersion = errors.New("unsupported record format version")
)

// Encode lays out a complete framed record, commit marker included. Backends
// that can write a record in one call may write this whole slice; a backend
// writing in stages must write bytes [0:len-CommitSize] first, flush, and only
// then the final four bytes.
func Encode(r *Record) ([]byte, error) {
	if len(r.Payload) > MaxPayload {
		return nil, ErrTooLarge
	}
	buf := make([]byte, HeaderSize+len(r.Payload)+CommitSize)

	binary.LittleEndian.PutUint32(buf[0:], Magic)
	binary.LittleEndian.PutUint16(buf[4:], FormatVersion)
	binary.LittleEndian.PutUint16(buf[6:], uint16(r.Type))
	binary.LittleEndian.PutUint32(buf[8:], uint32(len(r.Payload)))
	binary.LittleEndian.PutUint64(buf[12:], r.Sequence)

	copy(buf[HeaderSize:], r.Payload)

	// CRC over header-before-crc + payload.
	//
	// Update against the package table rather than crc32.NewIEEE(). The
	// constructor allocates a hash.Hash32 object - measured at 820 bytes per
	// call - and this function is on the write path of every single journal
	// record, so those 820 bytes were being spent and discarded on every
	// append. crc32.Update and crc32.ChecksumIEEE use the same cached table
	// and allocate nothing at all.
	//
	// Worth stating because it looked like a one-time cost: the first Encode
	// on the board charged 8 kB and later ones 5 kB, which reads like lazy
	// initialisation and is really just this, repeated.
	crc := crc32.Update(0, crc32.IEEETable, buf[0:20])
	crc = crc32.Update(crc, crc32.IEEETable, r.Payload)
	binary.LittleEndian.PutUint32(buf[20:], crc)

	binary.LittleEndian.PutUint32(buf[HeaderSize+len(r.Payload):], CommitMarker)
	return buf, nil
}

// CommitOffset is where the commit marker sits within an encoded record of the
// given payload size. Staged writers need it.
func CommitOffset(payloadLen int) int { return HeaderSize + payloadLen }

// Decode reads one record from the front of buf. It returns the record and the
// total bytes consumed.
//
// Every failure here means "the journal ends at this point". Replay treats
// them all the same way: stop, keep everything already read, and let the next
// append overwrite from here. That is the whole crash-recovery story.
func Decode(buf []byte) (*Record, int, error) {
	if len(buf) < HeaderSize {
		return nil, 0, ErrShortRead
	}
	if binary.LittleEndian.Uint32(buf[0:]) != Magic {
		return nil, 0, ErrBadMagic
	}
	if v := binary.LittleEndian.Uint16(buf[4:]); v != FormatVersion {
		return nil, 0, ErrBadVersion
	}
	typ := RecordType(binary.LittleEndian.Uint16(buf[6:]))
	length := binary.LittleEndian.Uint32(buf[8:])
	if length > MaxPayload {
		return nil, 0, ErrTooLarge
	}
	seq := binary.LittleEndian.Uint64(buf[12:])
	want := binary.LittleEndian.Uint32(buf[20:])

	total := HeaderSize + int(length) + CommitSize
	if len(buf) < total {
		return nil, 0, ErrShortRead
	}
	payload := buf[HeaderSize : HeaderSize+int(length)]

	// Table-based, like Encode: this runs once per record on every replay,
	// so a per-call allocation here is paid for the whole journal at boot.
	crc := crc32.Update(0, crc32.IEEETable, buf[0:20])
	crc = crc32.Update(crc, crc32.IEEETable, payload)
	if crc != want {
		return nil, 0, ErrBadCRC
	}
	if binary.LittleEndian.Uint32(buf[HeaderSize+int(length):]) != CommitMarker {
		return nil, 0, ErrNoCommit
	}

	// Copy the payload: buf is usually a mapped or reused read buffer, and
	// the decoded records outlive it.
	out := make([]byte, len(payload))
	copy(out, payload)
	return &Record{Type: typ, Sequence: seq, Payload: out}, total, nil
}
