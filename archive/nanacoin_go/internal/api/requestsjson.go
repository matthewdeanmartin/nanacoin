package api

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
	"sync"
)

// Parsers for every request body this API accepts.
//
// One function per request type, dispatching on the field name. The compiler
// turns the switch into a comparison chain rather than a map lookup, so
// nothing is allocated for the field set - see jsonreader.go for the scanner
// and for the measurement that motivated this.
//
// # Keeping these in step with the structs
//
// A field added to a request struct and not added here is silently ignored,
// which on a money API is worse than a crash. Two things guard it: the struct
// and its parser are adjacent (handlers.go and this file), and
// jsonreader_test.go round-trips every type against encoding/json with both
// full and partial bodies, so a drifted parser fails a desktop test.
//
// # escapeScratch
//
// Strings containing backslash escapes are decoded into a scratch buffer
// before being copied into the struct. One startup buffer is protected by a mutex
// during parsing only. Returned strings own their bytes; no request retains
// a reference to the shared scratch.

// escapeScratchSize bounds the unescaped form of one string field. The
// longest field this API accepts is a 500-character description, and
// MaxRequestBody caps the whole body well below this anyway.
const escapeScratchSize = 640

var parseScratch struct {
	sync.Mutex
	data [escapeScratchSize]byte
}

func parseProvisionRequest(body []byte, v *provisionRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "username"):
			v.Username = p.str(scratch[:])
		case keyIs(key, "display_name"):
			v.DisplayName = p.str(scratch[:])
		case keyIs(key, "password"):
			v.Password = p.str(scratch[:])
		case keyIs(key, "household_name"):
			v.HouseholdName = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseAuthorizeRequest(body []byte, v *authorizeRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "username"):
			v.Username = p.str(scratch[:])
		case keyIs(key, "password"):
			v.Password = p.str(scratch[:])
		case keyIs(key, "code_challenge"):
			v.CodeChallenge = p.str(scratch[:])
		case keyIs(key, "code_challenge_method"):
			v.CodeChallengeMethod = p.str(scratch[:])
		case keyIs(key, "redirect_uri"):
			v.RedirectURI = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseTokenRequest(body []byte, v *tokenRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "code"):
			v.Code = p.str(scratch[:])
		case keyIs(key, "code_verifier"):
			v.CodeVerifier = p.str(scratch[:])
		case keyIs(key, "redirect_uri"):
			v.RedirectURI = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseCreateUserRequest(body []byte, v *createUserRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "username"):
			v.Username = p.str(scratch[:])
		case keyIs(key, "display_name"):
			v.DisplayName = p.str(scratch[:])
		case keyIs(key, "password"):
			v.Password = p.str(scratch[:])
		case keyIs(key, "role"):
			v.Role = users.Role(p.str(scratch[:]))
		case keyIs(key, "grant"):
			if p.isNull() {
				v.Grant = nil
			} else {
				v.Grant = p.optBool()
			}
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseUpdateUserRequest(body []byte, v *updateUserRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "display_name"):
			if p.isNull() {
				v.DisplayName = nil
			} else {
				v.DisplayName = p.optStr(scratch[:])
			}
		case keyIs(key, "status"):
			if p.isNull() {
				v.Status = nil
			} else {
				v.Status = (*users.Status)(p.optStr(scratch[:]))
			}
		case keyIs(key, "role"):
			if p.isNull() {
				v.Role = nil
			} else {
				v.Role = (*users.Role)(p.optStr(scratch[:]))
			}
		case keyIs(key, "password"):
			if p.isNull() {
				v.Password = nil
			} else {
				v.Password = p.optStr(scratch[:])
			}
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseTransferRequest(body []byte, v *transferRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "to"):
			v.To = ledger.AccountID(p.str(scratch[:]))
		case keyIs(key, "amount"):
			v.Amount = ledger.Amount(p.int64())
		case keyIs(key, "memo"):
			v.Memo = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseIssueRequest(body []byte, v *issueRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "to"):
			v.To = ledger.AccountID(p.str(scratch[:]))
		case keyIs(key, "amount"):
			v.Amount = ledger.Amount(p.int64())
		case keyIs(key, "reason"):
			v.Reason = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseRetireRequest(body []byte, v *retireRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "from"):
			v.From = ledger.AccountID(p.str(scratch[:]))
		case keyIs(key, "amount"):
			v.Amount = ledger.Amount(p.int64())
		case keyIs(key, "reason"):
			v.Reason = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseReverseRequest(body []byte, v *reverseRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "reason"):
			v.Reason = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseSetConfigRequest(body []byte, v *setConfigRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "household_name"):
			if p.isNull() {
				v.HouseholdName = nil
			} else {
				v.HouseholdName = p.optStr(scratch[:])
			}
		case keyIs(key, "initial_grant"):
			if p.isNull() {
				v.InitialGrant = nil
			} else {
				v.InitialGrant = (*ledger.Amount)(p.optInt64())
			}
		case keyIs(key, "currency"):
			if p.isNull() {
				v.Currency = nil
			} else {
				v.Currency = p.optStr(scratch[:])
			}
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseCreateListingRequest(body []byte, v *createListingRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "title"):
			v.Title = p.str(scratch[:])
		case keyIs(key, "description"):
			v.Description = p.str(scratch[:])
		case keyIs(key, "price"):
			v.Price = ledger.Amount(p.int64())
		case keyIs(key, "kind"):
			v.Kind = p.str(scratch[:])
		case keyIs(key, "currency"):
			v.Currency = p.str(scratch[:])
		case keyIs(key, "minor_units"):
			v.MinorUnits = p.int64()
		case keyIs(key, "side"):
			v.Side = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseUpdateListingRequest(body []byte, v *updateListingRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "title"):
			if p.isNull() {
				v.Title = nil
			} else {
				v.Title = p.optStr(scratch[:])
			}
		case keyIs(key, "description"):
			if p.isNull() {
				v.Description = nil
			} else {
				v.Description = p.optStr(scratch[:])
			}
		case keyIs(key, "price"):
			if p.isNull() {
				v.Price = nil
			} else {
				v.Price = (*ledger.Amount)(p.optInt64())
			}
		default:
			return false
		}
		return true
	})
	return p.done()
}

// marketplace is referenced by the listing status type in the update path.
var _ marketplace.Status

func parseOfferRequest(body []byte, v *offerRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "amount"):
			v.Amount = ledger.Amount(p.int64())
		case keyIs(key, "message"):
			v.Message = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseUnacceptRequest(body []byte, v *unacceptRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		if keyIs(key, "reason") {
			v.Reason = p.str(scratch[:])
			return true
		}
		return false
	})
	return p.done()
}

func parseQuoteRequest(body []byte, v *quoteRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "side"):
			v.Side = p.str(scratch[:])
		case keyIs(key, "cents_per_coin"):
			v.CentsPerCoin = ledger.Amount(p.int64())
		case keyIs(key, "coins"):
			v.Coins = ledger.Amount(p.int64())
		case keyIs(key, "expires_at"):
			v.ExpiresAt = p.int64()
		default:
			return false
		}
		return true
	})
	return p.done()
}

func parseIssueUSDRequest(body []byte, v *issueUSDRequest) error {
	p := newJSONR(body)
	parseScratch.Lock()
	defer parseScratch.Unlock()
	scratch := &parseScratch.data
	p.object(0, func(key []byte) bool {
		switch {
		case keyIs(key, "to"):
			v.To = ledger.AccountID(p.str(scratch[:]))
		case keyIs(key, "cents"):
			v.Cents = ledger.Amount(p.int64())
		case keyIs(key, "reason"):
			v.Reason = p.str(scratch[:])
		default:
			return false
		}
		return true
	})
	return p.done()
}
