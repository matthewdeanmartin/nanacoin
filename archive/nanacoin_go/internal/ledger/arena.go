package ledger

import "sync"

// Text storage is fixed at startup. Evicted records release their blocks so
// history can wrap indefinitely without accumulating discarded memo bytes.
const ArenaSize = 12 << 10
const arenaBlock = 16

type Slot struct{ Off, Len uint16 }

func (s Slot) IsEmpty() bool { return s.Len == 0 }

type Arena struct {
	mu        sync.RWMutex
	buf       []byte
	blocks    [ArenaSize / arenaBlock]bool
	used      int
	truncated int
}

func NewArena() *Arena    { return &Arena{buf: make([]byte, ArenaSize)} }
func blocksFor(n int) int { return (n + arenaBlock - 1) / arenaBlock }

// freeRun excludes a hypothetical allocation, for a non-mutating two-slot check.
func (a *Arena) freeRun(need, skip, skipEnd int) int {
	if need == 0 {
		return 0
	}
	run := 0
	for i := range a.blocks {
		busy := a.blocks[i]
		if busy || i >= skip && i < skipEnd {
			run = 0
			continue
		}
		run++
		if run == need {
			return i + 1 - run
		}
	}
	return -1
}
func (a *Arena) CanStore(first, second string) bool {
	a.mu.RLock()
	defer a.mu.RUnlock()
	n := blocksFor(len(first))
	i := a.freeRun(n, 0, 0)
	return i >= 0 && a.freeRun(blocksFor(len(second)), i, i+n) >= 0
}
func (a *Arena) Put(s string) Slot {
	if s == "" {
		return Slot{}
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	n := len(s)
	i := a.freeRun(blocksFor(n), 0, 0)
	if i < 0 {
		// Preserve the previous bounded-text truncation policy if domain records
		// alone occupy the arena. Ledger writers first evict old text to make room.
		best, run := 0, 0
		for j := range a.blocks {
			busy := a.blocks[j]
			if busy {
				run = 0
				continue
			}
			run++
			if run > best {
				best, i = run, j+1-run
			}
		}
		n = best * arenaBlock
		a.truncated++
		for n > 0 && n < len(s) && s[n]&0xc0 == 0x80 {
			n--
		}
	}
	if n == 0 {
		return Slot{}
	}
	for j := i; j < i+blocksFor(n); j++ {
		a.blocks[j] = true
	}
	off := i * arenaBlock
	copy(a.buf[off:off+n], s[:n])
	a.used += n
	return Slot{Off: uint16(off), Len: uint16(n)}
}

// Release invalidates all aliases. Only whole slots returned by Put are valid.
func (a *Arena) Release(s Slot) {
	if s.Len == 0 || int(s.Off)%arenaBlock != 0 {
		return
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	start, count := int(s.Off)/arenaBlock, blocksFor(int(s.Len))
	if start+count > len(a.blocks) {
		return
	}
	for i := start; i < start+count; i++ {
		if !a.blocks[i] {
			return
		}
	}
	for i := start; i < start+count; i++ {
		a.blocks[i] = false
	}
	a.used -= int(s.Len)
}
func (a *Arena) Get(s Slot) string {
	if s.Len == 0 {
		return ""
	}
	a.mu.RLock()
	defer a.mu.RUnlock()
	end := int(s.Off) + int(s.Len)
	if end > len(a.buf) {
		return ""
	}
	return string(a.buf[s.Off:end])
}

// Bytes aliases storage until the owner is replaced/evicted. Hold the service
// lock across rendering so no writer can reuse a slot during the read.
func (a *Arena) Bytes(s Slot) []byte {
	if s.Len == 0 {
		return nil
	}
	a.mu.RLock()
	defer a.mu.RUnlock()
	end := int(s.Off) + int(s.Len)
	if end > len(a.buf) {
		return nil
	}
	return a.buf[s.Off:end]
}
// Equal reports whether a slot holds exactly this text, without building a
// string for it.
//
// Get allocates a copy on every call, which is invisible when rendering one
// record and expensive when scanning: the find-by-ID helpers compare a slot
// against a wanted ID for every occupied slot in the table, so a lookup over
// 16 quotes allocated 16 strings to return one index. Comparing a byte slice
// to a string is a direct comparison the compiler does not allocate for.
func (a *Arena) Equal(s Slot, want string) bool {
	if int(s.Len) != len(want) {
		return false
	}
	if s.Len == 0 {
		return true
	}
	a.mu.RLock()
	defer a.mu.RUnlock()
	end := int(s.Off) + int(s.Len)
	if end > len(a.buf) {
		return false
	}
	return string(a.buf[s.Off:end]) == want
}

func (a *Arena) Stats() (used, capacity, truncated int) {
	a.mu.RLock()
	defer a.mu.RUnlock()
	return a.used, len(a.buf), a.truncated
}
func (a *Arena) Reset() {
	a.mu.Lock()
	defer a.mu.Unlock()
	clear(a.blocks[:])
	a.used, a.truncated = 0, 0
}

// TextReplacement is caller-owned scratch, reused under the service lock.
// Pointers avoid aggregate argument/return handling differences on TinyGo.
type TextReplacement struct {
	Old    [3]Slot
	Values [3]string
	Slots  [3]Slot
}

func (a *Arena) PlanReplacement(p *TextReplacement) bool {
	a.mu.RLock()
	defer a.mu.RUnlock()
	return a.planReplacement(p)
}
func (a *Arena) planReplacement(p *TextReplacement) bool {
	p.Slots = [3]Slot{}
	// Preserve fitting fields before searching for space for larger ones.
	for i := range p.Values {
		n := len(p.Values[i])
		if n > 0 && blocksFor(n) <= blocksFor(int(p.Old[i].Len)) {
			p.Slots[i] = Slot{Off: p.Old[i].Off, Len: uint16(n)}
		}
	}
	for k := range p.Values {
		need := blocksFor(len(p.Values[k]))
		if need == 0 || p.Slots[k].Len > 0 {
			continue
		}
		run := 0
		for i := range a.blocks {
			busy := a.blocks[i]
			for j := range p.Old {
				slot := &p.Old[j]
				if slot.Len > 0 && i >= int(slot.Off)/arenaBlock && i < int(slot.Off)/arenaBlock+blocksFor(int(slot.Len)) {
					busy = false
				}
			}
			for j := range p.Slots {
				slot := &p.Slots[j]
				if slot.Len > 0 && i >= int(slot.Off)/arenaBlock && i < int(slot.Off)/arenaBlock+blocksFor(int(slot.Len)) {
					busy = true
				}
			}
			if busy {
				run = 0
			} else {
				run++
			}
			if run == need {
				p.Slots[k] = Slot{Off: uint16((i + 1 - need) * arenaBlock), Len: uint16(len(p.Values[k]))}
				break
			}
		}
		if p.Slots[k].Len == 0 {
			return false
		}
	}
	return true
}

// Replace installs all strings or changes nothing. Values must own their bytes;
// aliases from Bytes are not valid. Slots is valid only after a true result.
func (a *Arena) Replace(p *TextReplacement) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	if !a.planReplacement(p) {
		return false
	}
	for i := range p.Old {
		slot := &p.Old[i]
		for j := int(slot.Off) / arenaBlock; slot.Len > 0 && j < int(slot.Off)/arenaBlock+blocksFor(int(slot.Len)); j++ {
			a.blocks[j] = false
		}
		a.used -= int(slot.Len)
	}
	for i := range p.Slots {
		slot := &p.Slots[i]
		for j := int(slot.Off) / arenaBlock; slot.Len > 0 && j < int(slot.Off)/arenaBlock+blocksFor(int(slot.Len)); j++ {
			a.blocks[j] = true
		}
		copy(a.buf[int(slot.Off):int(slot.Off)+int(slot.Len)], p.Values[i])
		a.used += int(slot.Len)
	}
	return true
}
