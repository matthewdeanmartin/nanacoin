package storage

import (
	"bytes"
	"testing"
)

func TestEveryTornWritePrefixRejected(t *testing.T) {
	encoded, err := Encode(&Record{Type: TypeTransactionCreated, Sequence: 99, Payload: bytes.Repeat([]byte{0x5a}, 256)})
	if err != nil {
		t.Fatal(err)
	}
	for n := 0; n < len(encoded); n++ {
		if _, _, err := Decode(encoded[:n]); err == nil {
			t.Fatalf("accepted torn write at byte %d", n)
		}
	}
}
func FuzzDecodeFraming(f *testing.F) {
	good, _ := Encode(&Record{Type: TypeTransactionCreated, Sequence: 1, Payload: []byte("coins")})
	f.Add(good)
	f.Add([]byte{})
	f.Fuzz(func(t *testing.T, data []byte) {
		r, n, e := Decode(data)
		if e != nil {
			return
		}
		if n <= 0 || n > len(data) {
			t.Fatal("invalid consumed length")
		}
		encoded, e := Encode(r)
		if e != nil {
			t.Fatal(e)
		}
		again, m, e := Decode(encoded)
		if e != nil || m != len(encoded) || again.Sequence != r.Sequence || !bytes.Equal(again.Payload, r.Payload) {
			t.Fatal("accepted record does not round trip")
		}
	})
}
