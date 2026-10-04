package flashlog

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
)

func open(t *testing.T, path string, capacity int64) *Journal {
	t.Helper()
	j, err := Open(path, capacity)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	t.Cleanup(func() { j.Close() })
	return j
}

func replayAll(t *testing.T, j *Journal) []*storage.Record {
	t.Helper()
	var got []*storage.Record
	if err := j.Replay(func(r *storage.Record) error {
		got = append(got, r)
		return nil
	}); err != nil {
		t.Fatalf("Replay: %v", err)
	}
	return got
}

func TestAppendAndReplay(t *testing.T) {
	path := filepath.Join(t.TempDir(), "journal.log")
	j := open(t, path, 0)

	for i := 1; i <= 5; i++ {
		seq, err := j.Append(storage.TypeTransactionCreated, []byte(fmt.Sprintf(`{"n":%d}`, i)))
		if err != nil {
			t.Fatalf("Append %d: %v", i, err)
		}
		if seq != uint64(i) {
			t.Errorf("Append %d returned sequence %d", i, seq)
		}
	}

	got := replayAll(t, j)
	if len(got) != 5 {
		t.Fatalf("replayed %d records, want 5", len(got))
	}
	for i, r := range got {
		if r.Sequence != uint64(i+1) {
			t.Errorf("record %d has sequence %d", i, r.Sequence)
		}
	}
}

// Reopening is the reboot path: a fresh Journal must recover its end offset
// and sequence from the file alone, then keep appending without a gap.
func TestReopenResumesSequence(t *testing.T) {
	path := filepath.Join(t.TempDir(), "journal.log")

	j1 := open(t, path, 0)
	j1.Append(storage.TypeUserCreated, []byte(`{"a":1}`))
	j1.Append(storage.TypeUserCreated, []byte(`{"a":2}`))
	j1.Close()

	j2 := open(t, path, 0)
	if got := replayAll(t, j2); len(got) != 2 {
		t.Fatalf("after reopen replayed %d records, want 2", len(got))
	}
	seq, err := j2.Append(storage.TypeUserCreated, []byte(`{"a":3}`))
	if err != nil {
		t.Fatalf("Append after reopen: %v", err)
	}
	if seq != 3 {
		t.Errorf("resumed at sequence %d, want 3", seq)
	}

	j2.Close()
	j3 := open(t, path, 0)
	if got := replayAll(t, j3); len(got) != 3 {
		t.Errorf("final replay has %d records, want 3", len(got))
	}
}

// The central crash-safety claim. Truncating the file at every possible byte
// of the last record simulates power loss at that instant. In every case the
// committed prefix must survive intact and the torn record must vanish -
// never a partial transaction, never a corrupted earlier one.
func TestPowerLossAtEveryOffset(t *testing.T) {
	dir := t.TempDir()
	build := func(path string) int64 {
		j, err := Open(path, 0)
		if err != nil {
			t.Fatalf("Open: %v", err)
		}
		defer j.Close()
		for i := 1; i <= 3; i++ {
			if _, err := j.Append(storage.TypeTransactionCreated, []byte(fmt.Sprintf(`{"txn":%d}`, i))); err != nil {
				t.Fatalf("Append: %v", err)
			}
		}
		st, _ := os.Stat(path)
		return st.Size()
	}

	full := filepath.Join(dir, "full.log")
	size := build(full)
	golden, err := os.ReadFile(full)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}

	// One committed record is HeaderSize + payload + CommitSize bytes.
	recSize := size / 3

	for cut := int64(1); cut < size; cut++ {
		path := filepath.Join(dir, fmt.Sprintf("cut%d.log", cut))
		if err := os.WriteFile(path, golden[:cut], 0o600); err != nil {
			t.Fatalf("WriteFile: %v", err)
		}

		j, err := Open(path, 0)
		if err != nil {
			t.Fatalf("Open at cut %d: %v", cut, err)
		}
		got := replayAll(t, j)

		// Only whole records survive: the count is the number of complete
		// records wholly contained in the truncated prefix.
		want := int(cut / recSize)
		if len(got) != want {
			t.Errorf("cut at %d/%d: replayed %d records, want %d", cut, size, len(got), want)
		}
		for i, r := range got {
			if r.Sequence != uint64(i+1) {
				t.Errorf("cut at %d: record %d has sequence %d", cut, i, r.Sequence)
			}
		}

		// Recovery must leave the journal writable, resuming from the last
		// good record rather than from the torn tail.
		seq, err := j.Append(storage.TypeTransactionCreated, []byte(`{"after":"recovery"}`))
		if err != nil {
			t.Fatalf("cut at %d: append after recovery: %v", cut, err)
		}
		if seq != uint64(want+1) {
			t.Errorf("cut at %d: resumed at sequence %d, want %d", cut, seq, want+1)
		}
		j.Close()

		// And the appended record must be readable on the next boot.
		j2, _ := Open(path, 0)
		if got2 := replayAll(t, j2); len(got2) != want+1 {
			t.Errorf("cut at %d: after recovery+append, replayed %d, want %d", cut, len(got2), want+1)
		}
		j2.Close()
	}
}

// Bit rot in a committed record must stop replay there rather than feeding a
// corrupted transaction into the ledger.
func TestCorruptRecordTruncatesHistory(t *testing.T) {
	path := filepath.Join(t.TempDir(), "journal.log")
	j := open(t, path, 0)
	for i := 1; i <= 4; i++ {
		j.Append(storage.TypeTransactionCreated, []byte(fmt.Sprintf(`{"n":%d}`, i)))
	}
	j.Close()

	data, _ := os.ReadFile(path)
	recSize := len(data) / 4
	// Flip a payload bit in the third record.
	data[2*recSize+storage.HeaderSize+3] ^= 0x10
	os.WriteFile(path, data, 0o600)

	j2 := open(t, path, 0)
	got := replayAll(t, j2)
	if len(got) != 2 {
		t.Fatalf("replayed %d records, want 2 (stop at the corrupt third)", len(got))
	}
}

// The invalid tail is dropped from the file, so a journal does not carry its
// debris forward forever.
func TestReplayTruncatesInvalidTail(t *testing.T) {
	path := filepath.Join(t.TempDir(), "journal.log")
	j := open(t, path, 0)
	j.Append(storage.TypeUserCreated, []byte(`{"a":1}`))
	j.Close()

	data, _ := os.ReadFile(path)
	good := len(data)
	os.WriteFile(path, append(data, []byte("garbage that is not a record")...), 0o600)

	j2 := open(t, path, 0)
	replayAll(t, j2)
	j2.Close()

	st, _ := os.Stat(path)
	if st.Size() != int64(good) {
		t.Errorf("file is %d bytes after replay, want %d (tail not truncated)", st.Size(), good)
	}
}

func TestCapacityEnforced(t *testing.T) {
	path := filepath.Join(t.TempDir(), "journal.log")
	// Room for roughly two small records and no more.
	j := open(t, path, int64(2*(storage.HeaderSize+storage.CommitSize+8)))

	if _, err := j.Append(storage.TypeUserCreated, []byte(`{"a":1}`)); err != nil {
		t.Fatalf("first append: %v", err)
	}
	if _, err := j.Append(storage.TypeUserCreated, []byte(`{"a":2}`)); err != nil {
		t.Fatalf("second append: %v", err)
	}
	if _, err := j.Append(storage.TypeUserCreated, []byte(`{"a":3}`)); !errors.Is(err, ErrFull) {
		t.Errorf("third append: got %v, want ErrFull", err)
	}

	// A full journal must still be readable. Losing read access to the
	// ledger because it filled would be the worst possible failure.
	if got := replayAll(t, j); len(got) != 2 {
		t.Errorf("replayed %d records from a full journal, want 2", len(got))
	}
}

// A sequence gap means a record was lost. Replay must stop rather than
// continue with a hole in the money.
func TestSequenceGapStopsReplay(t *testing.T) {
	path := filepath.Join(t.TempDir(), "journal.log")

	f, _ := os.Create(path)
	var off int64
	for _, seq := range []uint64{1, 2, 4} { // 3 is missing
		buf, _ := storage.Encode(&storage.Record{
			Type: storage.TypeTransactionCreated, Sequence: seq, Payload: []byte(`{}`),
		})
		f.WriteAt(buf, off)
		off += int64(len(buf))
	}
	f.Close()

	j := open(t, path, 0)
	if got := replayAll(t, j); len(got) != 2 {
		t.Errorf("replayed %d records, want 2 (stop at the gap)", len(got))
	}
}
