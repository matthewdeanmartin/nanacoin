//go:build !nanacoin_nologs

package eventlog

import (
	"sync"
	"testing"
)

func TestConcurrentWrapHasNoMissingOrDuplicateSequences(t *testing.T) {
	l := New(nil)
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 200; j++ {
				l.Add(Info, "test", "event")
				snapshot := l.Recent(0)
				for k := 1; k < len(snapshot); k++ {
					if snapshot[k-1].Seq != snapshot[k].Seq+1 {
						t.Error("torn snapshot")
						return
					}
				}
			}
		}()
	}
	wg.Wait()
	if l.Count() != 1600 {
		t.Fatal("lost events")
	}
	snapshot := l.Recent(0)
	snapshot[0].Detail = "mutated"
	if l.Recent(1)[0].Detail != "event" {
		t.Fatal("snapshot aliases live ring")
	}
}
