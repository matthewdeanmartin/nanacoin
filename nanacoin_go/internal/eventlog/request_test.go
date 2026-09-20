//go:build !nanacoin_nologs

package eventlog

import (
	"strings"
	"testing"
)

func TestRequestLogStaysReadableAcrossWrap(t *testing.T) {
	l := New(nil)
	l.Request(Info, 201, "POST", "/api/v1/transfers")
	snapshot := l.Recent(1)
	for i := 0; i < Capacity; i++ {
		l.Add(Error, "journal_failed", "cannot append transaction")
	}
	if snapshot[0].Kind != "201" || snapshot[0].Detail != "POST /api/v1/transfers" {
		t.Fatal(snapshot)
	}
	for _, e := range l.Recent(0) {
		if e.Detail != "cannot append transaction" {
			t.Fatal("method leaked on slot reuse", e)
		}
	}
	l.Request(Warn, 413, "POST", strings.Repeat("x", 200))
	e := l.Recent(1)[0]
	if len(e.Detail) != MaxDetail || !strings.HasPrefix(e.Detail, "POST ") {
		t.Fatal(e)
	}
}

func TestRecordRequestDoesNotAllocate(t *testing.T) {
	l := New(nil)
	if n := testing.AllocsPerRun(100, func() { l.Request(Info, 201, "POST", "/api/v1/transfers") }); n != 0 {
		t.Fatal(n)
	}
}
