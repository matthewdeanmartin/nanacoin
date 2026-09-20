// Package ledger holds NanaCoin's money model: an append-only sequence of
// transactions, each a set of postings that sum to zero.
//
// There is no balance field anywhere in this package's persistent model. A
// balance is a fold over postings, and the cached copies in Book exist purely
// so that reads are cheap; they are rebuilt from the transaction list on boot
// and are never the authority for anything.
package ledger

// Amount is a whole NanaCoin. Integers only - see spec 2.3. Nothing in this
// codebase may represent money as a float, including in JSON, where an int64
// larger than 2^53 would lose precision. At household scale that ceiling is
// unreachable, but the type stays int64 so the ledger can never silently
// round.
type Amount = int64

type (
	AccountID     string
	UserID        string
	TransactionID string
	ListingID     string
	OfferID       string
	QuoteID       string
)

// USDAccount is the dollar wallet belonging to a coin account.
//
// Two accounts per person rather than one account holding two currencies,
// which is what keeps the balance machinery untouched: a balance is still the
// fold over one account's postings, and "does Alice have $5" is the same
// question as "does Alice have 5 coins", asked of a different account.
//
// MaxAccounts is 32 for 16 users, and the comment there has always said the
// second slot was for "a user to gain a second account later without a format
// change". This is that.
func USDAccount(coinAccount AccountID) AccountID {
	return coinAccount + USDSuffix
}

// USDSuffix is what USDAccount appends. Named so the allocation-free path in
// BalanceIn can build the same name in a stack buffer without the two
// spellings drifting apart.
const USDSuffix = "-usd"

// MaxAccountIDLen bounds an account name, so a derived one can be built on the
// stack. Account IDs are "account-" plus a short generated tail; the dollar
// wallets add USDSuffix on top of that.
const MaxAccountIDLen = 64

// USDIssuance is where dollars enter and leave the household, mirroring
// SystemIssuance for coins.
//
// It exists because dollars are the first thing this system holds that it
// cannot create. A coin's provenance is the issuance account and the invariant
// that every posting sums to zero; without an equivalent for dollars, "where
// did this $5 come from" has no answer the ledger can give. With it, the same
// whole-book property covers both.
const USDIssuance AccountID = "account:usd-issuance"

// SystemIssuance is the account new coin is created from and retired into. It
// is the one account permitted to go arbitrarily negative: its balance is the
// negation of all NanaCoin in circulation, which gives the invariant that
// every posting in the book sums to zero across all accounts.
//
// It is not a user account and must never be exposed as a transfer target.
const SystemIssuance AccountID = "account:system-issuance"

type TransactionKind string

const (
	KindIssue    TransactionKind = "ISSUE"
	KindRetire   TransactionKind = "RETIRE"
	KindTransfer TransactionKind = "TRANSFER"
	KindPurchase TransactionKind = "PURCHASE"
	KindReversal TransactionKind = "REVERSAL"
)

// Currency is what a posting is denominated in.
//
// NanaCoin is the zero value, so every record written before foreign exchange
// existed reads back as what it was, and every transaction that does not
// mention a currency is in coins.
//
// Deliberately a byte rather than a string code. A household trades in one or
// two currencies, the set is fixed at compile time, and a string would cost an
// intern slot per posting on a device where the intern table is a measured
// scarcity.
type Currency uint8

const (
	// NANA is the household currency. Zero, so it is the default everywhere.
	NANA Currency = 0
	// USD is United States dollars, held in cents - see Amount's no-floats
	// rule. $5.00 is 500, never 5.0.
	USD Currency = 1
)

func (c Currency) String() string {
	if c == USD {
		return "USD"
	}
	return "NANA"
}

// Minor reports whether the currency is counted in minor units, which decides
// how a client formats it: 500 USD renders as $5.00, 500 NANA as 500 coins.
func (c Currency) Minor() bool { return c == USD }

// Posting is one side of a transaction: a signed change to one account.
type Posting struct {
	Account AccountID `json:"account"`
	Amount  Amount    `json:"amount"`

	// Currency the amount is in. Omitted on the wire when NanaCoin, so an
	// older client sees exactly the JSON it saw before.
	Currency Currency `json:"currency,omitempty"`
}

// Transaction is immutable once appended. Corrections are made by appending a
// reversal, never by editing or deleting - see spec 11.
type Transaction struct {
	ID          TransactionID   `json:"id"`
	Kind        TransactionKind `json:"kind"`
	CreatedAt   int64           `json:"created_at"` // unix seconds
	Actor       UserID          `json:"actor"`
	Description string          `json:"description"`

	// Reference points at whatever non-ledger object caused this
	// transaction - a listing ID for a purchase, say. Free-form so that
	// future features need no ledger schema change.
	Reference string `json:"reference,omitempty"`

	// Reverses is set on a KindReversal transaction and names the
	// transaction being undone. The reverse direction (original -> its
	// reversal) is an in-memory index, not a stored field, because stored
	// records are never rewritten.
	Reverses TransactionID `json:"reverses,omitempty"`

	Postings []Posting `json:"postings"`
}

// Sum returns the total of all postings in one currency.
//
// Per currency, not across them: a cross-currency trade moves coins one way
// and dollars the other, and a single total would let 100 coins out cancel
// 100 cents in. Each currency must balance on its own, which is what makes
// "every coin is accounted for" and "every dollar is accounted for" two
// separate true statements rather than one averaged one.
func (t *Transaction) Sum(c Currency) Amount {
	var total Amount
	for _, p := range t.Postings {
		if p.Currency == c {
			total += p.Amount
		}
	}
	return total
}

// Currencies reports which currencies a transaction touches. A household
// transaction touches one; a forex leg touches one; only a hypothetical
// multi-leg record would touch two, and MaxInlinePostings does not allow it.
func (t *Transaction) Currencies() (nana, usd bool) {
	for _, p := range t.Postings {
		if p.Currency == USD {
			usd = true
		} else {
			nana = true
		}
	}
	return nana, usd
}

// Affects reports whether the transaction touches an account, so history
// queries do not need to care which side of it a user was on.
func (t *Transaction) Affects(acct AccountID) bool {
	for _, p := range t.Postings {
		if p.Account == acct {
			return true
		}
	}
	return false
}
