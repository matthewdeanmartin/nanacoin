package auth

import (
	"crypto/subtle"
	"encoding/base64"
	"errors"
	"sync"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
)

var (
	ErrNoSession        = errors.New("invalid or expired token")
	ErrNoCode           = errors.New("invalid or expired authorization code")
	ErrCodeUsed         = errors.New("authorization code already redeemed")
	ErrRedirectMismatch = errors.New("redirect_uri does not match")
	ErrRateLimited      = errors.New("too many failed attempts")
	ErrTooManySessions  = errors.New("too many active sessions")
)

// Session is a bearer token grant. Sessions live in RAM only and die at
// reboot, which the spec recommends (17) and which suits flash: a household
// logging in again after a power cut is a smaller cost than writing to flash
// on every login.
type Session struct {
	UserID    ledger.UserID
	CreatedAt time.Time
	ExpiresAt time.Time
}

// Slots retain hashes rather than request strings. Returned sessions are
// independent snapshots: callers cannot mutate a live slot or observe reuse.
type sessionSlot struct {
	key              [32]byte
	user             ledger.UserID
	created, expires int64
	used             bool
}
type codeSlot struct {
	key, challenge, redirect [32]byte
	user                     ledger.UserID
	expires                  int64
	used                     bool
}
type failureSlot struct {
	key   [32]byte
	until int64
	count int
}

const (
	MaxSessions    = 64
	MaxCodes       = 16
	MaxFailEntries = 64
)

type Store struct {
	mu                sync.Mutex
	sessions          []sessionSlot
	codes             []codeSlot
	fails             []failureSlot
	tokenTTL, codeTTL time.Duration
	now               func() time.Time
	maxFails          int
	lockoutFor        time.Duration
}
type Options struct {
	// Zero selects the package ceiling. Board builds reserve smaller pools.
	SessionCapacity, CodeCapacity, FailureCapacity int
	TokenTTL, CodeTTL                              time.Duration
	MaxFails                                       int
	LockoutFor                                     time.Duration
	Now                                            func() time.Time
}

//go:noinline
func NewStore(o Options) *Store {
	if o.TokenTTL == 0 {
		o.TokenTTL = 8 * time.Hour
	}
	if o.CodeTTL == 0 {
		o.CodeTTL = time.Minute
	}
	if o.MaxFails == 0 {
		o.MaxFails = 5
	}
	if o.LockoutFor == 0 {
		o.LockoutFor = 5 * time.Minute
	}
	if o.Now == nil {
		o.Now = time.Now
	}
	return &Store{sessions: make([]sessionSlot, boundedCapacity(o.SessionCapacity, MaxSessions)), codes: make([]codeSlot, boundedCapacity(o.CodeCapacity, MaxCodes)), fails: make([]failureSlot, boundedCapacity(o.FailureCapacity, MaxFailEntries)), tokenTTL: o.TokenTTL, codeTTL: o.CodeTTL, now: o.Now, maxFails: o.MaxFails, lockoutFor: o.LockoutFor}
}

// Keep the rate-limit operations out of their callers on Xtensa: inlining
// these alongside password verification triggers LLVM 22 register-scavenging
// failure. Keep the boundaries when changing this path.
//
//go:noinline
func (s *Store) CheckRateLimit(key string) error {
	hash := hashString(key)
	s.mu.Lock()
	defer s.mu.Unlock()
	now := s.now().UnixNano()
	for i := range s.fails {
		a := &s.fails[i]
		if a.count >= s.maxFails && a.key == hash && now < a.until {
			return ErrRateLimited
		}
	}
	return nil
}

//go:noinline
func (s *Store) RecordFailure(key string) {
	hash := hashString(key)
	s.mu.Lock()
	defer s.mu.Unlock()
	now := s.now().UnixNano()
	slot := 0
	for i := range s.fails {
		a := &s.fails[i]
		if a.count > 0 && a.key == hash {
			if now >= a.until {
				a.count = 0
			}
			if a.count < s.maxFails {
				a.count++
			}
			a.until = now + int64(s.lockoutFor)
			return
		}
		if a.until < s.fails[slot].until {
			slot = i
		}
	}
	// As before, a full failure table evicts the soonest-expiring counter.
	s.fails[slot] = failureSlot{key: hash, until: now + int64(s.lockoutFor), count: 1}
}

//go:noinline
func (s *Store) RecordSuccess(key string) {
	hash := hashString(key)
	s.mu.Lock()
	defer s.mu.Unlock()
	for i := range s.fails {
		if s.fails[i].key == hash {
			s.fails[i] = failureSlot{}
		}
	}
}

//go:noinline
func (s *Store) IssueCode(userID ledger.UserID, challenge, method, redirectURI string) (string, error) {
	if method != MethodS256 {
		return "", ErrPKCEMethod
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	now := s.now().UnixNano()
	for i := range s.codes {
		c := &s.codes[i]
		if c.used && now < c.expires {
			continue
		}
		code, err := RandomToken(32)
		if err != nil {
			return "", err
		}
		// Store each digest before computing the next to keep fewer
		// intermediate values live on Xtensa.
		c.key = hashString(code)
		c.challenge = hashString(challenge)
		c.redirect = hashString(redirectURI)
		c.user = userID
		c.expires = now + int64(s.codeTTL)
		c.used = true
		return code, nil
	}
	return "", ErrTooManySessions
}

//go:noinline
func (s *Store) RedeemCode(code, verifier, redirectURI string) (string, *Session, error) {
	key := hashString(code)
	s.mu.Lock()
	defer s.mu.Unlock()
	var ac codeSlot
	for i := range s.codes {
		if s.codes[i].used && s.codes[i].key == key {
			ac = s.codes[i]
			s.codes[i] = codeSlot{}
			break
		}
	}
	if !ac.used || s.now().UnixNano() >= ac.expires {
		return "", nil, ErrNoCode
	}
	if ac.redirect != hashString(redirectURI) {
		return "", nil, ErrRedirectMismatch
	}
	if len(verifier) < 43 || len(verifier) > 128 {
		return "", nil, ErrPKCELength
	}
	computed := verifierChallengeHash(verifier)
	if subtle.ConstantTimeCompare(computed[:], ac.challenge[:]) != 1 {
		return "", nil, ErrPKCEMismatch
	}
	return s.newSessionLocked(ac.user)
}
func (s *Store) NewSession(userID ledger.UserID) (string, *Session, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.newSessionLocked(userID)
}

// newSessionLocked takes a free or expired slot, and failing that evicts this
// user's own oldest session.
//
// # Why eviction, and why only your own
//
// Sessions are RAM-only and last eight hours, and nothing ever logs out: the
// switcher holds several accounts at once by design, and the demo seeder logs
// in as every household member on each run. So the table fills with sessions
// that are live, unexpired, and unreachable - nobody holds their tokens any
// more - and the board then refuses every login for the rest of the day.
//
// That failed in a way nobody could diagnose. The refusal is a 503 on
// /auth/authorize, but what the user sees is the *next* request failing with
// "invalid or expired token", because the client has already dropped the
// account it could not re-authenticate. The demo seeder would stop a few
// minutes in reporting a dead token, with a full session table as the real
// cause and nothing on screen pointing at it.
//
// Evicting the caller's own oldest session is the conservative form of the
// fix: logging in again as yourself can cost you your stalest session, and
// can never cost anyone else theirs. A browser holding one token per account
// is unaffected - it has one session each - while a repeated login as the
// same person recycles instead of accumulating. Only when *other* users fill
// the table does this still refuse, which is a real capacity limit rather
// than an accumulation of ghosts.
//
//go:noinline
func (s *Store) newSessionLocked(userID ledger.UserID) (string, *Session, error) {
	now := s.now()
	free := -1
	oldest, oldestAt := -1, int64(0)
	for i := range s.sessions {
		slot := &s.sessions[i]
		if !slot.used || now.UnixNano() >= slot.expires {
			if free < 0 {
				free = i
			}
			continue
		}
		if slot.user == userID && (oldest < 0 || slot.created < oldestAt) {
			oldest, oldestAt = i, slot.created
		}
	}
	if free < 0 {
		// The table is full of live sessions. Reuse one of this user's own
		// before refusing; never anyone else's.
		free = oldest
	}
	if free < 0 {
		return "", nil, ErrTooManySessions
	}

	token, err := RandomToken(32)
	if err != nil {
		return "", nil, err
	}
	slot := &s.sessions[free]
	sess := Session{UserID: userID, CreatedAt: now, ExpiresAt: now.Add(s.tokenTTL)}
	*slot = sessionSlot{key: hashString(token), user: userID, created: now.UnixNano(), expires: sess.ExpiresAt.UnixNano(), used: true}
	return token, &sess, nil
}

// Lookup returns an independent copy. Request handlers use LookupInto to avoid
// allocating that copy; neither API exposes mutable pooled storage.
func (s *Store) Lookup(token string) (*Session, error) {
	var out Session
	if err := s.LookupInto(token, &out); err != nil {
		return nil, err
	}
	return &out, nil
}
func (s *Store) LookupInto(token string, out *Session) error {
	*out = Session{}
	key := hashString(token)
	s.mu.Lock()
	defer s.mu.Unlock()
	for i := range s.sessions {
		slot := &s.sessions[i]
		if !slot.used || slot.key != key {
			continue
		}
		if s.now().UnixNano() >= slot.expires {
			*slot = sessionSlot{}
			break
		}
		*out = Session{UserID: slot.user, CreatedAt: time.Unix(0, slot.created), ExpiresAt: time.Unix(0, slot.expires)}
		return nil
	}
	return ErrNoSession
}
func (s *Store) Revoke(token string) {
	key := hashString(token)
	s.mu.Lock()
	defer s.mu.Unlock()
	for i := range s.sessions {
		if s.sessions[i].key == key {
			s.sessions[i] = sessionSlot{}
		}
	}
}
func (s *Store) RevokeUser(userID ledger.UserID) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i := range s.sessions {
		if s.sessions[i].user == userID {
			s.sessions[i] = sessionSlot{}
		}
	}
	// Pending codes must not recreate a revoked user's sessions.
	for i := range s.codes {
		if s.codes[i].user == userID {
			s.codes[i] = codeSlot{}
		}
	}
}
func (s *Store) Count() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	now := s.now().UnixNano()
	n := 0
	for i := range s.sessions {
		slot := &s.sessions[i]
		if slot.used && now >= slot.expires {
			*slot = sessionSlot{}
		}
		if slot.used {
			n++
		}
	}
	return n
}

// TinyGo conservatively escapes even local SHA input arrays. Reuse a small
// shared buffer so ordinary lookups do not allocate on the actual board.
var hashWork struct {
	sync.Mutex
	input   [128]byte
	encoded [43]byte
}

//go:noinline
func hashString(s string) [32]byte {
	hashWork.Lock()
	defer hashWork.Unlock()
	if len(s) <= len(hashWork.input) {
		copy(hashWork.input[:], s)
		var sum [32]byte
		shaInto(hashWork.input[:len(s)], &sum)
		clear(hashWork.input[:])
		return sum
	}
	var sum [32]byte
	shaInto([]byte(s), &sum)
	return sum
}
func verifierChallengeHash(verifier string) [32]byte {
	sum := hashString(verifier)
	hashWork.Lock()
	defer hashWork.Unlock()
	base64.RawURLEncoding.Encode(hashWork.encoded[:], sum[:])
	var result [32]byte
	shaInto(hashWork.encoded[:], &result)
	clear(hashWork.encoded[:])
	return result
}

func boundedCapacity(requested, ceiling int) int {
	if requested <= 0 || requested > ceiling {
		return ceiling
	}
	return requested
}
