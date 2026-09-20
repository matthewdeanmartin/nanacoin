// Package auth handles "who is making this request" - password verifiers,
// PKCE authorization codes, and bearer sessions.
//
// Authorization ("what may this person do") lives in the service layer, not
// here; the spec keeps the two ideas separate and so does the code.
package auth

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"strconv"
	"strings"
	"sync"
)

// DefaultIterations preserves the existing password work factor. Allocation
// changes must not silently change password strength or invalidate verifiers.
const DefaultIterations = 1000

const saltLen = 16
const keyLen = 32

var ErrBadVerifier = errors.New("malformed password verifier")

// Strict clones a 328-byte Encoding on ESP32; construct it once, not per login.
var verifierBase64 = base64.RawStdEncoding.Strict()

// HashPassword produces a verifier string of the form
//
//	pbkdf2-sha256$<iterations>$<base64 salt>$<base64 key>
//
// The parameters travel with the hash so that raising the work factor later
// does not invalidate existing passwords.
func HashPassword(password string) (string, error) {
	return hashPasswordWith(password, DefaultIterations)
}

// One small workspace bounds concurrent hashing memory. Hashing holds only this
// mutex; callers must not use this workspace for other service operations.
var passwordWork struct {
	sync.Mutex
	inner [128]byte
	outer [96]byte
	u     [32]byte
	h     [32]byte
}

// derivePassword implements the single 32-byte block of PBKDF2-HMAC-SHA256
// used by our verifier format. SHA-256 uses Go's generic compression code with
// reusable scratch (sha256_fixed.go and sha256block.go).
// Fixed messages avoid hash interfaces, marshalled HMAC state and per-round
// allocations (which differ substantially between Go and TinyGo).
func derivePassword(password string, salt *[saltLen]byte, iterations int, out *[keyLen]byte) {
	passwordWork.Lock()
	defer passwordWork.Unlock()
	w := &passwordWork
	clear(w.inner[:])
	clear(w.outer[:])
	if len(password) > 64 {
		key := hashString(password)
		copy(w.inner[:64], key[:])
	} else {
		copy(w.inner[:64], password)
	}
	for i := 0; i < 64; i++ {
		w.outer[i] = w.inner[i] ^ 0x5c
		w.inner[i] ^= 0x36
	}
	copy(w.inner[64:], salt[:])
	w.inner[83] = 1 // salt || big-endian block index 1
	shaInto(w.inner[:84], &w.h)
	copy(w.outer[64:], w.h[:])
	shaInto(w.outer[:], &w.u)
	copy(out[:], w.u[:])
	for n := 1; n < iterations; n++ {
		copy(w.inner[64:], w.u[:])
		shaInto(w.inner[:96], &w.h)
		copy(w.outer[64:], w.h[:])
		shaInto(w.outer[:], &w.u)
		for i := range out {
			out[i] ^= w.u[i]
		}
	}
	clear(w.inner[:])
	clear(w.outer[:])
	clear(w.u[:])
	clear(w.h[:])
}

func hashPasswordWith(password string, iters int) (string, error) {
	if iters < 1 || iters > maxIterations {
		return "", ErrBadVerifier
	}
	var salt [saltLen]byte
	if _, err := rand.Read(salt[:]); err != nil {
		return "", err
	}
	var key [keyLen]byte
	derivePassword(password, &salt, iters, &key)
	var buf [100]byte
	out := append(buf[:0], "pbkdf2-sha256$"...)
	out = strconv.AppendInt(out, int64(iters), 10)
	out = append(out, '$')
	out = base64.RawStdEncoding.AppendEncode(out, salt[:])
	out = append(out, '$')
	out = base64.RawStdEncoding.AppendEncode(out, key[:])
	return string(out), nil
}

// Bound corrupted verifier CPU cost while accepting the previous 20,000 rounds.
const maxIterations = 1000000

func VerifyPassword(verifier, password string) (bool, error) {
	rest, ok := strings.CutPrefix(verifier, "pbkdf2-sha256$")
	if !ok {
		return false, ErrBadVerifier
	}
	rounds, rest, ok := strings.Cut(rest, "$")
	if !ok {
		return false, ErrBadVerifier
	}
	iters, err := strconv.Atoi(rounds)
	if err != nil || iters < 1 || iters > maxIterations {
		return false, ErrBadVerifier
	}
	saltText, keyText, ok := strings.Cut(rest, "$")
	if !ok || len(saltText) != 22 || len(keyText) != 43 {
		return false, ErrBadVerifier
	}
	var salt [saltLen]byte
	var want [keyLen]byte
	n, err := verifierBase64.Decode(salt[:], []byte(saltText))
	if err != nil || n != saltLen {
		return false, ErrBadVerifier
	}
	n, err = verifierBase64.Decode(want[:], []byte(keyText))
	if err != nil || n != keyLen {
		return false, ErrBadVerifier
	}
	var got [keyLen]byte
	derivePassword(password, &salt, iters, &got)
	return subtle.ConstantTimeCompare(got[:], want[:]) == 1, nil
}

// DummyPasswordCheck keeps missing-user work comparable without generating a
// random salt or allocating a verifier that is immediately discarded.
func DummyPasswordCheck(password string) {
	var salt [saltLen]byte
	var discarded [keyLen]byte
	derivePassword(password, &salt, DefaultIterations, &discarded)
}
