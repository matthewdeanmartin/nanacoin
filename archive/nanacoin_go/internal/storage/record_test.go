package storage

import (
	"bytes"
	"testing"
)

func TestEncodeDecodeRoundTrip(t *testing.T) {
	rec := &Record{Type: TypeTransactionCreated, Sequence: 42, Payload: []byte(`{"hello":"world"}`)}
	buf, err := Encode(rec)
	if err != nil {
		t.Fatalf("Encode: %v", err)
	}

	got, n, err := Decode(buf)
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	if n != len(buf) {
		t.Errorf("consumed %d bytes, record is %d", n, len(buf))
	}
	if got.Type != rec.Type || got.Sequence != rec.Sequence {
		t.Errorf("got type %v seq %d, want %v %d", got.Type, got.Sequence, rec.Type, rec.Sequence)
	}
	if !bytes.Equal(got.Payload, rec.Payload) {
		t.Errorf("payload %q, want %q", got.Payload, rec.Payload)
	}
}

func TestDecodeEmptyPayload(t *testing.T) {
	buf, err := Encode(&Record{Type: TypeSnapshot, Sequence: 1})
	if err != nil {
		t.Fatalf("Encode: %v", err)
	}
	got, _, err := Decode(buf)
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	if len(got.Payload) != 0 {
		t.Errorf("payload should be empty, got %d bytes", len(got.Payload))
	}
}

// A record missing its commit marker is the signature of a power failure
// between the payload write and the commit write. It must not decode: this is
// the guarantee that a half-written transfer never becomes money.
func TestDecodeRejectsMissingCommitMarker(t *testing.T) {
	buf, _ := Encode(&Record{Type: TypeTransactionCreated, Sequence: 1, Payload: []byte("money")})
	truncated := buf[:len(buf)-CommitSize]

	if _, _, err := Decode(truncated); err != ErrShortRead {
		t.Errorf("truncated before marker: got %v, want ErrShortRead", err)
	}

	// And with the marker space present but unwritten - erased flash reads
	// as 0xFF, zeroed storage as 0x00. Neither is the marker.
	for _, fill := range []byte{0x00, 0xFF} {
		bad := append([]byte(nil), buf...)
		for i := len(bad) - CommitSize; i < len(bad); i++ {
			bad[i] = fill
		}
		if _, _, err := Decode(bad); err != ErrNoCommit {
			t.Errorf("marker filled with %#x: got %v, want ErrNoCommit", fill, err)
		}
	}
}

func TestDecodeRejectsCorruptPayload(t *testing.T) {
	buf, _ := Encode(&Record{Type: TypeTransactionCreated, Sequence: 7, Payload: []byte("100 coins")})
	// Flip a bit in the payload, as a flash read disturb or a bad cell would.
	buf[HeaderSize+2] ^= 0x08
	if _, _, err := Decode(buf); err != ErrBadCRC {
		t.Errorf("got %v, want ErrBadCRC", err)
	}
}

func TestDecodeRejectsCorruptHeader(t *testing.T) {
	buf, _ := Encode(&Record{Type: TypeTransactionCreated, Sequence: 7, Payload: []byte("x")})
	buf[12] ^= 0x01 // sequence number
	if _, _, err := Decode(buf); err != ErrBadCRC {
		t.Errorf("got %v, want ErrBadCRC", err)
	}
}

func TestDecodeRejectsBadMagic(t *testing.T) {
	// Unwritten flash: all ones, no magic anywhere.
	erased := bytes.Repeat([]byte{0xFF}, 64)
	if _, _, err := Decode(erased); err != ErrBadMagic {
		t.Errorf("erased flash: got %v, want ErrBadMagic", err)
	}
}

// A corrupt length field must not become an allocation request. On a device
// with 300KB of RAM, honouring a bogus 4GB length is a crash.
func TestDecodeRejectsAbsurdLength(t *testing.T) {
	buf, _ := Encode(&Record{Type: TypeTransactionCreated, Sequence: 1, Payload: []byte("x")})
	buf[8], buf[9], buf[10], buf[11] = 0xFF, 0xFF, 0xFF, 0xFF
	if _, _, err := Decode(buf); err != ErrTooLarge {
		t.Errorf("got %v, want ErrTooLarge", err)
	}
}

func TestEncodeRejectsOversizePayload(t *testing.T) {
	if _, err := Encode(&Record{Payload: make([]byte, MaxPayload+1)}); err != ErrTooLarge {
		t.Errorf("got %v, want ErrTooLarge", err)
	}
}

func TestDecodeRejectsFutureVersion(t *testing.T) {
	buf, _ := Encode(&Record{Type: TypeSnapshot, Sequence: 1})
	buf[4] = 99
	if _, _, err := Decode(buf); err != ErrBadVersion {
		t.Errorf("got %v, want ErrBadVersion", err)
	}
}

// Decode must copy the payload: replay hands it a shared read buffer that gets
// reused, and records that alias it would mutate under the ledger.
func TestDecodeCopiesPayload(t *testing.T) {
	buf, _ := Encode(&Record{Type: TypeTransactionCreated, Sequence: 1, Payload: []byte("original")})
	rec, _, err := Decode(buf)
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	copy(buf[HeaderSize:], "OVERWRIT")
	if string(rec.Payload) != "original" {
		t.Errorf("payload aliased the input buffer: got %q", rec.Payload)
	}
}
