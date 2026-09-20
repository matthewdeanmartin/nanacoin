package auth

import (
	"crypto/sha256"
	"encoding/base64"
	"fmt"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func testStore() (*Store, *time.Time) {
	now := time.Unix(1000, 0)
	return NewStore(Options{Now: func() time.Time { return now }}), &now
}
func challenge(v string) string {
	s := sha256.Sum256([]byte(v))
	return base64.RawURLEncoding.EncodeToString(s[:])
}
func TestSessionCapacityExpiryAndValueIsolation(t *testing.T) {
	s, now := testStore()
	var first string
	for i := 0; i < MaxSessions; i++ {
		token, _, e := s.NewSession("alice")
		if e != nil {
			t.Fatal(e)
		}
		if i == 0 {
			first = token
		}
	}
	if _, _, e := s.NewSession("bob"); e != ErrTooManySessions {
		t.Fatal("live session evicted", e)
	}
	a, e := s.Lookup(first)
	if e != nil {
		t.Fatal(e)
	}
	a.UserID = "mallory"
	if a, e = s.Lookup(first); e != nil || a.UserID != "alice" {
		t.Fatal("caller changed slot")
	}
	s.Revoke(first)
	if _, _, e = s.NewSession("bob"); e != nil {
		t.Fatal(e)
	}
	if _, e = s.Lookup(first); e != ErrNoSession {
		t.Fatal("revoked token aliases reused slot")
	}
	*now = now.Add(8 * time.Hour)
	if s.Count() != 0 {
		t.Fatal("sessions valid at expiry boundary")
	}
	if _, _, e = s.NewSession("carol"); e != nil {
		t.Fatal(e)
	}
}
func TestFailureWindowResetsAndDoesNotAllocate(t *testing.T) {
	s, now := testStore()
	for i := 0; i < 5; i++ {
		s.RecordFailure("alice")
	}
	if s.CheckRateLimit("alice") != ErrRateLimited {
		t.Fatal("not locked")
	}
	*now = now.Add(5 * time.Minute)
	s.RecordFailure("alice")
	if e := s.CheckRateLimit("alice"); e != nil {
		t.Fatal("expired failures carried into new window")
	}
	s.RecordSuccess("alice")
	if n := testing.AllocsPerRun(100, func() { s.RecordFailure("alice"); s.CheckRateLimit("alice"); s.RecordSuccess("alice") }); n != 0 {
		t.Fatalf("failure churn allocs=%v", n)
	}
	for i := 0; i < MaxFailEntries*4; i++ {
		s.RecordFailure(fmt.Sprint(i))
	}
}
func TestCodeConsumptionBindingCapacityAndExpiry(t *testing.T) {
	for _, mode := range []string{"wrong-verifier", "wrong-redirect", "expiry", "revocation"} {
		t.Run(mode, func(t *testing.T) {
			s, now := testStore()
			v := strings.Repeat("v", 43)
			c, e := s.IssueCode("alice", challenge(v), MethodS256, "/callback")
			if e != nil {
				t.Fatal(e)
			}
			redirect := "/callback"
			switch mode {
			case "wrong-verifier":
				v = strings.Repeat("x", 43)
			case "wrong-redirect":
				redirect = "/evil"
			case "expiry":
				*now = now.Add(time.Minute)
			case "revocation":
				s.RevokeUser("alice")
			}
			if _, _, e = s.RedeemCode(c, v, redirect); e == nil {
				t.Fatal("invalid code accepted")
			}
			if _, _, e = s.RedeemCode(c, strings.Repeat("v", 43), "/callback"); e != ErrNoCode {
				t.Fatal("code not consumed", e)
			}
		})
	}
	s, now := testStore()
	v := strings.Repeat("v", 43)
	for i := 0; i < MaxCodes; i++ {
		if _, e := s.IssueCode("alice", challenge(v), MethodS256, "/"); e != nil {
			t.Fatal(e)
		}
	}
	if _, e := s.IssueCode("bob", challenge(v), MethodS256, "/"); e != ErrTooManySessions {
		t.Fatal(e)
	}
	*now = now.Add(time.Minute)
	if _, e := s.IssueCode("bob", challenge(v), MethodS256, "/"); e != nil {
		t.Fatal("expired slots unavailable", e)
	}
}
func TestConcurrentCodeRedemptionExactlyOnce(t *testing.T) {
	s := NewStore(Options{})
	v := strings.Repeat("v", 43)
	c, _ := s.IssueCode("alice", challenge(v), MethodS256, "/")
	var wins atomic.Int32
	var wg sync.WaitGroup
	for i := 0; i < 32; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, _, e := s.RedeemCode(c, v, "/"); e == nil {
				wins.Add(1)
			} else if e != ErrNoCode {
				t.Error(e)
			}
		}()
	}
	wg.Wait()
	if wins.Load() != 1 || s.Count() != 1 {
		t.Fatal("code redeemed more than once")
	}
}
func TestConcurrentSessionChurnAndRevocation(t *testing.T) {
	s := NewStore(Options{})
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 100; j++ {
				token, _, e := s.NewSession("alice")
				if e != nil {
					t.Error(e)
					return
				}
				if _, e = s.Lookup(token); e != nil {
					t.Error(e)
				}
				s.Revoke(token)
				s.RecordFailure("alice")
				s.RecordSuccess("alice")
			}
		}()
	}
	wg.Wait()
	if s.Count() != 0 {
		t.Fatal("sessions leaked")
	}
	token, _, _ := s.NewSession("alice")
	if n := testing.AllocsPerRun(100, func() {
		var out Session
		if e := s.LookupInto(token, &out); e != nil {
			panic(e)
		}
	}); n != 0 {
		t.Fatalf("lookup allocs=%v", n)
	}
}

func TestRevocationRacingRedemptionCannotLeaveSession(t *testing.T) {
	for i := 0; i < 100; i++ {
		s := NewStore(Options{})
		v := strings.Repeat("v", 43)
		code, _ := s.IssueCode("alice", challenge(v), MethodS256, "/")
		var wg sync.WaitGroup
		start := make(chan struct{})
		wg.Add(2)
		go func() { defer wg.Done(); <-start; s.RedeemCode(code, v, "/") }()
		go func() { defer wg.Done(); <-start; s.RevokeUser("alice") }()
		close(start)
		wg.Wait()
		if s.Count() != 0 {
			t.Fatal("revoked pending code recreated a session")
		}
	}
}
func TestTokenEntropyAndAllocationBudget(t *testing.T) {
	seen := make(map[string]bool)
	for i := 0; i < 500; i++ {
		v, e := RandomToken(32)
		if e != nil {
			t.Fatal(e)
		}
		if len(v) != 43 || seen[v] {
			t.Fatal("token reused or truncated")
		}
		seen[v] = true
	}
	if n := testing.AllocsPerRun(100, func() {
		v, e := RandomToken(32)
		if e != nil || len(v) != 43 {
			panic("token")
		}
	}); n > 1 {
		t.Fatalf("token allocations=%v", n)
	}
}

func TestConfiguredPoolsRefuseLiveEvictionAndReuseSlots(t *testing.T) {
	s := NewStore(Options{SessionCapacity: 2, CodeCapacity: 1, FailureCapacity: 2})
	first, _, _ := s.NewSession("alice")
	s.NewSession("bob")
	if _, _, e := s.NewSession("carol"); e != ErrTooManySessions {
		t.Fatal("configured ceiling ignored")
	}
	if _, e := s.Lookup(first); e != nil {
		t.Fatal("live slot overwritten")
	}
	s.Revoke(first)
	if _, _, e := s.NewSession("carol"); e != nil {
		t.Fatal("revoked slot not reusable", e)
	}
	code, _ := s.IssueCode("alice", challenge(strings.Repeat("v", 43)), MethodS256, "/")
	if _, e := s.IssueCode("bob", "challenge", MethodS256, "/"); e != ErrTooManySessions {
		t.Fatal("code ceiling ignored")
	}
	s.RedeemCode(code, strings.Repeat("x", 43), "/")
	if _, e := s.IssueCode("bob", "challenge", MethodS256, "/"); e != nil {
		t.Fatal("consumed code slot not reusable")
	}
}

func TestMixedPasswordAndSessionHashingCannotCrossContaminate(t *testing.T) {
	s := NewStore(Options{})
	v, _ := hashPasswordWith("pin-1234", 2)
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 30; j++ {
				token, _, e := s.NewSession("alice")
				if e != nil {
					t.Error(e)
					return
				}
				if ok, e := VerifyPassword(v, "pin-1234"); !ok || e != nil {
					t.Error("shared hash corrupted password")
				}
				var out Session
				if e := s.LookupInto(token, &out); e != nil || out.UserID != "alice" {
					t.Error("shared hash corrupted token")
				}
				s.Revoke(token)
			}
		}()
	}
	wg.Wait()
}
