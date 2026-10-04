// Package memory is a Journal that keeps records in RAM. It is the test
// backend, and it is also what a "sessions expire on reboot, ledger does not"
// deployment would never use - anything that matters gets a real backend.
package memory

import (
	"errors"
	"sync"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
)

// Journal is an append-only log held in RAM.
//
// # Retention
//
// By default every record is kept, because that is what makes Replay work and
// tests depend on it. On the board that default is wrong in a way that is
// easy to miss: the board runs this backend, and the service *also* rebuilds
// full state in RAM as each record is applied. So every mutation was stored
// twice - once as the live object and once as the encoded record that
// produced it - and the second copy could never be read, because a Replay
// only ever happens at boot and the board's RAM does not survive a boot.
//
// That made every successful write a permanent leak with no possible reader.
// Discard, set by the board, keeps sequence numbering and byte accounting
// without encoding data that can never be replayed. The retaining backend
// still exercises framing and replay; codec tests cover corrupt records.
//
// A flash backend makes this moot - the records live on flash and cost no RAM
// at all - but until that lands this is the difference between a board that
// can be used and one that fills up.
type Journal struct {
	mu   sync.Mutex
	recs []*storage.Record
	seq  uint64

	// Discard counts each record without retaining or framing it.
	// Replay is then a no-op. Set by hosts that rebuild state some other way
	// and can never replay - which on this board is all of them, since RAM
	// does not survive the reboot that would trigger a replay.
	Discard bool

	// written accumulates the bytes a real backend would have consumed, so
	// that Size stays meaningful once the records themselves are discarded.
	written int64

	// FailAfter, when positive, makes the Nth Append fail. Tests use it to
	// prove that a failed journal write leaves no trace in the RAM model.
	FailAfter int
	n         int
	err       error
}

func New() *Journal { return &Journal{} }

// NewDiscarding returns a journal that numbers and validates records but
// retains none. See the Discard field.
func NewDiscarding() *Journal { return &Journal{Discard: true} }

// SetFailure arms the backend to fail every Append after the given count.
func (j *Journal) SetFailure(after int, err error) {
	j.mu.Lock()
	defer j.mu.Unlock()
	j.FailAfter, j.err = after, err
}

// Appends reports how many Append calls have been made, so a test can arm a
// failure starting from the current point.
func (j *Journal) Appends() int {
	j.mu.Lock()
	defer j.mu.Unlock()
	return j.n
}

func (j *Journal) Append(typ storage.RecordType, payload []byte) (uint64, error) {
	j.mu.Lock()
	defer j.mu.Unlock()

	j.n++
	if j.FailAfter > 0 && j.n > j.FailAfter {
		return 0, j.err
	}

	if j.Discard {
		return j.appendDiscardedLocked(len(payload))
	}

	// Round-trip through the real encoder so the memory backend exercises
	// the same framing bugs a flash backend would. Discarding never reaches
	// this path because there is no stored frame to validate.
	j.seq++
	buf, err := storage.Encode(&storage.Record{Type: typ, Sequence: j.seq, Payload: payload})
	if err != nil {
		j.seq--
		return 0, err
	}
	rec, _, err := storage.Decode(buf)
	if err != nil {
		j.seq--
		return 0, err
	}
	j.recs = append(j.recs, rec)
	return rec.Sequence, nil
}

func (j *Journal) Replay(fn func(*storage.Record) error) error {
	j.mu.Lock()
	snapshot := make([]*storage.Record, len(j.recs))
	copy(snapshot, j.recs)
	j.mu.Unlock()

	for _, r := range snapshot {
		if err := fn(r); err != nil {
			return err
		}
	}
	return nil
}

// Size reports the bytes a real backend would have used.
//
// When discarding, the records are gone but the figure is still meaningful -
// it is what the journal would occupy on flash, which is what the status
// endpoint is reporting on. So it is accumulated as records pass through
// rather than measured from the slice.
func (j *Journal) Size() (int64, int64) {
	j.mu.Lock()
	defer j.mu.Unlock()
	if j.Discard {
		return j.written, 0
	}
	var used int64
	for _, r := range j.recs {
		used += int64(storage.HeaderSize + len(r.Payload) + storage.CommitSize)
	}
	return used, 0 // unbounded
}

func (j *Journal) Close() error { return nil }

// Discard must be configured before the journal is handed to a service.
func (j *Journal) DiscardsPayload() bool { return j.Discard }

func (j *Journal) AppendDiscarded(_ storage.RecordType, size int) (uint64, error) {
	j.mu.Lock()
	defer j.mu.Unlock()
	if !j.Discard {
		return 0, errors.New("retaining journal requires payload bytes")
	}
	j.n++
	if j.FailAfter > 0 && j.n > j.FailAfter {
		return 0, j.err
	}
	return j.appendDiscardedLocked(size)
}
func (j *Journal) appendDiscardedLocked(size int) (uint64, error) {
	if size < 0 || size > storage.MaxPayload {
		return 0, storage.ErrTooLarge
	}
	j.seq++
	j.written += int64(storage.HeaderSize + size + storage.CommitSize)
	return j.seq, nil
}
