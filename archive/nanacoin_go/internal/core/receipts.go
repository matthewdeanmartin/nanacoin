package core

import "github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"

const MaxIdempotencyEntries = 16
const MaxIdempotencyBytes = 4 << 10

type receiptEntry struct {
	user     ledger.UserID
	endpoint string
	key      [MaxIdempotencyKey]byte
	keyLen   uint8
	size     uint16
}

// FIFO entries and their contiguous bodies move together under Service.mu.
// No pointers into this storage leave the lock: readers get their own copy.
type receiptCache struct {
	entries [MaxIdempotencyEntries]receiptEntry
	data    [MaxIdempotencyBytes]byte
	count   int
	used    int
}

func (c *receiptCache) find(user ledger.UserID, endpoint, key string) (int, int, bool) {
	off := 0
	for i := 0; i < c.count; i++ {
		e := &c.entries[i]
		if e.user == user && e.endpoint == endpoint && int(e.keyLen) == len(key) && string(e.key[:e.keyLen]) == key {
			return off, int(e.size), true
		}
		off += int(e.size)
	}
	return 0, 0, false
}

func (c *receiptCache) remember(user ledger.UserID, endpoint, key string, result []byte) {
	if len(key) > MaxIdempotencyKey || len(result) > len(c.data) {
		return
	}
	if _, _, ok := c.find(user, endpoint, key); ok {
		return
	}
	for c.count > 0 && (c.count == len(c.entries) || c.used+len(result) > len(c.data)) {
		n := int(c.entries[0].size)
		copy(c.data[:], c.data[n:c.used])
		c.used -= n
		copy(c.entries[:], c.entries[1:c.count])
		c.count--
		c.entries[c.count] = receiptEntry{}
	}
	e := &c.entries[c.count]
	e.user = user
	e.endpoint = endpoint
	e.keyLen = uint8(len(key))
	e.size = uint16(len(result))
	copy(e.key[:], key)
	copy(c.data[c.used:], result)
	c.used += len(result)
	c.count++
}
