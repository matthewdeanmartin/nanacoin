//go:build !nanacoin_nologs

package eventlog

import (
	"strings"
	"testing"
)

func fixedClock() func() int64 {
	var t int64
	return func() int64 {
		t++
		return t
	}
}

func TestRecentIsNewestFirst(t *testing.T) {
	l := New(fixedClock())
	l.Add(Info, "200", "GET /a")
	l.Add(Warn, "403", "POST /b")
	l.Add(Error, "500", "GET /c")

	got := l.Recent(0)
	if len(got) != 3 {
		t.Fatalf("got %d events, want 3", len(got))
	}
	if got[0].Detail != "GET /c" || got[2].Detail != "GET /a" {
		t.Errorf("order is %q, %q, %q; want newest first",
			got[0].Detail, got[1].Detail, got[2].Detail)
	}
	if got[0].Level != "error" || got[1].Level != "warn" || got[2].Level != "info" {
		t.Errorf("levels are %q, %q, %q", got[0].Level, got[1].Level, got[2].Level)
	}
}

// The sequence number is the ordering that works even on a board with no
// clock, so it must be monotonic regardless of timestamps.
func TestSequenceIsMonotonic(t *testing.T) {
	l := New(nil) // nil clock: every timestamp is zero
	for i := 0; i < 5; i++ {
		l.Add(Info, "200", "GET /x")
	}
	got := l.Recent(0)
	for i := range got {
		want := uint64(len(got) - i)
		if got[i].Seq != want {
			t.Errorf("event %d has seq %d, want %d", i, got[i].Seq, want)
		}
		if got[i].At != 0 {
			t.Errorf("nil clock produced timestamp %d", got[i].At)
		}
	}
}

// The ring is the whole point: a board running for a month must use the same
// memory as one that just booted.
func TestRingDropsOldestAndStaysBounded(t *testing.T) {
	l := New(fixedClock())
	for i := 0; i < Capacity*3; i++ {
		l.Add(Info, "200", "req-"+Itoa(i))
	}

	got := l.Recent(0)
	if len(got) != Capacity {
		t.Fatalf("got %d events, want exactly %d", len(got), Capacity)
	}
	// Newest is the last one added.
	if want := "req-" + Itoa(Capacity*3-1); got[0].Detail != want {
		t.Errorf("newest is %q, want %q", got[0].Detail, want)
	}
	// Oldest retained is Capacity back from the newest.
	if want := "req-" + Itoa(Capacity*2); got[len(got)-1].Detail != want {
		t.Errorf("oldest retained is %q, want %q", got[len(got)-1].Detail, want)
	}
	// Count keeps rising even though Recent is capped, so a reader can tell
	// that events were dropped.
	if l.Count() != uint64(Capacity*3) {
		t.Errorf("Count is %d, want %d", l.Count(), Capacity*3)
	}
}

func TestRecentRespectsLimit(t *testing.T) {
	l := New(fixedClock())
	for i := 0; i < 10; i++ {
		l.Add(Info, "200", "req-"+Itoa(i))
	}
	if got := l.Recent(3); len(got) != 3 {
		t.Errorf("Recent(3) returned %d events", len(got))
	}
	// A limit larger than the number recorded returns what there is, not
	// padding.
	if got := l.Recent(100); len(got) != 10 {
		t.Errorf("Recent(100) returned %d events, want 10", len(got))
	}
}

func TestPartiallyFilledRingReturnsOnlyWhatWasAdded(t *testing.T) {
	l := New(fixedClock())
	l.Add(Info, "200", "only-one")

	got := l.Recent(0)
	if len(got) != 1 {
		t.Fatalf("got %d events, want 1 - empty slots must not be returned", len(got))
	}
	if got[0].Detail != "only-one" {
		t.Errorf("got %q", got[0].Detail)
	}
}

func TestEmptyLog(t *testing.T) {
	l := New(fixedClock())
	if got := l.Recent(0); len(got) != 0 {
		t.Errorf("a fresh log returned %d events", len(got))
	}
	if l.Count() != 0 {
		t.Errorf("a fresh log has count %d", l.Count())
	}
}

// A long detail is clipped rather than refused: a truncated reason is still a
// reason, and refusing would lose the event entirely.
func TestOverLongDetailIsTruncated(t *testing.T) {
	l := New(fixedClock())
	l.Add(Warn, "400", strings.Repeat("x", MaxDetail*2))

	got := l.Recent(1)
	if len(got) != 1 {
		t.Fatal("event was not recorded")
	}
	if len(got[0].Detail) != MaxDetail {
		t.Errorf("detail is %d bytes, want %d", len(got[0].Detail), MaxDetail)
	}
}

// A nil log is a working no-op so that callers never have to check.
func TestNilLogIsSafe(t *testing.T) {
	var l *Log
	l.Add(Info, "200", "GET /x") // must not panic
	if got := l.Recent(0); got != nil {
		t.Errorf("nil log returned %v", got)
	}
	if l.Count() != 0 {
		t.Errorf("nil log has count %d", l.Count())
	}
}

func TestConcurrentAddIsSafe(t *testing.T) {
	l := New(fixedClock())
	const writers = 8
	const each = 50

	done := make(chan struct{})
	for w := 0; w < writers; w++ {
		go func() {
			defer func() { done <- struct{}{} }()
			for i := 0; i < each; i++ {
				l.Add(Info, "200", "concurrent")
			}
		}()
	}
	for w := 0; w < writers; w++ {
		<-done
	}

	if want := uint64(writers * each); l.Count() != want {
		t.Errorf("Count is %d, want %d", l.Count(), want)
	}
	if got := l.Recent(0); len(got) != Capacity {
		t.Errorf("got %d events, want %d", len(got), Capacity)
	}
}
