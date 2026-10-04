package core

import (
	"errors"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

func TestReceiptFailureStillDeduplicatesInRAM(t *testing.T) {
	j := memory.New()
	s := newService(t, j)
	nana, alice, bob := household(t, s)
	if _, err := s.Issue(nana, alice.Account, 10, "fund"); err != nil {
		t.Fatal(err)
	}
	j.SetFailure(j.Appends()+1, errors.New("receipt write failed"))
	calls := 0
	for i := 0; i < 2; i++ {
		_, err := s.Idempotent(alice.ID, "transfer", "receipt-fail", func() ([]byte, error) {
			calls++
			_, err := s.Transfer(alice, bob.Account, 1, "once")
			return []byte("ok"), err
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	if calls != 1 {
		t.Fatal("receipt failure allowed duplicate execution")
	}
}

func TestConcurrentRetriesExecuteOnce(t *testing.T) {
	s := newService(t, memory.New())
	nana, alice, bob := household(t, s)
	if _, err := s.Issue(nana, alice.Account, 100, "fund"); err != nil {
		t.Fatal(err)
	}
	before := s.Balance(alice.Account)
	var calls atomic.Int32
	var wg sync.WaitGroup
	start := make(chan struct{})
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			<-start
			result, err := s.Idempotent(alice.ID, "transfer", "same", func() ([]byte, error) {
				calls.Add(1)
				time.Sleep(10 * time.Millisecond)
				_, err := s.Transfer(alice, bob.Account, 1, "once")
				return []byte("receipt"), err
			})
			if err != nil || string(result) != "receipt" {
				t.Errorf("result %s: %v", result, err)
			}
		}()
	}
	close(start)
	wg.Wait()
	if calls.Load() != 1 || s.Balance(alice.Account) != before-1 {
		t.Fatal("duplicate money movement")
	}
}

func TestStreamSendAllowsWritesAndSkipsOverwrittenHistory(t *testing.T) {
	s := newService(t, memory.NewDiscarding())
	nana, alice, _ := household(t, s)
	for i := 0; i < 10; i++ {
		if _, err := s.Issue(nana, alice.Account, 1, "old"); err != nil {
			t.Fatal(err)
		}
	}
	entered, release, done := make(chan struct{}), make(chan struct{}), make(chan struct{})
	var releaseOnce sync.Once
	unblock := func() { releaseOnce.Do(func() { close(release) }) }
	defer unblock()
	seen := 0
	go func() {
		defer close(done)
		s.EachTxnRender("", 30, func(r *TxnRender) bool { seen++; return true }, func() bool {
			if seen == 1 {
				close(entered)
				<-release
			}
			return true
		})
	}()
	<-entered
	written := make(chan error, 1)
	go func() {
		for i := 0; i < ledger.Capacity+1; i++ {
			if _, err := s.Issue(nana, alice.Account, 1, "new"); err != nil {
				written <- err
				return
			}
		}
		written <- nil
	}()
	select {
	case err := <-written:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("socket send holds service lock")
	}
	unblock()
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("stream did not finish")
	}
	if seen != 1 {
		t.Fatalf("read appended or overwritten records: %d", seen)
	}
}
