package api

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Encoders for every shape this API emits.
//
// These are the hand-written replacement for encoding/json's reflection. Each
// one writes the same JSON its struct's tags describe, in the same field
// order, so no client can tell the difference - the tags on the view types in
// views.go remain the specification and these are the implementation.
//
// # Keeping the two in step
//
// A field added to a view type and not added here is a silently missing
// field. That is the real cost of dropping reflection, and it is mitigated
// two ways: the view type and its encoder are adjacent (views.go and this
// file), and encoding_test.go round-trips every shape through encoding/json
// and compares, so a drifted encoder fails a test on a desktop rather than
// producing a subtly wrong response on a board nobody can attach a debugger
// to.
//
// That test is the reason this is safe to hand-write. Without it, this file
// would be a slow-motion bug.

func (j *jsonw) userView(v *userView) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("username", v.Username)
	j.fStr("display_name", v.DisplayName)
	j.fStr("role", string(v.Role))
	j.fStr("status", string(v.Status))
	j.fStr("account", string(v.Account))
	j.fInt64("created_at", v.CreatedAt)
	// Balance is a *Amount with omitempty: present only where the caller is
	// entitled to see it.
	if v.Balance != nil {
		j.fInt64("balance", int64(*v.Balance))
	}
	if v.USDCents != nil {
		j.fInt64("usd_cents", int64(*v.USDCents))
	}
	j.objClose()
}

func (j *jsonw) accountView(v *accountView) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("user_id", string(v.UserID))
	j.fStr("name", v.Name)
	j.fStr("status", string(v.Status))
	j.fInt64("balance", int64(v.Balance))
	j.objClose()
}

func (j *jsonw) postingView(v *postingView) {
	j.objOpen()
	j.fStr("account", string(v.Account))
	j.fStr("name", v.Name)
	j.fInt64("amount", int64(v.Amount))
	j.objClose()
}

func (j *jsonw) transactionView(v *transactionView) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("kind", string(v.Kind))
	j.fInt64("created_at", v.CreatedAt)
	j.fStr("actor", string(v.Actor))
	j.fStr("description", v.Description)
	j.fStrOmit("reference", v.Reference)
	j.fStrOmit("reverses", string(v.Reverses))
	j.fStrOmit("reversed_by", string(v.ReversedBy))

	j.key("postings")
	j.arrOpen()
	for i := range v.Postings {
		j.comma()
		j.needComma = false
		j.postingView(&v.Postings[i])
	}
	j.arrClose()

	j.objClose()
}

func (j *jsonw) listingView(v *listingView) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("seller", string(v.Seller))
	j.fStr("seller_name", v.SellerName)
	j.fStr("title", v.Title)
	j.fStr("description", v.Description)
	j.fInt64("price", int64(v.Price))
	j.fStr("status", string(v.Status))
	j.fInt64("created_at", v.CreatedAt)
	j.fInt64("updated_at", v.UpdatedAt)
	j.fStrOmit("buyer", string(v.Buyer))
	j.fStrOmit("buyer_name", v.BuyerName)
	j.fStrOmit("sold_tx", string(v.SoldTx))
	j.fStrOmit("kind", v.Kind)
	j.fStrOmit("currency", v.Currency)
	j.fInt64Omit("minor_units", v.MinorUnits)
	j.fStrOmit("side", v.Side)
	j.objClose()
}

func (j *jsonw) offerView(v *offerView) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("listing", string(v.Listing))
	j.fStr("listing_title", v.ListingTitle)
	j.fStr("offerer", string(v.Offerer))
	j.fStr("offerer_name", v.OffererName)
	j.fInt64("amount", int64(v.Amount))
	j.fStr("message", v.Message)
	j.fStr("status", string(v.Status))
	j.fInt64("created_at", v.CreatedAt)
	j.fInt64("updated_at", v.UpdatedAt)
	j.fStrOmit("settled_tx", string(v.SettledTx))
	j.fInt64Omit("settles_at", v.SettlesAt)
	j.fBool("reversible", v.Reversible)
	j.objClose()
}

func (j *jsonw) errorBody(v *errorBody) {
	j.objOpen()
	j.fStr("error", v.Error)
	j.fStr("message", v.Message)
	j.objClose()
}

func (j *jsonw) status(v *core.Status) {
	j.objOpen()
	j.fBool("provisioned", v.Provisioned)
	j.fStr("household", v.Household)
	j.fStr("currency", v.Currency)
	j.fInt("users", v.Users)
	j.fInt("transactions", v.Transactions)
	j.fInt("retained_transactions", v.RetainedTransactions)
	j.fInt("transaction_capacity", v.TransactionCapacity)
	j.fUint64("oldest_transaction", uint64(v.OldestTransaction))
	j.fInt("active_listings", v.ActiveList)
	j.fInt64("circulation", int64(v.Circulation))
	j.fInt64("journal_used", v.JournalUsed)
	j.fInt64("journal_capacity", v.JournalCap)
	j.fBool("ledger_balanced", v.LedgerBalance)
	j.features(v)
	j.objClose()
}

// features reports which optional diagnostics this build carries.
//
// It lives on /status rather than on a health header because the client needs
// it before it renders a nav bar, and /status is the one endpoint it can read
// unauthenticated and before provisioning. A health header only exists on
// responses the board is still well enough to send, which is the wrong time
// to be deciding whether a tab exists.
func (j *jsonw) features(v *core.Status) {
	j.fBool("logs_enabled", v.LogsEnabled)
	j.fBool("diag_enabled", v.DiagEnabled)
}

// statusWithHealth is core.Status embedded plus one field. Written flat,
// because that is what embedding produces on the wire.
func (j *jsonw) statusWithHealth(v *core.Status, health string) {
	j.objOpen()
	j.fBool("provisioned", v.Provisioned)
	j.fStr("household", v.Household)
	j.fStr("currency", v.Currency)
	j.fInt("users", v.Users)
	j.fInt("transactions", v.Transactions)
	j.fInt("retained_transactions", v.RetainedTransactions)
	j.fInt("transaction_capacity", v.TransactionCapacity)
	j.fUint64("oldest_transaction", uint64(v.OldestTransaction))
	j.fInt("active_listings", v.ActiveList)
	j.fInt64("circulation", int64(v.Circulation))
	j.fInt64("journal_used", v.JournalUsed)
	j.fInt64("journal_capacity", v.JournalCap)
	j.fBool("ledger_balanced", v.LedgerBalance)
	j.fStr("health", health)
	j.features(v)
	j.objClose()
}

func (j *jsonw) config(v *core.Config) {
	j.objOpen()
	j.fStr("household_name", v.HouseholdName)
	j.fInt64("initial_grant", int64(v.InitialGrant))
	j.fStr("currency", v.Currency)
	j.fInt64("offer_settles_after", v.OfferSettlesAfter)
	j.objClose()
}

func (j *jsonw) event(v *eventlog.Event) {
	j.objOpen()
	j.fUint64("seq", v.Seq)
	j.fInt64("at", v.At)
	j.fStr("level", v.Level)
	j.fStr("kind", v.Kind)
	j.fStr("detail", v.Detail)
	j.objClose()
}

func (j *jsonw) diagnostics(v *Diagnostics) {
	j.objOpen()
	if v.Machine.Enabled {
		j.fInt("schema", 1)
		j.fStr("sampling", "on_request")
		j.fStr("memory_scope", "TinyGo managed heap; not all physical SRAM")
		j.fUint64("sampled_at_ms", v.Machine.SampledAtMS)
		j.fUint64("machine_samples", uint64(v.Machine.Count))
		j.fUint64("free_heap", v.Machine.Free)
		j.fInt("sampler_core", 0)
		j.fInt("http_core", 0)
		j.fInt("psram_free", 0)
		j.fBool("psram_enabled", false)
		j.key("internal")
		j.objOpen()
		j.fUint64("total", v.Machine.Total)
		j.fUint64("free", v.Machine.Free)
		for _, name := range [...]string{"largest", "minimum", "allocated_blocks", "free_blocks"} {
			j.key(name)
			j.null()
		}
		j.objClose()
		for _, name := range [...]string{"largest_free_block", "minimum_free_heap", "psram", "temperature_c", "rssi_dbm", "wifi_channel", "ip", "gateway", "netmask", "unix_seconds", "tasks", "sampler_stack_free_min_bytes", "ledger_storage", "boot_ready_ms", "requests", "errors"} {
			j.key(name)
			j.null()
		}
	}
	j.fStr("last_boot", v.LastBoot)
	j.fBool("crashed", v.Crashed)
	j.fUint64("boots", uint64(v.Boots))
	j.fStrOmit("phase", v.Phase)
	j.fStrOmit("route", v.Route)
	j.fUint64("served_before_crash", uint64(v.ServedBeforeCrash))
	j.fUint64("heap_free_at_crash", v.HeapFreeAtCrash)
	j.fUint64("alloc_failures", uint64(v.AllocFailures))
	j.fUint64("worst_headroom", v.WorstHeadroom)
	j.fInt64("uptime_seconds", v.UptimeSeconds)
	j.fStrOmit("health", v.Health)
	j.fUint64("free_now", v.FreeNow)
	j.fUint64("free_oldest", v.FreeOldest)
	j.fInt64("free_drop", v.FreeDrop)
	j.fInt("samples", v.Samples)
	j.fUint64("objects_now", v.ObjectsNow)
	j.fUint64("frag_now", uint64(v.FragNow))
	j.fUint64("frag_oldest", uint64(v.FragOldest))
	j.fUint64("gcs_now", uint64(v.GCsNow))
	j.fUint64("gc_delta", uint64(v.GCDelta))
	j.objClose()
}

func (j *jsonw) authorizeResponse(v *authorizeResponse) {
	j.objOpen()
	j.fStr("code", v.Code)
	j.objClose()
}

func (j *jsonw) tokenResponse(v *tokenResponse) {
	j.objOpen()
	j.fStr("access_token", v.AccessToken)
	j.fStr("token_type", v.TokenType)
	j.fInt64("expires_in", v.ExpiresIn)
	j.key("user")
	j.needComma = false
	j.userView(&v.User)
	j.objClose()
}

func (j *jsonw) purchaseResponse(v *purchaseResponse) {
	j.objOpen()
	j.key("listing")
	j.needComma = false
	j.listingView(&v.Listing)
	j.key("transaction")
	j.needComma = false
	j.transactionView(&v.Transaction)
	j.objClose()
}

// --- domain helpers ---------------------------------------------------------
//
// These build a view and encode it in one step, so a handler never holds a
// view struct longer than the call. The view types are small and stack-
// allocated here, which is what keeps the whole path allocation-free.

func (j *jsonw) writeUser(u *userView) { j.userView(u) }

func (j *jsonw) writeTransaction(n lockedNamer, t *ledger.Transaction) {
	v := n.transaction(t)
	j.transactionView(&v)
}

func (j *jsonw) writeListing(n lockedNamer, l *marketplace.Listing) {
	v := n.listing(l)
	j.listingView(&v)
}

// --- idempotent bodies ------------------------------------------------------
//
// Legacy allocating helpers for host tests. Board write handlers use the
// recordBuffer encoders below and copy retained receipts into the fixed cache.

// encodeToBytes runs an encoder into a fresh slice.
func encodeToBytes(fn func(*jsonw)) []byte {
	var sink sliceWriter
	var mem [jsonBufSize]byte
	j := newJSONW(&sink, mem[:])
	fn(&j)
	_ = j.done()
	return sink.b
}

// encodeTxnBody is the shape every money-moving endpoint returns.
func encodeTxnBody(n namer, txn *ledger.Transaction) []byte {
	v := n.transaction(txn)
	return encodeToBytes(func(j *jsonw) { j.transactionView(&v) })
}

// sliceWriter collects output into a growable slice.
type sliceWriter struct{ b []byte }

func (s *sliceWriter) Write(p []byte) (int, error) {
	s.b = append(s.b, p...)
	return len(p), nil
}

// txnRender writes a transaction straight from its packed record.
//
// Nothing here is a Go string: the actor and account names are interned
// bytes, the memo and reference are arena slices, and the IDs were formatted
// into the renderer's scratch. A thirty-item list therefore costs no
// per-record allocation at all - see internal/core/render.go for why that
// matters on a fragmenting heap.
func (j *jsonw) txnRender(r *core.TxnRender) {
	j.objOpen()
	j.fStrBytes("id", r.ID)
	j.fStr("kind", r.Kind)
	j.fInt64("created_at", r.CreatedAt)
	j.fStrBytes("actor", r.Actor)
	j.fStrBytes("description", r.Description)
	j.fStrBytesOmit("reference", r.Reference)
	j.fStrBytesOmit("reverses", r.Reverses)
	j.fStrBytesOmit("reversed_by", r.ReversedBy)

	j.key("postings")
	j.arrOpen()
	for i := 0; i < r.N; i++ {
		p := &r.Postings[i]
		j.comma()
		j.needComma = false
		j.objOpen()
		j.fStrBytes("account", p.Account)
		j.fStrBytes("name", p.Name)
		j.fInt64("amount", int64(p.Amount))
		j.objClose()
	}
	j.arrClose()

	j.objClose()
}

func (j *jsonw) transactionSource(n namer, v *ledger.Transaction) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("kind", string(v.Kind))
	j.fInt64("created_at", v.CreatedAt)
	j.fStr("actor", string(v.Actor))
	j.fStr("description", v.Description)
	j.fStrOmit("reference", v.Reference)
	j.fStrOmit("reverses", string(v.Reverses))
	if reversed, ok := n.svc.ReversalOf(v.ID); ok {
		j.fStrOmit("reversed_by", string(reversed))
	}

	j.key("postings")
	j.arrOpen()
	for i := range v.Postings {
		j.comma()
		j.needComma = false
		p := &v.Postings[i]
		j.objOpen()
		j.fStr("account", string(p.Account))
		j.fStr("name", n.name(p.Account))
		j.fInt64("amount", p.Amount)
		j.objClose()
	}
	j.arrClose()

	j.objClose()
}

func (j *jsonw) listingSource(n namer, v *marketplace.Listing) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("seller", string(v.Seller))
	j.fStr("seller_name", n.name(v.Seller))
	j.fStr("title", v.Title)
	j.fStr("description", v.Description)
	j.fInt64("price", int64(v.Price))
	j.fStr("status", string(v.Status))
	j.fInt64("created_at", v.CreatedAt)
	j.fInt64("updated_at", v.UpdatedAt)
	j.fStrOmit("buyer", string(v.Buyer))
	j.fStrOmit("buyer_name", n.name(v.Buyer))
	j.fStrOmit("sold_tx", string(v.SoldTx))
	j.fStrOmit("kind", v.Kind)
	j.fStrOmit("currency", v.Currency)
	j.fInt64Omit("minor_units", v.MinorUnits)
	j.objClose()
}

func (b *recordBuffer) encodeTransaction(n namer, t *ledger.Transaction) ([]byte, error) {
	b.n = 0
	b.json = newJSONW(b, b.jsonMemory[:])
	b.json.transactionSource(n, t)
	err := b.json.done()
	return b.data[:b.n], err
}
func (b *recordBuffer) encodePurchase(n namer, l *marketplace.Listing, t *ledger.Transaction) ([]byte, error) {
	b.n = 0
	b.json = newJSONW(b, b.jsonMemory[:])
	j := &b.json
	j.objOpen()
	j.key("listing")
	j.needComma = false
	j.listingSource(n, l)
	j.key("transaction")
	j.needComma = false
	j.transactionSource(n, t)
	j.objClose()
	err := j.done()
	return b.data[:b.n], err
}

// offerList is the shape /offers returns: an object with one array, matching
// the other list endpoints rather than a bare array.
func (j *jsonw) offerList(vs []offerView) {
	j.objOpen()
	j.key("offers")
	j.needComma = false
	j.arrOpen()
	for i := range vs {
		j.comma()
		j.needComma = false
		j.offerView(&vs[i])
		j.needComma = true
	}
	j.arrClose()
	j.objClose()
}

// encodeOfferResult is what accepting or unaccepting returns: the offer, the
// transaction that moved the money, and the listing it was against - the same
// three things a purchase returns, for the same reason.
func (b *recordBuffer) encodeOfferResult(
	n namer,
	o *marketplace.Offer,
	t *ledger.Transaction,
	title string,
	now int64,
) ([]byte, error) {
	b.n = 0
	b.json = newJSONW(b, b.jsonMemory[:])
	j := &b.json
	v := n.offer(o, title, now)

	j.objOpen()
	j.key("offer")
	j.needComma = false
	j.offerView(&v)
	j.key("transaction")
	j.needComma = false
	j.transactionSource(n, t)
	j.objClose()
	err := j.done()
	return b.data[:b.n], err
}

func (j *jsonw) quoteView(v *quoteView) {
	j.objOpen()
	j.fStr("id", string(v.ID))
	j.fStr("maker", string(v.Maker))
	j.fStr("maker_name", v.MakerName)
	j.fStr("side", v.Side)
	j.fInt64("cents_per_coin", int64(v.CentsPerCoin))
	j.fInt64("coins", int64(v.Coins))
	j.fInt64("cents", int64(v.Cents))
	j.fStr("status", string(v.Status))
	j.fInt64("created_at", v.CreatedAt)
	j.fInt64("updated_at", v.UpdatedAt)
	j.fInt64Omit("expires_at", v.ExpiresAt)
	j.fBool("live", v.Live)
	j.fStrOmit("taker", string(v.Taker))
	j.fStrOmit("taker_name", v.TakerName)
	j.fStrOmit("coin_tx", string(v.CoinTx))
	j.fStrOmit("cash_tx", string(v.CashTx))
	j.objClose()
}

// encodeTradeResult is what taking a quote returns: the filled quote and both
// legs of the money that moved.
//
// Both legs, because a client that saw only one could not tell a completed
// trade from a half-landed one - which is the failure mode this design
// deliberately accepts, so it has to be visible.
func (b *recordBuffer) encodeTradeResult(
	n namer,
	q *marketplace.Quote,
	coin, cash *ledger.Transaction,
	now int64,
) ([]byte, error) {
	b.n = 0
	b.json = newJSONW(b, b.jsonMemory[:])
	j := &b.json
	v := n.quote(q, now)

	j.objOpen()
	j.key("quote")
	j.needComma = false
	j.quoteView(&v)
	j.key("coin_transaction")
	j.needComma = false
	j.transactionSource(n, coin)
	j.key("cash_transaction")
	j.needComma = false
	j.transactionSource(n, cash)
	j.objClose()
	err := j.done()
	return b.data[:b.n], err
}
