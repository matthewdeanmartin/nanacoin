//go:build nanacoin_nologs

package eventlog

import (
	"testing"
	"unsafe"
)

// The no-logs build must cost nothing, not merely hide the tab.
//
// The whole point of making Capacity a compile-time constant rather than a
// runtime switch is that the ring is an array inside Log: on this target
// memory is claimed at boot or it is not available later, so a build that
// "disables" logging while still reserving 58 slots would have saved nothing.
// These tests fail if someone reintroduces a runtime flag over a real ring.

func TestDisabledBuildReservesNoRingMemory(t *testing.T) {
	var l Log
	// entries and methods are the two arrays that dominate a live Log.
	if n := unsafe.Sizeof(l.entries); n != 0 {
		t.Errorf("entries array occupies %d bytes, want 0", n)
	}
	if n := unsafe.Sizeof(l.methods); n != 0 {
		t.Errorf("methods array occupies %d bytes, want 0", n)
	}
}

func TestDisabledBuildRecordsNothingAndDoesNotPanic(t *testing.T) {
	l := New(func() int64 { return 0 })

	// Every entry point, on the build where the ring has no slots. The ring
	// arithmetic divides by Capacity, so an unguarded path panics here.
	l.Add(Info, "kind", "detail")
	l.Request(Warn, 404, "GET", "/api/v1/missing")

	if got := l.Recent(0); len(got) != 0 {
		t.Errorf("Recent returned %d events, want 0", len(got))
	}
	l.Each(0, func(Event) bool {
		t.Error("Each yielded an event on a no-logs build")
		return false
	})
}

// A nil Log and a zero-capacity Log must behave identically, because callers
// are written against the nil case and never learn which build they are in.
func TestNilAndDisabledAgree(t *testing.T) {
	var nilLog *Log
	nilLog.Add(Info, "kind", "detail")
	nilLog.Request(Info, 200, "GET", "/")
	if got := nilLog.Recent(0); len(got) != 0 {
		t.Errorf("nil Recent returned %d, want 0", len(got))
	}
	if Enabled {
		t.Error("Enabled is true in a nanacoin_nologs build")
	}
}
