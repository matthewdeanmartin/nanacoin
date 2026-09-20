package auth

import (
	"encoding/binary"
	"sync"
)

// Go's generic SHA-256 compression is in sha256block.go, with its BSD license.
// This one-shot wrapper owns all scratch: Go 1.26's Sum/checkSum padding and
// AppendBinary zero-fill buffers escape in TinyGo. No interfaces or heap-backed
// padding are needed here. Differential tests cover every length through 4095.
type fixedSHAState struct {
	h [8]uint32
	w [64]uint32
}

var shaWork struct {
	sync.Mutex
	digest  fixedSHAState
	padding [128]byte
}

//go:noinline
func shaInto(input []byte, out *[32]byte) {
	shaWork.Lock()
	defer shaWork.Unlock()
	d := &shaWork.digest
	d.h = [8]uint32{0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19}
	complete := len(input) &^ 63
	blockGeneric(d, input[:complete])
	clear(shaWork.padding[:])
	remaining := len(input) - complete
	copy(shaWork.padding[:], input[complete:])
	shaWork.padding[remaining] = 0x80
	padded := 64
	if remaining >= 56 {
		padded = 128
	}
	binary.BigEndian.PutUint64(shaWork.padding[padded-8:padded], uint64(len(input))*8)
	blockGeneric(d, shaWork.padding[:padded])
	for i := range d.h {
		binary.BigEndian.PutUint32(out[i*4:i*4+4], d.h[i])
	}
	clear(d.h[:])
	clear(d.w[:])
	clear(shaWork.padding[:])
}
