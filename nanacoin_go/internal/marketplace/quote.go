package marketplace

import "github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"

// Quotes: a standing offer to exchange NanaCoin for dollars at a stated rate.
//
// # Why a quote is not a listing
//
// A listing is prose with a price - "One hour of Switch time, 12 coins" - and
// a person reads it. A quote is a number a machine acts on: the client is
// meant to watch the book and take a good rate without anyone typing
// anything. Burying a rate in a listing title would make that parsing, which
// is the wrong shape for the one job this record has.
//
// So a quote carries no free text at all. It is a side, a rate, an amount and
// an expiry, and every one of those is an integer.

// QuoteSide is which way the quoter is trading, stated from their point of
// view.
//
// Bid means they are buying NanaCoin and paying dollars; ask means they are
// selling NanaCoin for dollars. The names are the market's, and they are worth
// keeping: "bid" and "ask" are unambiguous about who pays what, where "buy"
// and "sell" invite the question "buying which one?".
type QuoteSide uint8

const (
	// Bid: I will pay dollars for your NanaCoin.
	Bid QuoteSide = 0
	// Ask: I will sell NanaCoin for your dollars.
	Ask QuoteSide = 1
)

func (s QuoteSide) String() string {
	if s == Ask {
		return "ASK"
	}
	return "BID"
}

// ParseQuoteSide reads the wire form. Anything unrecognised is a bid, which is
// the side that cannot create NanaCoin out of nothing if it is wrong.
func ParseQuoteSide(s string) QuoteSide {
	if s == "ASK" {
		return Ask
	}
	return Bid
}

type QuoteStatus string

const (
	QuoteOpen      QuoteStatus = "OPEN"
	QuoteFilled    QuoteStatus = "FILLED"
	QuoteCancelled QuoteStatus = "CANCELLED"
	QuoteExpired   QuoteStatus = "EXPIRED"
)

// Quote is a standing offer to exchange, good until it is taken, cancelled or
// expires.
//
// # The rate
//
// CentsPerCoin is how many US cents one NanaCoin is worth, as an integer.
// 25 means a coin trades for a quarter; 100 means a coin is worth a dollar.
//
// Integer, because the no-floats rule in ledger.Amount applies here for the
// same reason: a rate of 0.333 would make every trade round somewhere, and
// the household would lose or gain coins to arithmetic nobody could audit. A
// rate that cannot be expressed in whole cents per coin is a rate this system
// does not offer, which is an honest limitation rather than a hidden rounding.
//
// # The amount
//
// Coins is how many NanaCoin the quoter will trade at that rate. The dollar
// side follows from it: Coins x CentsPerCoin, exactly, with no remainder by
// construction.
//
// Partial fills are not supported. Taking a quote takes all of it, which is
// what keeps this a quote board rather than an order book - there is no
// resting remainder to track, and no matching engine to write.
type Quote struct {
	ID ledger.QuoteID `json:"id"`

	// Maker is the account that posted it. Which of their two accounts pays
	// depends on the side, not on this.
	Maker ledger.AccountID `json:"maker"`

	Side         QuoteSide     `json:"side"`
	CentsPerCoin ledger.Amount `json:"cents_per_coin"`
	Coins        ledger.Amount `json:"coins"`

	Status    QuoteStatus `json:"status"`
	CreatedAt int64       `json:"created_at"`
	UpdatedAt int64       `json:"updated_at"`

	// ExpiresAt is when an untaken quote stops being available. Zero means it
	// stands until cancelled.
	//
	// Worth having because a rate is only meaningful for as long as the person
	// who quoted it still means it. A board full of week-old rates is worse
	// than an empty one: it invites someone to take a price nobody would
	// honour.
	ExpiresAt int64 `json:"expires_at,omitempty"`

	// Taker and the two transaction IDs are set when the quote is filled,
	// linking it to the money that moved.
	Taker  ledger.AccountID     `json:"taker,omitempty"`
	CoinTx ledger.TransactionID `json:"coin_tx,omitempty"`
	CashTx ledger.TransactionID `json:"cash_tx,omitempty"`
}

// Cents is the dollar side of the quote: what the whole thing costs.
//
// Exact by construction - Coins and CentsPerCoin are both integers, so their
// product is one too, and no trade ever rounds.
func (q *Quote) Cents() ledger.Amount { return q.Coins * q.CentsPerCoin }

// Live reports whether a quote can still be taken.
func (q *Quote) Live(now int64) bool {
	if q.Status != QuoteOpen {
		return false
	}
	return q.ExpiresAt == 0 || now < q.ExpiresAt
}

// Pays reports which account sends what when this quote is taken.
//
// The direction is the whole feature, and getting it backwards would move
// money the wrong way, so it is computed in one place rather than at each call
// site.
//
// On a bid the maker is buying coins: they send cents and receive coins. On an
// ask they are selling: they send coins and receive cents.
func (q *Quote) Pays(taker ledger.AccountID) (coinsFrom, coinsTo, centsFrom, centsTo ledger.AccountID) {
	if q.Side == Ask {
		// Maker sells coins for the taker's cents.
		return q.Maker, taker, taker, q.Maker
	}
	// Bid: maker buys coins with cents.
	return taker, q.Maker, q.Maker, taker
}
