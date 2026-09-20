package auth

import (
	"crypto/sha256"
	"encoding/base64"
	"fmt"
	"golang.org/x/crypto/pbkdf2"
	"strings"
	"sync"
	"testing"
)

// Independent implementation is the oracle, including HMAC block boundaries.
func TestPasswordCompatibility(t *testing.T) {
	var salt [16]byte
	for i := range salt {
		salt[i] = byte(i * 13)
	}
	for _, length := range []int{0, 1, 63, 64, 65, 128, 1024} {
		for _, rounds := range []int{1, 2, 1000, 20000} {
			password := strings.Repeat("p", length)
			want := pbkdf2.Key([]byte(password), salt[:], rounds, 32, sha256.New)
			var got [32]byte
			derivePassword(password, &salt, rounds, &got)
			if string(got[:]) != string(want) {
				t.Fatalf("length=%d rounds=%d", length, rounds)
			}
			verifier := fmt.Sprintf("pbkdf2-sha256$%d$%s$%s", rounds, base64.RawStdEncoding.EncodeToString(salt[:]), base64.RawStdEncoding.EncodeToString(want))
			if ok, err := VerifyPassword(verifier, password); !ok || err != nil {
				t.Fatalf("old verifier: %v", err)
			}
			if ok, _ := VerifyPassword(verifier, password+"wrong"); ok {
				t.Fatal("wrong password accepted")
			}
		}
	}
}
func TestMalformedVerifierNeverPanicsOrAuthenticates(t *testing.T) {
	salt := strings.Repeat("A", 22)
	key := strings.Repeat("A", 43)
	for _, v := range []string{"", "x", "pbkdf2-sha256$1$$", "pbkdf2-sha256$0$" + salt + "$" + key, "pbkdf2-sha256$1000001$" + salt + "$" + key, "pbkdf2-sha256$999999999999999999999$" + salt + "$" + key, "pbkdf2-sha256$1$" + salt + "$" + key + "$extra", "pbkdf2-sha256$1$" + salt + "$" + strings.Repeat("!", 43)} {
		if ok, err := VerifyPassword(v, "password"); ok || err != ErrBadVerifier {
			t.Fatalf("%q: %v %v", v, ok, err)
		}
	}
}
func TestPasswordSaltAndConcurrentIsolation(t *testing.T) {
	a, _ := HashPassword("same")
	b, _ := HashPassword("same")
	if a == b {
		t.Fatal("salt reused")
	}
	var wg sync.WaitGroup
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			p := fmt.Sprint(i)
			v, e := HashPassword(p)
			if e != nil {
				t.Error(e)
				return
			}
			for j := 0; j < 5; j++ {
				if ok, e := VerifyPassword(v, p); !ok || e != nil {
					t.Error("workspace cross-contamination")
				}
			}
		}(i)
	}
	wg.Wait()
}
func TestPasswordVerificationAllocationBudget(t *testing.T) {
	v, _ := HashPassword("password")
	if n := testing.AllocsPerRun(20, func() {
		ok, e := VerifyPassword(v, "password")
		if !ok || e != nil {
			panic("verification")
		}
	}); n != 0 {
		t.Fatalf("verify allocs=%v", n)
	}
	if n := testing.AllocsPerRun(20, func() { DummyPasswordCheck("missing") }); n != 0 {
		t.Fatalf("dummy allocs=%v", n)
	}
}
func FuzzVerifierParsing(f *testing.F) {
	valid, _ := hashPasswordWith("test", 1)
	f.Add(valid)
	f.Add("pbkdf2-sha256$1$$")
	f.Add("")
	f.Fuzz(func(t *testing.T, v string) {
		if len(v) > 200 {
			return
		} // avoid expensive valid work factors during parser fuzzing
		parts := strings.Split(v, "$")
		if len(parts) == 4 {
			parts[1] = "1"
			v = strings.Join(parts, "$")
		}
		_, _ = VerifyPassword(v, "test")
	})
}

func TestFixedSHAMatchesStandard(t *testing.T) {
	for n := 0; n < 4096; n++ {
		data := make([]byte, n)
		for i := range data {
			data[i] = byte(i*17 + n)
		}
		want := sha256.Sum256(data)
		var got [32]byte
		shaInto(data, &got)
		if got != want {
			t.Fatalf("SHA finalization length %d", n)
		}
	}
}
