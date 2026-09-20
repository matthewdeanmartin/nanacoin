package core

import (
	"errors"
	"fmt"
	"strings"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// checkPasswordLength is the household PIN rule.
//
// Four is low, and deliberately so: this is a household PIN, and the rate
// limiter rather than the password length is what makes guessing impractical
// here (spec 16).
//
// Checked by the callers before hashing, because hashing now happens outside
// the service lock and a rejected password must not pay for a hash first.
func checkPasswordLength(password string) error {
	if len(password) < 4 {
		return fmt.Errorf("%w: password must be at least 4 characters", ErrBadInput)
	}
	return nil
}

// Provisioned reports whether the first Nana exists. An unprovisioned server
// serves only /status and the provisioning endpoint.
func (s *Service) Provisioned() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.provisionedLocked()
}

func (s *Service) provisionedLocked() bool {
	return s.countNanaLocked() > 0
}

// Provision creates the first Nana. It is the one privileged operation with no
// authenticated caller, so it is permitted exactly once: afterwards, making
// another Nana requires being one (spec 6).
func (s *Service) Provision(username, displayName, password, householdName string) (*users.User, error) {
	if err := checkPasswordLength(password); err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if s.provisionedLocked() {
		return nil, fmt.Errorf("%w: already provisioned", ErrForbidden)
	}
	if householdName != "" {
		cfg := s.cfg
		cfg.HouseholdName = householdName
		if err := s.commitEvent(storage.TypeConfigUpdated, &configUpdatedEvent{Config: cfg}); err != nil {
			return nil, err
		}
	}
	verifier, err := auth.HashPassword(password)
	if err != nil {
		return nil, err
	}
	return s.createUserLocked(username, displayName, verifier, users.RoleNana, "", false)
}

// CreateUser is Nana creating a household member, optionally with the
// configured initial grant.
func (s *Service) CreateUser(actor *users.User, username, displayName, password string, role users.Role, grant bool) (*users.User, error) {
	if !actor.IsNana() {
		return nil, ErrForbidden
	}
	if err := checkPasswordLength(password); err != nil {
		return nil, err
	}
	// Hash before taking the lock. PBKDF2 is the longest single piece of CPU
	// this service does, and holding the global lock across it stalls every
	// other request - which on the board means connections piling up in the
	// router, each holding a goroutine stack and a pool slot. The work does
	// not depend on any locked state, so there is no reason to serialise it.
	verifier, err := auth.HashPassword(password)
	if err != nil {
		return nil, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	return s.createUserLocked(username, displayName, verifier, role, actor.ID, grant)
}

// createUserLocked records a user whose password has already been hashed.
//
// Takes a verifier rather than a password precisely so that the hashing
// happens outside the lock - see CreateUser. The length check therefore has
// to move to the callers, which is the one cost of the arrangement.
func (s *Service) createUserLocked(username, displayName, verifier string, role users.Role, actor ledger.UserID, grant bool) (*users.User, error) {
	username = strings.TrimSpace(username)
	if err := validateName(username, "username"); err != nil {
		return nil, err
	}
	if displayName == "" {
		displayName = username
	}
	if err := validateName(displayName, "display name"); err != nil {
		return nil, err
	}
	if s.store.findUserByName(username) >= 0 {
		return nil, ErrUsernameTaken
	}

	now := s.now().Unix()
	uid := ledger.UserID(s.newID("user"))
	aid := ledger.AccountID(s.newID("account"))

	ev := userCreatedEvent{
		User: users.User{
			ID: uid, Username: username, DisplayName: displayName,
			Role: role, Status: users.StatusActive, Account: aid,
			CreatedAt: now, Verifier: verifier,
		},
		Account: users.Account{
			ID: aid, UserID: uid, Name: displayName,
			Status: users.StatusActive, CreatedAt: now,
		},
	}
	if err := s.commitEvent(storage.TypeUserCreated, &ev); err != nil {
		return nil, err
	}

	if grant && s.cfg.InitialGrant > 0 {
		if _, err := s.issueLocked(actor, aid, s.cfg.InitialGrant, "Initial household allocation"); err != nil {
			// The user exists and is usable; only the grant failed. Say
			// so rather than pretending the whole operation failed, and
			// leave Nana to issue the grant by hand.
			created, _ := s.userByID(uid)
			return created, fmt.Errorf("user created but initial grant failed: %w", err)
		}
	}
	return s.userByID(uid)
}

// UpdateUser changes a display name, status, role or password. Nana may change
// anyone's; a user may change only their own display name and password.
func (s *Service) UpdateUser(actor *users.User, id ledger.UserID, displayName *string, status *users.Status, role *users.Role, password *string) (*users.User, error) {
	// Hash before the lock, for the reason given on CreateUser. Done up
	// front even though the update may still be refused on authorization
	// below: a wasted hash on a rejected request is cheaper than holding the
	// global lock through one on an accepted request.
	var newVerifier *string
	if password != nil {
		if err := checkPasswordLength(*password); err != nil {
			return nil, err
		}
		v, err := auth.HashPassword(*password)
		if err != nil {
			return nil, err
		}
		newVerifier = &v
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	target, _ := s.userByID(id)
	if target == nil {
		return nil, ErrUserNotFound
	}
	self := actor.ID == id
	if !actor.IsNana() && !self {
		return nil, ErrForbidden
	}
	if !actor.IsNana() && (status != nil || role != nil) {
		// Self-promotion is the one thing a household member must not be
		// able to do, so status and role are Nana-only even on oneself.
		return nil, ErrForbidden
	}

	ev := userUpdatedEvent{ID: id}
	if displayName != nil {
		if err := validateName(*displayName, "display name"); err != nil {
			return nil, err
		}
		ev.DisplayName = displayName
	}
	if status != nil {
		if *status != users.StatusActive && *status != users.StatusDisabled {
			return nil, fmt.Errorf("%w: unknown status %q", ErrBadInput, *status)
		}
		if *status == users.StatusDisabled && target.IsNana() && s.countActiveNanaLocked() < 2 {
			// Disabling the last Nana would leave a household with no one
			// able to create users or fix mistakes, and no way back in
			// short of reflashing.
			return nil, fmt.Errorf("%w: cannot disable the only Nana", ErrForbidden)
		}
		ev.Status = status
	}
	if role != nil {
		if *role != users.RoleNana && *role != users.RoleUser {
			return nil, fmt.Errorf("%w: unknown role %q", ErrBadInput, *role)
		}
		if *role == users.RoleUser && target.IsNana() && s.countActiveNanaLocked() < 2 {
			return nil, fmt.Errorf("%w: cannot demote the only Nana", ErrForbidden)
		}
		ev.Role = role
	}
	if newVerifier != nil {
		ev.Verifier = newVerifier
	}

	if err := s.commitEvent(storage.TypeUserUpdated, &ev); err != nil {
		return nil, err
	}
	return s.userByID(id)
}

func (s *Service) countActiveNanaLocked() int {
	return s.countNanaLocked()
}

// Authenticate verifies a username and password, returning the user. It is the
// only place a verifier is checked.
func (s *Service) Authenticate(username, password string) (*users.User, error) {
	s.mu.Lock()
	u := s.userByName(username)
	s.mu.Unlock()

	if u == nil {
		// Run a hash anyway so that a missing user and a wrong password
		// take comparable time. On an MCU the difference would otherwise
		// be hundreds of milliseconds and trivially observable.
		auth.DummyPasswordCheck(password)
		return nil, ErrUserNotFound
	}
	ok, err := auth.VerifyPassword(u.Verifier, password)
	if err != nil {
		return nil, err
	}
	if !ok {
		return nil, ErrUserNotFound
	}
	if !u.IsActive() {
		return nil, ErrDisabled
	}
	return u, nil
}

func (s *Service) User(id ledger.UserID) (*users.User, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	u, err := s.userByID(id)
	return u, err == nil
}

func validateName(v, what string) error {
	if v == "" {
		return fmt.Errorf("%w: %s is required", ErrBadInput, what)
	}
	if len(v) > MaxNameLen {
		return fmt.Errorf("%w: %s must be at most %d characters", ErrBadInput, what, MaxNameLen)
	}
	for _, r := range v {
		// Control characters in a name end up in JSON, in the UI and in
		// the journal. Rejecting them at the door is cheaper than
		// escaping them everywhere downstream.
		if r < 0x20 || r == 0x7f {
			return fmt.Errorf("%w: %s contains a control character", ErrBadInput, what)
		}
	}
	return nil
}

var _ = errors.Is
