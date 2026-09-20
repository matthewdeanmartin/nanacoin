package api

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// The view types below are what the API actually serialises. They exist so
// that adding a field to a domain struct - a password verifier, say - cannot
// leak it through an endpoint by accident. Nothing in a handler serialises a
// domain object directly.

type userView struct {
	ID          ledger.UserID    `json:"id"`
	Username    string           `json:"username"`
	DisplayName string           `json:"display_name"`
	Role        users.Role       `json:"role"`
	Status      users.Status     `json:"status"`
	Account     ledger.AccountID `json:"account"`
	CreatedAt   int64            `json:"created_at"`

	// Balance is filled only where the caller is entitled to see it.
	Balance *ledger.Amount `json:"balance,omitempty"`

	// USDCents is the dollar balance, on the same terms as Balance. In cents,
	// like everything else here - there are no floats anywhere in this system,
	// and $5.00 is 500.
	USDCents *ledger.Amount `json:"usd_cents,omitempty"`
}

// viewUser projects a user for the API.
//
// usd is variadic so the existing call sites, which predate dollars, keep
// working unchanged: a caller that passes nothing gets no dollar field, which
// is exactly what a client that has never heard of dollars should see.
func viewUser(u *users.User, balance *ledger.Amount, usd ...*ledger.Amount) userView {
	v := userView{
		ID: u.ID, Username: u.Username, DisplayName: u.DisplayName,
		Role: u.Role, Status: u.Status, Account: u.Account,
		CreatedAt: u.CreatedAt, Balance: balance,
	}
	if len(usd) > 0 {
		v.USDCents = usd[0]
	}
	return v
}

type accountView struct {
	ID      ledger.AccountID `json:"id"`
	UserID  ledger.UserID    `json:"user_id"`
	Name    string           `json:"name"`
	Status  users.Status     `json:"status"`
	Balance ledger.Amount    `json:"balance"`
}

type postingView struct {
	Account ledger.AccountID `json:"account"`
	Name    string           `json:"name"`
	Amount  ledger.Amount    `json:"amount"`
}

type transactionView struct {
	ID          ledger.TransactionID   `json:"id"`
	Kind        ledger.TransactionKind `json:"kind"`
	CreatedAt   int64                  `json:"created_at"`
	Actor       ledger.UserID          `json:"actor"`
	Description string                 `json:"description"`
	Reference   string                 `json:"reference,omitempty"`
	Reverses    ledger.TransactionID   `json:"reverses,omitempty"`
	ReversedBy  ledger.TransactionID   `json:"reversed_by,omitempty"`
	Postings    []postingView          `json:"postings"`
}

type listingView struct {
	ID          ledger.ListingID     `json:"id"`
	Seller      ledger.AccountID     `json:"seller"`
	SellerName  string               `json:"seller_name"`
	Title       string               `json:"title"`
	Description string               `json:"description"`
	Price       ledger.Amount        `json:"price"`
	Status      marketplace.Status   `json:"status"`
	CreatedAt   int64                `json:"created_at"`
	UpdatedAt   int64                `json:"updated_at"`
	Buyer       ledger.AccountID     `json:"buyer,omitempty"`
	BuyerName   string               `json:"buyer_name,omitempty"`
	SoldTx      ledger.TransactionID `json:"sold_tx,omitempty"`
	Kind        string               `json:"kind,omitempty"`
	Currency    string               `json:"currency,omitempty"`
	MinorUnits  int64                `json:"minor_units,omitempty"`

	// Side is "SELL" or "BUY". Omitted when SELL, so a client that predates
	// two-way listings sees exactly what it used to.
	Side string `json:"side,omitempty"`
}

// offerView is a proposal as the API returns it.
//
// ListingTitle and OffererName are denormalised here for the same reason
// posting names are: the client renders a list of offers and would otherwise
// have to fetch every listing to caption them.
type offerView struct {
	ID           ledger.OfferID          `json:"id"`
	Listing      ledger.ListingID        `json:"listing"`
	ListingTitle string                  `json:"listing_title"`
	Offerer      ledger.AccountID        `json:"offerer"`
	OffererName  string                  `json:"offerer_name"`
	Amount       ledger.Amount           `json:"amount"`
	Message      string                  `json:"message"`
	Status       marketplace.OfferStatus `json:"status"`
	CreatedAt    int64                   `json:"created_at"`
	UpdatedAt    int64                   `json:"updated_at"`
	SettledTx    ledger.TransactionID    `json:"settled_tx,omitempty"`

	// SettlesAt is when an acceptance stops being reversible, and Reversible
	// is whether it still is right now. Both are sent because the client
	// cannot compute the second without the server's clock - a board with no
	// RTC and a phone in another timezone will not agree.
	SettlesAt  int64 `json:"settles_at,omitempty"`
	Reversible bool  `json:"reversible"`
}

// namer resolves an account to a display name for the views. Transactions are
// stored with account IDs only; putting names in the ledger would mean a
// rename rewrote history.
type namer struct{ svc *core.Service }

func (n namer) name(id ledger.AccountID) string {
	if id == ledger.SystemIssuance {
		return "Issuance"
	}
	return n.svc.AccountName(id)
}

func (n namer) transaction(t *ledger.Transaction) transactionView {
	postings := make([]postingView, len(t.Postings))
	for i, p := range t.Postings {
		postings[i] = postingView{Account: p.Account, Name: n.name(p.Account), Amount: p.Amount}
	}
	v := transactionView{
		ID: t.ID, Kind: t.Kind, CreatedAt: t.CreatedAt, Actor: t.Actor,
		Description: t.Description, Reference: t.Reference,
		Reverses: t.Reverses, Postings: postings,
	}
	if rev, ok := n.svc.ReversalOf(t.ID); ok {
		v.ReversedBy = rev
	}
	return v
}

func (n namer) transactions(ts []*ledger.Transaction) []transactionView {
	out := make([]transactionView, len(ts))
	for i, t := range ts {
		out[i] = n.transaction(t)
	}
	return out
}

func (n namer) listing(l *marketplace.Listing) listingView {
	v := listingView{
		ID: l.ID, Seller: l.Seller, SellerName: n.name(l.Seller),
		Title: l.Title, Description: l.Description, Price: l.Price,
		Status: l.Status, CreatedAt: l.CreatedAt, UpdatedAt: l.UpdatedAt,
		Buyer: l.Buyer, SoldTx: l.SoldTx,
		Kind: l.Kind, Currency: l.Currency, MinorUnits: l.MinorUnits,
		Side: sideString(l.Side),
	}
	if l.Buyer != "" {
		v.BuyerName = n.name(l.Buyer)
	}
	return v
}

func (n namer) listings(ls []*marketplace.Listing) []listingView {
	out := make([]listingView, len(ls))
	for i, l := range ls {
		out[i] = n.listing(l)
	}
	return out
}

// lockedNamer is namer for use inside an Each* callback.
//
// namer calls svc.Account and svc.ReversalOf, each of which takes the service
// lock. Inside a walk the lock is already held, so using namer there would
// deadlock. This resolves through the *Locked accessors instead.
//
// The two exist separately rather than one growing a flag because the
// distinction is not a preference - getting it wrong is a hang on the board,
// which is indistinguishable from the network failures this project has
// already spent a long time chasing.
type lockedNamer struct{ svc *core.Service }

func (n lockedNamer) name(id ledger.AccountID) string {
	return n.svc.NameLocked(id)
}

func (n lockedNamer) transaction(t *ledger.Transaction) transactionView {
	// Postings are built per transaction and handed straight to the encoder,
	// which encodes and discards before the next record is unpacked. So this
	// allocation is live for one element, not for the page.
	postings := make([]postingView, len(t.Postings))
	for i, p := range t.Postings {
		postings[i] = postingView{Account: p.Account, Name: n.name(p.Account), Amount: p.Amount}
	}
	v := transactionView{
		ID: t.ID, Kind: t.Kind, CreatedAt: t.CreatedAt, Actor: t.Actor,
		Description: t.Description, Reference: t.Reference,
		Reverses: t.Reverses, Postings: postings,
	}
	if rev, ok := n.svc.ReversalOfLocked(t.ID); ok {
		v.ReversedBy = rev
	}
	return v
}

func (n lockedNamer) listing(l *marketplace.Listing) listingView {
	v := listingView{
		ID: l.ID, Seller: l.Seller, SellerName: n.name(l.Seller),
		Title: l.Title, Description: l.Description, Price: l.Price,
		Status: l.Status, CreatedAt: l.CreatedAt, UpdatedAt: l.UpdatedAt,
		Buyer: l.Buyer, SoldTx: l.SoldTx,
		Kind: l.Kind, Currency: l.Currency, MinorUnits: l.MinorUnits,
		Side: sideString(l.Side),
	}
	if l.Buyer != "" {
		v.BuyerName = n.name(l.Buyer)
	}
	return v
}

// sideString omits SELL, which is the default and the only value a client
// that predates two-way listings knows about.
func sideString(s marketplace.Side) string {
	if s == marketplace.SideBuy {
		return "BUY"
	}
	return ""
}

func (n namer) offer(o *marketplace.Offer, title string, now int64) offerView {
	return offerView{
		ID: o.ID, Listing: o.Listing, ListingTitle: title,
		Offerer: o.Offerer, OffererName: n.name(o.Offerer),
		Amount: o.Amount, Message: o.Message, Status: o.Status,
		CreatedAt: o.CreatedAt, UpdatedAt: o.UpdatedAt,
		SettledTx: o.SettledTx, SettlesAt: o.SettlesAt,
		Reversible: o.Reversible(now),
	}
}

func (n lockedNamer) offer(o *marketplace.Offer, title string, now int64) offerView {
	return offerView{
		ID: o.ID, Listing: o.Listing, ListingTitle: title,
		Offerer: o.Offerer, OffererName: n.name(o.Offerer),
		Amount: o.Amount, Message: o.Message, Status: o.Status,
		CreatedAt: o.CreatedAt, UpdatedAt: o.UpdatedAt,
		SettledTx: o.SettledTx, SettlesAt: o.SettlesAt,
		Reversible: o.Reversible(now),
	}
}

func (n lockedNamer) listingTitle(id ledger.ListingID) string {
	return n.svc.ListingTitleLocked(id)
}

// quoteView is a standing rate as the API returns it.
//
// MakerName is denormalised for the same reason posting names are: the client
// renders a book and would otherwise fetch every account to caption it.
//
// Cents is sent even though it is Coins x CentsPerCoin, because a client that
// recomputed it would be a second place for that arithmetic to be wrong, and
// the whole point of integer rates is that the number is exact.
type quoteView struct {
	ID           ledger.QuoteID   `json:"id"`
	Maker        ledger.AccountID `json:"maker"`
	MakerName    string           `json:"maker_name"`
	Side         string           `json:"side"`
	CentsPerCoin ledger.Amount    `json:"cents_per_coin"`
	Coins        ledger.Amount    `json:"coins"`
	Cents        ledger.Amount    `json:"cents"`

	Status    marketplace.QuoteStatus `json:"status"`
	CreatedAt int64                   `json:"created_at"`
	UpdatedAt int64                   `json:"updated_at"`
	ExpiresAt int64                   `json:"expires_at,omitempty"`

	// Live is whether it can still be taken right now. Sent rather than left
	// to the client because expiry is judged against the server's clock, and
	// a board with no RTC will not agree with a phone about what time it is.
	Live bool `json:"live"`

	Taker     ledger.AccountID     `json:"taker,omitempty"`
	TakerName string               `json:"taker_name,omitempty"`
	CoinTx    ledger.TransactionID `json:"coin_tx,omitempty"`
	CashTx    ledger.TransactionID `json:"cash_tx,omitempty"`
}

func (n namer) quote(q *marketplace.Quote, now int64) quoteView {
	v := quoteView{
		ID: q.ID, Maker: q.Maker, MakerName: n.name(q.Maker),
		Side: q.Side.String(), CentsPerCoin: q.CentsPerCoin, Coins: q.Coins,
		Cents: q.Cents(), Status: q.Status,
		CreatedAt: q.CreatedAt, UpdatedAt: q.UpdatedAt, ExpiresAt: q.ExpiresAt,
		Live:   q.Live(now),
		Taker:  q.Taker,
		CoinTx: q.CoinTx, CashTx: q.CashTx,
	}
	if q.Taker != "" {
		v.TakerName = n.name(q.Taker)
	}
	return v
}

func (n lockedNamer) quote(q *marketplace.Quote, now int64) quoteView {
	v := quoteView{
		ID: q.ID, Maker: q.Maker, MakerName: n.name(q.Maker),
		Side: q.Side.String(), CentsPerCoin: q.CentsPerCoin, Coins: q.Coins,
		Cents: q.Cents(), Status: q.Status,
		CreatedAt: q.CreatedAt, UpdatedAt: q.UpdatedAt, ExpiresAt: q.ExpiresAt,
		Live:   q.Live(now),
		Taker:  q.Taker,
		CoinTx: q.CoinTx, CashTx: q.CashTx,
	}
	if q.Taker != "" {
		v.TakerName = n.name(q.Taker)
	}
	return v
}
