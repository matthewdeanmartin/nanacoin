// Package flashlog is a Journal backed by a single append-only file.
//
// It is the local-development backend, but it is written to behave the way the
// eventual flash backend must, so that bugs show up on a laptop rather than on
// a board with no debugger:
//
//   - records are only ever appended; nothing already written is rewritten
//   - the commit marker is written in a second call, after the header and
//     payload have been flushed, so an interrupted append is indistinguishable
//     from the real hardware failure mode
//   - replay stops at the first bad record and truncates the tail, exactly as
//     a flash reader would resume writing from that offset
//
// A flash implementation replaces the file handle with a partition handle and
// keeps everything else.
package flashlog

import (
	"errors"
	"fmt"
	"io"
	"os"
	"sync"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
)

// ErrFull is returned when the journal has no room for another record. It is
// a real condition on a fixed flash partition, so the desktop backend can be
// given a capacity and made to hit it in tests.
var ErrFull = errors.New("journal full")

type Journal struct {
	mu  sync.Mutex
	f   *os.File
	end int64 // offset just past the last committed record
	seq uint64
	cap int64 // 0 means unbounded
}

// Open opens or creates the journal at path. capacity bounds it in bytes; pass
// 0 for unbounded, or the size of the target flash partition to rehearse the
// full condition locally.
func Open(path string, capacity int64) (*Journal, error) {
	f, err := os.OpenFile(path, os.O_RDWR|os.O_CREATE, 0o600)
	if err != nil {
		return nil, err
	}
	return &Journal{f: f, cap: capacity}, nil
}

func (j *Journal) Append(typ storage.RecordType, payload []byte) (uint64, error) {
	j.mu.Lock()
	defer j.mu.Unlock()

	seq := j.seq + 1
	buf, err := storage.Encode(&storage.Record{Type: typ, Sequence: seq, Payload: payload})
	if err != nil {
		return 0, err
	}
	if j.cap > 0 && j.end+int64(len(buf)) > j.cap {
		return 0, ErrFull
	}

	// Two-phase write. Phase one lands everything but the commit marker; a
	// crash here leaves a record replay will reject on its missing marker,
	// and the next boot resumes writing at j.end, overwriting the debris.
	body := buf[:storage.CommitOffset(len(payload))]
	if _, err := j.f.WriteAt(body, j.end); err != nil {
		return 0, err
	}
	if err := j.f.Sync(); err != nil {
		return 0, err
	}

	// Phase two commits it. Once this sync returns, the record exists.
	marker := buf[storage.CommitOffset(len(payload)):]
	if _, err := j.f.WriteAt(marker, j.end+int64(len(body))); err != nil {
		return 0, err
	}
	if err := j.f.Sync(); err != nil {
		return 0, err
	}

	j.end += int64(len(buf))
	j.seq = seq
	return seq, nil
}

func (j *Journal) Replay(fn func(*storage.Record) error) error {
	j.mu.Lock()
	defer j.mu.Unlock()

	data, err := io.ReadAll(io.NewSectionReader(j.f, 0, 1<<62))
	if err != nil {
		return err
	}

	var off int64
	var lastSeq uint64
	for off < int64(len(data)) {
		rec, n, err := storage.Decode(data[off:])
		if err != nil {
			// Any decode failure is the end of valid history. This is
			// the normal case on an unclean shutdown, not an error.
			break
		}
		if rec.Sequence != lastSeq+1 {
			// A gap means a record vanished. Continuing would replay a
			// ledger with a hole in it, so stop here and treat the rest
			// as lost - a state Nana can inspect, rather than one the
			// server papers over.
			break
		}
		if err := fn(rec); err != nil {
			return fmt.Errorf("replaying record %d (%s): %w", rec.Sequence, rec.Type, err)
		}
		lastSeq = rec.Sequence
		off += int64(n)
	}

	j.end = off
	j.seq = lastSeq

	// Drop the invalid tail so the file on disk matches what we will replay
	// next time. Appends would overwrite it anyway; truncating makes the
	// file honest in the meantime.
	if off < int64(len(data)) {
		if err := j.f.Truncate(off); err != nil {
			return err
		}
	}
	return nil
}

func (j *Journal) Size() (int64, int64) {
	j.mu.Lock()
	defer j.mu.Unlock()
	return j.end, j.cap
}

func (j *Journal) Close() error {
	j.mu.Lock()
	defer j.mu.Unlock()
	return j.f.Close()
}
