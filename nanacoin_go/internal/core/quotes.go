package core

import (
	"fmt"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// Foreign exchange: quoting a rate, and taking one.
//
// See FOREX.md for the design and what it costs. The two facts that shape this
// file:
//
// A rate is whole cents per coin. Integers only, for the same reason the
// ledger has no floats: a rate of 0.333 would round somewhere on every trade
// and the household would gain or lose coins to arithmetic nobody could audit.
//
// A trade is two transactions, not one. MaxInlinePostings is 2, and raising it
// to 4 so a cross-currency trade fits in one record costs 7.1 KB across the
// ring. So the coin leg and the cash leg are separate records joined by
// Reference, and the cost of that - they can half-land - is handled below
// rather than hidden.

// Limits on a quote. Bounded like everything else here.
const (
	// MaxCentsPerCoin caps the rate. A coin worth more than a hundred dollars
	// is a typo, not a household exchange rate.
	MaxCentsPerCoin ledger.Amount = 10_000

	// MaxQuoteCoins caps one quote's size, so a single trade cannot move a
	// household's entire supply on one tap.
	MaxQuoteCoins ledger.Amount = 100_000
)

// IssueUSD brings dollars into the household. Nana only.
//
// The mirror of Issue for coins, and it exists for the same reason: without a
// distinguished account that dollars come from, "where did this $5 come from"
// has no answer the ledger can give. With it, every dollar is accounted for by
// the same whole-book property that covers every coin.
//
// What this does *not* claim is that the dollars physically exist. Nana saying
// the household holds $20 is Nana's word, exactly as it is for the rest of
// this system - see the settlement note in FOREX.md.
func (s *Service) IssueUSD(actor *users.User, to ledger.AccountID, cents ledger.Amount, reason string, scratch ...*WriteResult) (*ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if !actor.IsNana() {
		return nil, ErrForbidden
	}
	if err := validateAmount(cents); err != nil {
		return nil, err
	}
	if err := validateText(reason, MaxMemoLen, "reason"); err != nil {
		return nil, err
	}
	if err := s.checkUserAccountLocked(to); err != nil {
		return nil, err
	}

	out := writeResult(scratch)
	txn := &out.Transaction
	*txn = ledger.Transaction{
		ID:          s.book.NextID(),
		Kind:        ledger.KindIssue,
		CreatedAt:   s.now().Unix(),
		Actor:       actor.ID,
		Description: reason,
		Postings:    out.Postings[:],
	}
	out.Postings = [ledger.MaxInlinePostings]ledger.Posting{
		{Account: ledger.USDIssuance, Amount: -cents, Currency: ledger.USD},
		{Account: ledger.USDAccount(to), Amount: cents, Currency: ledger.USD},
	}
	if err := s.book.Validate(txn, false); err != nil {
		return nil, err
	}
	if err := s.commitEvent(storage.TypeTransactionCreated, &transactionEvent{Txn: *txn}); err != nil {
		return nil, err
	}
	return txn, nil
}

// PostQuote advertises a rate. No money moves.
//
// Anyone may quote, not only Nana. A household where only the bank sets prices
// is not a market, and the client is meant to watch this book for good rates -
// which only works if there are several people making them.
//
// Funds are not checked here. A quote is an intention, and someone may
// reasonably advertise a rate against an allowance not yet paid; the check
// happens when it is taken, which is when money actually moves.
func (s *Service) PostQuote(
	actor *users.User,
	side marketplace.QuoteSide,
	centsPerCoin, coins ledger.Amount,
	expiresAt int64,
) (*marketplace.Quote, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, err
	}
	if centsPerCoin <= 0 || centsPerCoin > MaxCentsPerCoin {
		return nil, fmt.Errorf("%w: rate must be between 1 and %d cents per coin",
			ErrBadInput, MaxCentsPerCoin)
	}
	if coins <= 0 || coins > MaxQuoteCoins {
		return nil, fmt.Errorf("%w: amount must be between 1 and %d coins",
			ErrBadInput, MaxQuoteCoins)
	}

	now := s.now().Unix()
	if expiresAt != 0 && expiresAt <= now {
		return nil, fmt.Errorf("%w: expiry is in the past", ErrBadInput)
	}

	q := marketplace.Quote{
		ID:           ledger.QuoteID(s.newID("quote")),
		Maker:        actor.Account,
		Side:         side,
		CentsPerCoin: centsPerCoin,
		Coins:        coins,
		Status:       marketplace.QuoteOpen,
		CreatedAt:    now,
		UpdatedAt:    now,
		ExpiresAt:    expiresAt,
	}
	if !s.store.canWriteQuote(&q) {
		return nil, ErrCapacity
	}
	if err := s.commitEvent(storage.TypeQuoteCreated, &quoteCreatedEvent{Quote: q}); err != nil {
		return nil, err
	}
	return s.quoteByID(q.ID)
}

// TakeQuote executes a trade at the quoted rate.
//
// # Two legs, and what that costs
//
// The coin leg and the cash leg are separate ledger records - see the file
// comment. Both are validated before either is committed, so the common
// failure (someone cannot afford it) refuses cleanly with nothing written.
//
// What remains is a commit failing between the two, which would leave coins
// moved and cash not. That window is one journal append wide and cannot be
// closed without a four-posting record. It is recorded rather than papered
// over: the quote keeps both transaction IDs, so a half-landed trade is a
// quote with a CoinTx and no CashTx - findable, and reversible by hand.
func (s *Service) TakeQuote(
	actor *users.User,
	id ledger.QuoteID,
	scratch ...*WriteResult,
) (*marketplace.Quote, *ledger.Transaction, *ledger.Transaction, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := s.requireActiveLocked(actor); err != nil {
		return nil, nil, nil, err
	}
	q, err := s.quoteByID(id)
	if err != nil {
		return nil, nil, nil, err
	}
	if q.Maker == actor.Account {
		return nil, nil, nil, ErrSelfDeal
	}

	now := s.now().Unix()
	if q.Status != marketplace.QuoteOpen {
		return nil, nil, nil, ErrQuoteClosed
	}
	if !q.Live(now) {
		return nil, nil, nil, ErrQuoteExpired
	}

	coinsFrom, coinsTo, centsFrom, centsTo := q.Pays(actor.Account)
	// A stack array, not a slice literal: ranging over []ledger.AccountID{...}
	// put a four-element slice on the heap on every take.
	accts := [4]ledger.AccountID{coinsFrom, coinsTo, centsFrom, centsTo}
	for i := range accts {
		if err := s.checkUserAccountLocked(accts[i]); err != nil {
			return nil, nil, nil, fmt.Errorf("account unavailable: %w", err)
		}
	}

	// Both legs are built in caller-supplied scratch - the same pre-allocated
	// buffer every other write endpoint uses. Written as locals with slice
	// literals for their postings, they escaped to the heap: four objects per
	// trade, on the board's most allocation-sensitive path.
	out := writeResult(scratch)
	coin, cash := &out.Transaction, &out.Second

	// Build both legs before committing either. The coin leg takes the next
	// sequence number and the cash leg the one after, which is also the order
	// they are appended in.
	coinSeq := s.book.NextID()
	*coin = ledger.Transaction{
		ID:          coinSeq,
		Kind:        ledger.KindTransfer,
		CreatedAt:   now,
		Actor:       actor.ID,
		Description: "Exchange",
		Reference:   string(q.ID),
		Postings:    out.Postings[:],
	}
	out.Postings = [ledger.MaxInlinePostings]ledger.Posting{
		{Account: coinsFrom, Amount: -q.Coins, Currency: ledger.NANA},
		{Account: coinsTo, Amount: q.Coins, Currency: ledger.NANA},
	}
	if err := s.book.Validate(coin, false); err != nil {
		return nil, nil, nil, err
	}

	// The cash leg is validated against the book as it will be *after* the
	// coin leg lands. Validate does not mutate, so checking it here is
	// checking the same balances - the two legs touch different currencies,
	// so the coin leg cannot change what the cash leg can afford.
	//
	// A provisional ID for validation; the real one is taken after the coin
	// leg has been appended and the sequence has moved.
	*cash = ledger.Transaction{
		ID:          coinSeq,
		Kind:        ledger.KindTransfer,
		CreatedAt:   now,
		Actor:       actor.ID,
		Description: "Exchange",
		Reference:   string(q.ID),
		Postings:    out.SecondPostings[:],
	}
	// The dollar wallets, not the coin accounts: dollars live in a separate
	// account so a balance stays a fold over one account.
	//
	// These two USDAccount calls each allocate, and unlike the one in
	// BalanceIn they cannot use a stack buffer: the name is stored in the
	// posting and outlives this function. Two allocations per trade for names
	// that get retained is the same price the coin accounts pay.
	out.SecondPostings = [ledger.MaxInlinePostings]ledger.Posting{
		{Account: ledger.USDAccount(centsFrom), Amount: -q.Cents(), Currency: ledger.USD},
		{Account: ledger.USDAccount(centsTo), Amount: q.Cents(), Currency: ledger.USD},
	}
	if err := s.book.Validate(cash, false); err != nil {
		return nil, nil, nil, err
	}

	ev := quoteTakenEvent{
		QuoteID:   q.ID,
		Taker:     actor.Account,
		CoinTxn:   *coin,
		UpdatedAt: now,
	}
	if err := s.commitEvent(storage.TypeQuoteTaken, &ev); err != nil {
		return nil, nil, nil, err
	}

	// The cash leg, now that the coin leg has a sequence number.
	cash.ID = s.book.NextID()
	if err := s.book.Validate(cash, false); err != nil {
		// The coin leg is already committed. Report it rather than pretending
		// the trade did not happen: the quote records CoinTx with no CashTx,
		// which is exactly what a half-landed trade looks like.
		return nil, nil, nil, fmt.Errorf("%w: cash leg refused after coins moved", err)
	}
	cashEv := quoteSettledEvent{
		QuoteID:   q.ID,
		CashTxn:   *cash,
		UpdatedAt: now,
	}
	if err := s.commitEvent(storage.TypeQuoteSettled, &cashEv); err != nil {
		return nil, nil, nil, err
	}

	updated, err := s.quoteByID(id)
	if err != nil {
		return nil, nil, nil, err
	}
	return updated, coin, cash, nil
}

// CancelQuote withdraws a standing rate. The maker's, or Nana's.
func (s *Service) CancelQuote(actor *users.User, id ledger.QuoteID) (*marketplace.Quote, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	q, err := s.quoteByID(id)
	if err != nil {
		return nil, err
	}
	if q.Maker != actor.Account && !actor.IsNana() {
		return nil, ErrForbidden
	}
	if q.Status != marketplace.QuoteOpen {
		return nil, ErrQuoteClosed
	}

	ev := quoteUpdatedEvent{
		ID:        q.ID,
		Status:    marketplace.QuoteCancelled,
		UpdatedAt: s.now().Unix(),
	}
	if err := s.commitEvent(storage.TypeQuoteUpdated, &ev); err != nil {
		return nil, err
	}
	return s.quoteByID(id)
}

// EachQuote walks the book, best rate first, without building a list.
//
// Streaming like every other list endpoint here: see EachOffer for what
// happened when one of them allocated per record instead.
//
// Order is what makes this a book rather than a pile. Asks ascend - the
// cheapest coins to buy come first - and bids descend, so the best price to
// sell into is first. A client watching for a good rate reads the top of each
// side and stops.
func (s *Service) EachQuote(fn func(*marketplace.Quote) bool, send ...func() bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	now := s.now().Unix()

	// A stack array, not a slice: 16 quotes is 128 bytes of stack against a
	// heap allocation on every request.
	var order [MaxQuotes]int
	n := 0
	for i := range s.store.quotesArr {
		if s.store.quotesArr[i].InUse {
			order[n] = i
			n++
		}
	}

	// Insertion sort: live before closed, then asks cheapest-first and bids
	// dearest-first, then oldest first so an equal rate is served in the order
	// it was quoted.
	for a := 1; a < n; a++ {
		v := order[a]
		b := a - 1
		for b >= 0 && s.quoteBefore(v, order[b], now) {
			order[b+1] = order[b]
			b--
		}
		order[b+1] = v
	}

	for k := 0; k < n; k++ {
		if !fn(s.store.unpackQuote(order[k])) {
			return
		}
		if len(send) > 0 && !s.sendUnlocked(send[0]) {
			return
		}
	}
}

// quoteBefore reports whether quote i sorts ahead of quote j.
func (s *Service) quoteBefore(i, j int, now int64) bool {
	a, b := &s.store.quotesArr[i], &s.store.quotesArr[j]

	aLive := a.Status == quoteStatusOpen && (a.ExpiresAt == 0 || now < a.ExpiresAt)
	bLive := b.Status == quoteStatusOpen && (b.ExpiresAt == 0 || now < b.ExpiresAt)
	if aLive != bLive {
		return aLive
	}
	if a.Side != b.Side {
		// Asks first, so the book reads sell-side then buy-side.
		return a.Side > b.Side
	}
	if a.CentsPerCoin != b.CentsPerCoin {
		if a.Side == uint8(marketplace.Ask) {
			return a.CentsPerCoin < b.CentsPerCoin
		}
		return a.CentsPerCoin > b.CentsPerCoin
	}
	return a.CreatedAt < b.CreatedAt
}

// Quote returns one quote by ID.
func (s *Service) Quote(id ledger.QuoteID) (*marketplace.Quote, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	q, err := s.quoteByID(id)
	return q, err == nil
}

// USDHeld is the total dollars the household holds, in cents.
func (s *Service) USDHeld() ledger.Amount {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.book.USDHeld()
}

// USDBalance is one account's dollar balance, in cents.
func (s *Service) USDBalance(id ledger.AccountID) ledger.Amount {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.book.BalanceIn(id, ledger.USD)
}
