package core

import (
	"errors"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
	"testing"
)

func TestFailedPasswordChangePreservesCredentialsAcrossReplay(t *testing.T) {
	j := memory.New()
	s := newService(t, j)
	nana, alice, _ := household(t, s)
	replacement := "replacement-pin"
	boom := errors.New("write failed")
	j.SetFailure(j.Appends(), boom)
	if _, e := s.UpdateUser(nana, alice.ID, nil, nil, nil, &replacement); !errors.Is(e, boom) {
		t.Fatal(e)
	}
	for _, svc := range []*Service{s, newService(t, j)} {
		if _, e := svc.Authenticate("alice", "alice-pin"); e != nil {
			t.Fatal("failed update destroyed original credential", e)
		}
		if _, e := svc.Authenticate("alice", replacement); e == nil {
			t.Fatal("uncommitted password took effect")
		}
	}
	j.SetFailure(0, nil)
	if _, e := s.UpdateUser(nana, alice.ID, nil, nil, nil, &replacement); e != nil {
		t.Fatal(e)
	}
	for _, svc := range []*Service{s, newService(t, j)} {
		if _, e := svc.Authenticate("alice", replacement); e != nil {
			t.Fatal(e)
		}
		if _, e := svc.Authenticate("alice", "alice-pin"); e == nil {
			t.Fatal("old password still works")
		}
	}
}
