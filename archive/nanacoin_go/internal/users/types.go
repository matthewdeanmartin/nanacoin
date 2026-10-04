// Package users holds household identities and their accounts.
package users

import "github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"

type Role string

const (
	RoleNana Role = "nana"
	RoleUser Role = "user"
)

type Status string

const (
	StatusActive   Status = "ACTIVE"
	StatusDisabled Status = "DISABLED"
)

// User is a person. The password verifier lives here rather than in a separate
// credential store because at household scale there is exactly one credential
// per person and splitting it buys nothing.
type User struct {
	ID          ledger.UserID    `json:"id"`
	Username    string           `json:"username"`
	DisplayName string           `json:"display_name"`
	Role        Role             `json:"role"`
	Status      Status           `json:"status"`
	Account     ledger.AccountID `json:"account"`
	CreatedAt   int64            `json:"created_at"`

	// Verifier is the salted password hash. It is never serialised to the
	// API - see api's user view type - but is journalled, since a household
	// that loses its passwords on reboot is unusable.
	Verifier string `json:"verifier"`
}

func (u *User) IsNana() bool   { return u.Role == RoleNana }
func (u *User) IsActive() bool { return u.Status == StatusActive }

// Account is the money-holding half of a user. It is a separate object from
// User because account IDs must be immutable and independent of display names
// (spec 5), and because a later version may give one user several accounts.
type Account struct {
	ID        ledger.AccountID `json:"id"`
	UserID    ledger.UserID    `json:"user_id"`
	Name      string           `json:"name"`
	Status    Status           `json:"status"`
	CreatedAt int64            `json:"created_at"`
}
