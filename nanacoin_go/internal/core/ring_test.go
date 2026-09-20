package core

import (
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

func TestStatusDistinguishesLifetimeFromRetainedHistory(t *testing.T) {
	s := newService(t, memory.NewDiscarding())
	for i := 0; i < ledger.Capacity+1; i++ {
		_, err := s.book.Append(&ledger.Transaction{
			Kind: ledger.KindIssue,
			Postings: []ledger.Posting{
				{Account: ledger.SystemIssuance, Amount: -1},
				{Account: "account:alice", Amount: 1},
			},
		}, false)
		if err != nil {
			t.Fatal(err)
		}
	}
	st := s.Status()
	if st.Transactions != ledger.Capacity+1 || st.RetainedTransactions != ledger.Capacity || st.OldestTransaction != 2 || st.TransactionCapacity != ledger.Capacity {
		t.Fatalf("incorrect wrapped status: %+v", st)
	}
}
