package auth

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"sync"
)

var (
	ErrPKCEMethod   = errors.New("only S256 is supported")
	ErrPKCEMismatch = errors.New("code verifier does not match challenge")
	ErrPKCELength   = errors.New("code verifier must be 43-128 characters")
)

// MethodS256 is the only challenge method accepted. `plain` is refused
// outright (spec 15) rather than supported and discouraged.
const MethodS256 = "S256"

// VerifyPKCE checks a code verifier against the challenge recorded when the
// authorization code was issued.
func VerifyPKCE(method, challenge, verifier string) error {
	if method != MethodS256 {
		return ErrPKCEMethod
	}
	if len(verifier) < 43 || len(verifier) > 128 {
		return ErrPKCELength
	}
	sum := hashString(verifier)
	var computed [43]byte
	base64.RawURLEncoding.Encode(computed[:], sum[:])
	if subtle.ConstantTimeCompare(computed[:], []byte(challenge)) != 1 {
		return ErrPKCEMismatch
	}
	return nil
}

// RandomToken returns n bytes of randomness as an unpadded base64url string.
// Used for authorization codes and access tokens, both of which are opaque -
// there is no JWT here and no reason for one (spec 17).
var tokenWork struct {
	sync.Mutex
	random  [32]byte
	encoded [43]byte
}

func RandomToken(n int) (string, error) {
	if n == 32 {
		tokenWork.Lock()
		defer tokenWork.Unlock()
		if _, err := rand.Read(tokenWork.random[:]); err != nil {
			clear(tokenWork.random[:])
			return "", err
		}
		base64.RawURLEncoding.Encode(tokenWork.encoded[:], tokenWork.random[:])
		token := string(tokenWork.encoded[:])
		clear(tokenWork.random[:])
		clear(tokenWork.encoded[:])
		return token, nil
	}
	if n < 0 {
		return "", errors.New("negative token length")
	}
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}
