package api

import (
	"net/http"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

func (s *Server) handleStatus(w http.ResponseWriter, r *http.Request) {
	st := s.svc.Status()

	// Which diagnostics exist is the API layer's fact, not the service's:
	// these are build tags over route registration. Filling them in here
	// keeps core free of any knowledge about HTTP.
	st.LogsEnabled = s.logsEnabled()
	st.DiagEnabled = DiagEnabled

	// Health goes in the body here as well as the header. A staged header can
	// be dropped when the response buffer fills - which is exactly what hid
	// the heap reading from the seven responses before a crash - and /status
	// is the endpoint a monitor polls, so this is the one place the reading
	// should be impossible to lose.
	if h := s.healthLine(); h != "" {
		encodeJSON(w, http.StatusOK, func(j *jsonw) { j.statusWithHealth(&st, h) })
		return
	}
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.status(&st) })
}

// statusWithHealth is Status plus the host's health line. Embedded so the
// existing fields keep their names and no client has to change.
type statusWithHealth struct {
	core.Status
	Health string `json:"health"`
}

// --- provisioning -----------------------------------------------------------

type provisionRequest struct {
	Username      string `json:"username"`
	DisplayName   string `json:"display_name"`
	Password      string `json:"password"`
	HouseholdName string `json:"household_name"`
}

func (s *Server) handleProvision(w http.ResponseWriter, r *http.Request) {
	if !s.allowProvision {
		writeError(w, core.ErrForbidden)
		return
	}
	// Log it. This is the only unauthenticated state change in the API, so a
	// board that was provisioned by someone unexpected should say so
	// somewhere that survives - the serial console is not watched, and the
	// event ring is readable from the logs page.
	s.log.Add(eventlog.Warn, "provision-attempt", r.Header.Get("Origin"))
	var req provisionRequest
	if !decodeInto(w, r, func(b []byte) error { return parseProvisionRequest(b, &req) }) {
		return
	}
	u, err := s.svc.Provision(req.Username, req.DisplayName, req.Password, req.HouseholdName)
	if err != nil {
		writeError(w, err)
		return
	}
	uv := viewUser(u, nil)
	encodeJSON(w, http.StatusCreated, func(j *jsonw) { j.userView(&uv) })
}

// --- auth -------------------------------------------------------------------

type authorizeRequest struct {
	Username            string `json:"username"`
	Password            string `json:"password"`
	CodeChallenge       string `json:"code_challenge"`
	CodeChallengeMethod string `json:"code_challenge_method"`
	RedirectURI         string `json:"redirect_uri"`
}

type authorizeResponse struct {
	Code string `json:"code"`
}

// handleAuthorize is the credential step of the PKCE flow. It returns an
// authorization code rather than a token, so the token only ever travels in
// response to a request that proves possession of the verifier.
func (s *Server) handleAuthorize(w http.ResponseWriter, r *http.Request) {
	var req authorizeRequest
	if !decodeInto(w, r, func(b []byte) error { return parseAuthorizeRequest(b, &req) }) {
		return
	}
	if req.CodeChallenge == "" {
		badRequest(w, "code_challenge is required")
		return
	}
	if req.CodeChallengeMethod != auth.MethodS256 {
		// plain is refused rather than tolerated (spec 15).
		badRequest(w, "code_challenge_method must be S256")
		return
	}

	// Rate limit before hashing. On an MCU the PBKDF2 round is the expensive
	// part of a login, so checking the limiter first is both a brute-force
	// defence and a denial-of-service defence.
	if err := s.sessions.CheckRateLimit(req.Username); err != nil {
		writeError(w, err)
		return
	}

	u, err := s.svc.Authenticate(req.Username, req.Password)
	if err != nil {
		s.sessions.RecordFailure(req.Username)
		if err == core.ErrDisabled {
			writeError(w, err)
			return
		}
		// A wrong password and an unknown user get the same answer, so the
		// endpoint cannot be used to enumerate household members.
		writeErrorBody(w, http.StatusUnauthorized, "invalid_credentials",
			"username or password is incorrect")
		return
	}
	s.sessions.RecordSuccess(req.Username)

	code, err := s.sessions.IssueCode(u.ID, req.CodeChallenge, req.CodeChallengeMethod, req.RedirectURI)
	if err != nil {
		writeError(w, err)
		return
	}
	ar := authorizeResponse{Code: code}
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.authorizeResponse(&ar) })
}

type tokenRequest struct {
	Code         string `json:"code"`
	CodeVerifier string `json:"code_verifier"`
	RedirectURI  string `json:"redirect_uri"`
}

type tokenResponse struct {
	AccessToken string   `json:"access_token"`
	TokenType   string   `json:"token_type"`
	ExpiresIn   int64    `json:"expires_in"`
	User        userView `json:"user"`
}

func (s *Server) handleToken(w http.ResponseWriter, r *http.Request) {
	var req tokenRequest
	if !decodeInto(w, r, func(b []byte) error { return parseTokenRequest(b, &req) }) {
		return
	}
	token, sess, err := s.sessions.RedeemCode(req.Code, req.CodeVerifier, req.RedirectURI)
	if err != nil {
		writeError(w, err)
		return
	}
	u, ok := s.svc.User(sess.UserID)
	if !ok {
		writeError(w, auth.ErrNoSession)
		return
	}
	bal := s.svc.Balance(u.Account)
	tr := tokenResponse{
		AccessToken: token,
		TokenType:   "Bearer",
		ExpiresIn:   int64(sess.ExpiresAt.Sub(sess.CreatedAt).Seconds()),
		User:        viewUser(u, &bal),
	}
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.tokenResponse(&tr) })
}

func (s *Server) handleLogout(w http.ResponseWriter, r *http.Request) {
	const prefix = "Bearer "
	if h := r.Header.Get("Authorization"); len(h) > len(prefix) {
		s.sessions.Revoke(h[len(prefix):])
	}
	// Logout is idempotent and never reports failure: a client trying to end
	// a session it no longer holds has got what it wanted.
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) handleMe(w http.ResponseWriter, r *http.Request) {
	u, ok := s.require(w, r)
	if !ok {
		return
	}
	bal := s.svc.Balance(u.Account)
	// /me shows both currencies: this is where a person looks for what they
	// have, and "how many dollars do I hold" is half of that once the
	// exchange exists.
	usd := s.svc.USDBalance(u.Account)
	uv := viewUser(u, &bal, &usd)
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.userView(&uv) })
}

// --- users ------------------------------------------------------------------

// handleListUsers returns the household. Everyone may see who is in it and
// what their account IDs are - that is what makes transfers possible - but
// balances are shown only to Nana and to the user themselves.
func (s *Server) handleListUsers(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	// Streamed: one user encoded at a time rather than a slice of views
	// built whole and then encoded whole. See internal/api/stream.go.
	record := <-s.freeRecords
	defer func() { s.freeRecords <- record }()
	sw := beginStream(w, http.StatusOK)
	sw.array("users", func(add func(func(*jsonw))) {
		s.svc.EachUser(func(u *users.User) bool {
			var bal *ledger.Amount
			if actor.IsNana() || actor.ID == u.ID {
				b := s.svc.BalanceLocked(u.Account)
				bal = &b
			}
			uv := viewUser(u, bal)
			return record.prepare(sw, func(j *jsonw) { j.userView(&uv) })
		}, func() bool { return record.send(sw, add) })
	})
	if err := sw.end(); err != nil {
		s.log.Add(eventlog.Error, "stream-failed", "GET /users: "+err.Error())
	}
}

type createUserRequest struct {
	Username    string     `json:"username"`
	DisplayName string     `json:"display_name"`
	Password    string     `json:"password"`
	Role        users.Role `json:"role"`
	Grant       *bool      `json:"grant"`
}

func (s *Server) handleCreateUser(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req createUserRequest
	if !decodeInto(w, r, func(b []byte) error { return parseCreateUserRequest(b, &req) }) {
		return
	}
	if req.Role == "" {
		req.Role = users.RoleUser
	}
	if req.Role != users.RoleUser && req.Role != users.RoleNana {
		badRequest(w, "role must be user or nana")
		return
	}
	// The initial grant is on by default, since issuing it is the normal
	// reason to create a user (spec 7).
	grant := true
	if req.Grant != nil {
		grant = *req.Grant
	}

	u, err := s.svc.CreateUser(actor, req.Username, req.DisplayName, req.Password, req.Role, grant)
	if err != nil {
		if u == nil {
			writeError(w, err)
			return
		}
		// The user exists but the grant failed. Report the user as created
		// with a warning rather than a bare error, because retrying the
		// whole call would hit ErrUsernameTaken and leave Nana confused
		// about what actually happened.
		bal := s.svc.Balance(u.Account)
		uv := viewUser(u, &bal)
		warning := err.Error()
		encodeJSON(w, http.StatusCreated, func(j *jsonw) {
			j.objOpen()
			j.key("user")
			j.needComma = false
			j.userView(&uv)
			j.fStr("warning", warning)
			j.objClose()
		})
		return
	}
	bal := s.svc.Balance(u.Account)
	uv := viewUser(u, &bal)
	encodeJSON(w, http.StatusCreated, func(j *jsonw) { j.userView(&uv) })
}

type updateUserRequest struct {
	DisplayName *string       `json:"display_name"`
	Status      *users.Status `json:"status"`
	Role        *users.Role   `json:"role"`
	Password    *string       `json:"password"`
}

func (s *Server) handleUpdateUser(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req updateUserRequest
	if !decodeInto(w, r, func(b []byte) error { return parseUpdateUserRequest(b, &req) }) {
		return
	}
	id := ledger.UserID(lastSegment(r.URL.Path))
	u, err := s.svc.UpdateUser(actor, id, req.DisplayName, req.Status, req.Role, req.Password)
	if err != nil {
		writeError(w, err)
		return
	}
	// A disabled user must lose their sessions now, not when their token
	// happens to expire.
	if req.Status != nil && *req.Status == users.StatusDisabled {
		s.sessions.RevokeUser(id)
	}
	// A password change invalidates other sessions, which is what someone
	// changing it because they think it leaked would expect.
	if req.Password != nil {
		s.sessions.RevokeUser(id)
	}
	bal := s.svc.Balance(u.Account)
	uv := viewUser(u, &bal)
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.userView(&uv) })
}

// --- accounts ---------------------------------------------------------------

// canSeeAccount is the read rule for money: your own, or anyone's if you are
// Nana (spec 6, ledger:read_own vs ledger:read_all).
func canSeeAccount(actor *users.User, id ledger.AccountID) bool {
	return actor.IsNana() || actor.Account == id
}

func (s *Server) handleAccount(w http.ResponseWriter, r *http.Request, id ledger.AccountID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	if !canSeeAccount(actor, id) {
		writeError(w, core.ErrForbidden)
		return
	}
	acct, owner, found := s.svc.Account(id)
	if !found {
		writeError(w, core.ErrAccountUnknown)
		return
	}
	_ = owner
	av := accountView{
		ID: acct.ID, UserID: acct.UserID, Name: acct.Name,
		Status: acct.Status, Balance: s.svc.Balance(id),
	}
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.accountView(&av) })
}

func (s *Server) handleAccountTransactions(w http.ResponseWriter, r *http.Request, id ledger.AccountID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	if !canSeeAccount(actor, id) {
		writeError(w, core.ErrForbidden)
		return
	}
	if _, _, found := s.svc.Account(id); !found {
		writeError(w, core.ErrAccountUnknown)
		return
	}
	limit := s.pageSize(r, 50)
	bal := s.svc.Balance(id)

	record := <-s.freeRecords
	defer func() { s.freeRecords <- record }()
	sw := beginStream(w, http.StatusOK)
	sw.fieldStr("account", string(id))
	sw.fieldInt64("balance", int64(bal))
	sw.array("transactions", func(add func(func(*jsonw))) {
		s.svc.EachTxnRender(id, limit, func(r *core.TxnRender) bool {
			return record.prepare(sw, func(j *jsonw) { j.txnRender(r) })
		}, func() bool { return record.send(sw, add) })
	})
	if err := sw.end(); err != nil {
		s.log.Add(eventlog.Error, "stream-failed", "GET history: "+err.Error())
	}
}

// --- money ------------------------------------------------------------------

type transferRequest struct {
	To     ledger.AccountID `json:"to"`
	Amount ledger.Amount    `json:"amount"`
	Memo   string           `json:"memo"`
}

func (s *Server) handleTransfer(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req transferRequest
	if !decodeInto(w, r, func(b []byte) error { return parseTransferRequest(b, &req) }) {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}
	body, err := s.svc.IdempotentInto(actor.ID, "transfer", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		txn, err := s.svc.Transfer(actor, req.To, req.Amount, req.Memo, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeTransaction(n, txn)
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

type issueRequest struct {
	To     ledger.AccountID `json:"to"`
	Amount ledger.Amount    `json:"amount"`
	Reason string           `json:"reason"`
}

func (s *Server) handleIssue(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req issueRequest
	if !decodeInto(w, r, func(b []byte) error { return parseIssueRequest(b, &req) }) {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}
	body, err := s.svc.IdempotentInto(actor.ID, "issue", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		txn, err := s.svc.Issue(actor, req.To, req.Amount, req.Reason, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeTransaction(n, txn)
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

type retireRequest struct {
	From   ledger.AccountID `json:"from"`
	Amount ledger.Amount    `json:"amount"`
	Reason string           `json:"reason"`
}

func (s *Server) handleRetire(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req retireRequest
	if !decodeInto(w, r, func(b []byte) error { return parseRetireRequest(b, &req) }) {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}
	body, err := s.svc.IdempotentInto(actor.ID, "retire", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		txn, err := s.svc.Retire(actor, req.From, req.Amount, req.Reason, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeTransaction(n, txn)
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

type reverseRequest struct {
	Reason string `json:"reason"`
}

func (s *Server) handleReverse(w http.ResponseWriter, r *http.Request, id ledger.TransactionID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req reverseRequest
	if !decodeInto(w, r, func(b []byte) error { return parseReverseRequest(b, &req) }) {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}
	body, err := s.svc.IdempotentInto(actor.ID, "reverse", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		txn, err := s.svc.Reverse(actor, id, req.Reason, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeTransaction(n, txn)
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

func (s *Server) handleTransaction(w http.ResponseWriter, r *http.Request, id ledger.TransactionID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	txn, _, found := s.svc.Transaction(id)
	if !found {
		writeError(w, ledger.ErrNotFound)
		return
	}
	// A user may read a transaction they were party to. Anything else is
	// Nana's business - otherwise transaction IDs would be a way to browse
	// the household's private dealings.
	if !actor.IsNana() && !txn.Affects(actor.Account) {
		writeError(w, core.ErrForbidden)
		return
	}
	tv := namer{s.svc}.transaction(txn)
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.transactionView(&tv) })
}

// handleAllTransactions is the full ledger. Nana only (ledger:read_all).
func (s *Server) handleAllTransactions(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	if !actor.IsNana() {
		writeError(w, core.ErrForbidden)
		return
	}
	limit := s.pageSize(r, 100)
	circulation := s.svc.Circulation()

	record := <-s.freeRecords
	defer func() { s.freeRecords <- record }()
	sw := beginStream(w, http.StatusOK)
	sw.array("transactions", func(add func(func(*jsonw))) {
		s.svc.EachTxnRender("", limit, func(r *core.TxnRender) bool {
			return record.prepare(sw, func(j *jsonw) { j.txnRender(r) })
		}, func() bool { return record.send(sw, add) })
	})
	sw.fieldInt64("circulation", int64(circulation))
	if err := sw.end(); err != nil {
		s.log.Add(eventlog.Error, "stream-failed", "GET /transactions: "+err.Error())
	}
}

// --- config -----------------------------------------------------------------
func (s *Server) handleGetConfig(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	if !actor.IsNana() {
		writeError(w, core.ErrForbidden)
		return
	}
	cfg := s.svc.Config()
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.config(&cfg) })
}

type setConfigRequest struct {
	HouseholdName *string        `json:"household_name"`
	InitialGrant  *ledger.Amount `json:"initial_grant"`
	Currency      *string        `json:"currency"`
}

func (s *Server) handleSetConfig(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req setConfigRequest
	if !decodeInto(w, r, func(b []byte) error { return parseSetConfigRequest(b, &req) }) {
		return
	}
	cfg := s.svc.Config()
	if req.HouseholdName != nil {
		cfg.HouseholdName = *req.HouseholdName
	}
	if req.InitialGrant != nil {
		cfg.InitialGrant = *req.InitialGrant
	}
	if req.Currency != nil {
		cfg.Currency = *req.Currency
	}
	updated, err := s.svc.SetConfig(actor, cfg)
	if err != nil {
		writeError(w, err)
		return
	}
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.config(&updated) })
}

// --- marketplace ------------------------------------------------------------

func (s *Server) handleListListings(w http.ResponseWriter, r *http.Request) {
	if _, ok := s.require(w, r); !ok {
		return
	}
	status := marketplace.Status(r.URL.Query().Get("status"))
	switch status {
	case "", marketplace.StatusActive, marketplace.StatusSold, marketplace.StatusCancelled:
	default:
		badRequest(w, "unknown status filter")
		return
	}
	record := <-s.freeRecords
	defer func() { s.freeRecords <- record }()
	sw := beginStream(w, http.StatusOK)
	sw.array("listings", func(add func(func(*jsonw))) {
		n := lockedNamer{s.svc}
		s.svc.EachListing(status, func(l *marketplace.Listing) bool {
			lv := n.listing(l)
			return record.prepare(sw, func(j *jsonw) { j.listingView(&lv) })
		}, func() bool { return record.send(sw, add) })
	})
	if err := sw.end(); err != nil {
		s.log.Add(eventlog.Error, "stream-failed", "GET /listings: "+err.Error())
	}
}

type createListingRequest struct {
	Title       string        `json:"title"`
	Description string        `json:"description"`
	Price       ledger.Amount `json:"price"`
	Kind        string        `json:"kind"`
	Currency    string        `json:"currency"`
	MinorUnits  int64         `json:"minor_units"`
	Side        string        `json:"side"`
}

func (s *Server) handleCreateListing(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req createListingRequest
	if !decodeInto(w, r, func(b []byte) error { return parseCreateListingRequest(b, &req) }) {
		return
	}
	l, err := s.svc.CreateListing(actor, core.ListingInput{
		Title: req.Title, Description: req.Description, Price: req.Price,
		Kind: req.Kind, Currency: req.Currency, MinorUnits: req.MinorUnits,
		Side: marketplace.ParseSide(req.Side),
	})
	if err != nil {
		writeError(w, err)
		return
	}
	lv := namer{s.svc}.listing(l)
	encodeJSON(w, http.StatusCreated, func(j *jsonw) { j.listingView(&lv) })
}

func (s *Server) handleListing(w http.ResponseWriter, r *http.Request, id ledger.ListingID) {
	if _, ok := s.require(w, r); !ok {
		return
	}
	l, found := s.svc.Listing(id)
	if !found {
		writeError(w, core.ErrListingUnknown)
		return
	}
	lv := namer{s.svc}.listing(l)
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.listingView(&lv) })
}

type updateListingRequest struct {
	Title       *string        `json:"title"`
	Description *string        `json:"description"`
	Price       *ledger.Amount `json:"price"`
}

func (s *Server) handleUpdateListing(w http.ResponseWriter, r *http.Request, id ledger.ListingID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req updateListingRequest
	if !decodeInto(w, r, func(b []byte) error { return parseUpdateListingRequest(b, &req) }) {
		return
	}
	l, err := s.svc.UpdateListing(actor, id, req.Title, req.Description, req.Price)
	if err != nil {
		writeError(w, err)
		return
	}
	lv := namer{s.svc}.listing(l)
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.listingView(&lv) })
}

func (s *Server) handleCancelListing(w http.ResponseWriter, r *http.Request, id ledger.ListingID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	l, err := s.svc.CancelListing(actor, id)
	if err != nil {
		writeError(w, err)
		return
	}
	lv := namer{s.svc}.listing(l)
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.listingView(&lv) })
}

type purchaseResponse struct {
	Listing     listingView     `json:"listing"`
	Transaction transactionView `json:"transaction"`
}

// handlePurchase is one call doing what a client must never do in two (spec 13).
func (s *Server) handlePurchase(w http.ResponseWriter, r *http.Request, id ledger.ListingID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}
	body, err := s.svc.IdempotentInto(actor.ID, "purchase", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		l, txn, err := s.svc.Purchase(actor, id, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodePurchase(n, l, txn)
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

// handleLogs returns the recent server events.
//
// Readable without a token, and deliberately so: the commonest thing to
// diagnose is a client that cannot authenticate, and a diagnostic endpoint
// that needs authentication would be useless exactly when it is needed. The
// entries carry paths, statuses and origins - no tokens, no passwords, no
// balances - so this is not a way to learn anything about the household's
// money.
func (s *Server) handleLogs(w http.ResponseWriter, r *http.Request) {
	limit := s.pageSize(r, 50)
	total := s.log.Count()
	health := s.healthLine()

	// Streamed like the other lists. The log is the endpoint most likely to
	// be polled while the board is already short of memory, so it is the
	// worst possible place to build a 64-entry slice and encode it whole.
	sw := beginStream(w, http.StatusOK)
	sw.array("events", func(add func(func(*jsonw))) {
		s.log.Each(limit, func(e eventlog.Event) bool {
			ev := e
			add(func(j *jsonw) { j.event(&ev) })
			return true
		})
	})
	sw.fieldUint64("total", total)
	// The host's own health, where it has any to report. On the board this is
	// the heap, which is the number that mattered most while diagnosing an
	// out-of-memory and was previously only visible over USB.
	if health != "" {
		sw.fieldStr("health", health)
	}
	if err := sw.end(); err != nil {
		println("boardhttp: logs stream failed:", err.Error())
	}
}
