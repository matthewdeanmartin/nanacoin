package memory

import (
	"errors"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"testing"
)

func TestFailureDoesNotConsumeSequenceOrBytes(t *testing.T) {
	for _, discard := range []bool{false, true} {
		j := New()
		j.Discard = discard
		payload := []byte("original")
		if n, e := j.Append(storage.TypeSnapshot, payload); n != 1 || e != nil {
			t.Fatal(n, e)
		}
		before, _ := j.Size()
		boom := errors.New("power failed")
		j.SetFailure(j.Appends(), boom)
		if _, e := j.Append(storage.TypeSnapshot, payload); e != boom {
			t.Fatal(e)
		}
		after, _ := j.Size()
		if before != after {
			t.Fatal("failed append consumed space")
		}
		j.SetFailure(0, nil)
		if n, e := j.Append(storage.TypeSnapshot, payload); n != 2 || e != nil {
			t.Fatal("failed append consumed sequence", n, e)
		}
		payload[0] = 'X'
		seen := 0
		if e := j.Replay(func(r *storage.Record) error {
			seen++
			if string(r.Payload) != "original" {
				t.Fatal("journal aliases caller buffer")
			}
			return nil
		}); e != nil {
			t.Fatal(e)
		}
		if discard && seen != 0 || !discard && seen != 2 {
			t.Fatal("retention", seen)
		}
	}
}
func TestReplayCallbackCanAppendAndStopsOnError(t *testing.T) {
	j := New()
	j.Append(storage.TypeSnapshot, []byte("first"))
	sentinel := errors.New("stop")
	seen := 0
	e := j.Replay(func(r *storage.Record) error {
		seen++
		if _, e := j.Append(storage.TypeSnapshot, []byte("second")); e != nil {
			t.Fatal(e)
		}
		return sentinel
	})
	if e != sentinel || seen != 1 {
		t.Fatal("snapshot/error contract", e, seen)
	}
}

func TestDiscardSizePathValidationFailureAndNoAllocation(t *testing.T) {
	j := NewDiscarding()
	for _, n := range []int{-1, storage.MaxPayload + 1} {
		if _, err := j.AppendDiscarded(storage.TypeSnapshot, n); err != storage.ErrTooLarge {
			t.Fatal(n, err)
		}
	}
	if seq, err := j.AppendDiscarded(storage.TypeSnapshot, 123); err != nil || seq != 1 {
		t.Fatal(seq, err)
	}
	before, _ := j.Size()
	boom := errors.New("journal refused")
	j.SetFailure(j.Appends(), boom)
	if _, err := j.AppendDiscarded(storage.TypeSnapshot, 12); err != boom {
		t.Fatal(err)
	}
	after, _ := j.Size()
	if before != after {
		t.Fatal("failed append advanced bytes")
	}
	j.SetFailure(0, nil)
	if seq, err := j.AppendDiscarded(storage.TypeSnapshot, 123); err != nil || seq != 2 {
		t.Fatal(seq, err)
	}
	if n := testing.AllocsPerRun(100, func() { _, _ = j.AppendDiscarded(storage.TypeSnapshot, 123) }); n != 0 {
		t.Fatal(n)
	}
	retained := New()
	if _, err := retained.AppendDiscarded(storage.TypeSnapshot, 1); err == nil {
		t.Fatal("persistent bytes silently discarded")
	}
}
