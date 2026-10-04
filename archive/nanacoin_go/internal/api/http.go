// Package api is the HTTP surface: a plain JSON API over net/http, with no
// framework and no code generation.
//
// The client is a static page that may be served from an entirely different
// origin (spec 3), so everything here is designed around that: bearer tokens
// rather than cookies, explicit CORS, and no assumption that the page and the
// API share a host.
package api

import (
	"errors"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/users"
)

// MaxRequestBody bounds request bodies. Every request this API accepts is a
// handful of fields; anything larger is a mistake or an attack, and on a
// device with a few hundred KB of RAM it must be refused before it is read.
const MaxRequestBody = 16 * 1024

// DefaultMaxPageSize caps how many transactions or listings a response may
// contain when the host does not set its own limit.
//
// A cap is necessary because the client chooses the limit: `?limit=500` is a
// request the server has to be able to refuse to honour in full. On the board
// this is what stands between a ledger page and an out-of-memory - rendering
// fifty transactions there produced "fatal error: out of memory", because a
// growing buffer transiently holds twice what it ends up with.
const DefaultMaxPageSize = 100

// Server wires the service, the session store and the allowed origins into an
// http.Handler.
type Server struct {
	records     *[RecordBufferCount]recordBuffer
	freeRecords chan *recordBuffer
	svc         *core.Service
	sessions    *auth.Store
	origins     []string

	// log records what the server decided, so a board with no screen can be
	// asked. Never nil - NewServer installs one if the caller supplied none.
	log *eventlog.Log

	// maxPage caps every list response. Always positive after NewServer.
	maxPage int

	// diag reports the host's crash record and health. Optional; nil on the
	// desktop, which has a console and a supervisor and needs neither.
	diag        DiagnosticsFunc
	machineInfo func() MachineInfo

	// allowProvision permits the unauthenticated bootstrap endpoint.
	//
	// This is the one endpoint that changes state without a token, so it is
	// what the wildcard CORS policy rests on: once a household exists, the
	// service refuses a second provisioning and there is nothing an
	// unauthenticated caller can do. Before that, an open board is open to
	// whoever reaches it first - which is true of the serial console too,
	// and is why the window is meant to be minutes rather than the board's
	// lifetime.
	allowProvision bool

	// health reports the host's own condition - heap figures on the board,
	// nothing on a desktop.
	//
	// Held here rather than on the event log, which is where it used to live.
	// That coupling meant a no-logs build reported no health either: the one
	// number needed to diagnose a memory problem vanished exactly when
	// logging had been turned off to save memory. The two are unrelated.
	health func() string
}

type Config struct {
	AllowedOrigins []string
	AllowProvision bool

	// Log receives request and decision events. Optional; a nil Log gets a
	// working one on a logging build, since the diagnostics are worth
	// having by default. A no-logs build leaves it nil: constructing a ring
	// there would allocate nothing (Capacity is 0) but would still claim
	// the log exists, and /status reports the opposite.
	Log *eventlog.Log

	// NoLog turns event recording off even on a build that supports it.
	//
	// It exists because nil Log already means "give me the default one",
	// and that convention cannot also express "explicitly none" - a host
	// passing nil to mean off would silently get a working ring. Rather
	// than change what nil means for every existing caller, off is its own
	// field.
	NoLog bool

	// MaxPageSize caps list responses however large a limit the client asks
	// for. Zero means DefaultMaxPageSize. The board sets this low: it is the
	// difference between a ledger page and an out-of-memory.
	MaxPageSize int
}

func NewServer(svc *core.Service, sessions *auth.Store, cfg Config) *Server {
	log := cfg.Log
	if log == nil && eventlog.Enabled && !cfg.NoLog {
		log = eventlog.New(func() int64 { return time.Now().Unix() })
	}
	if cfg.NoLog {
		log = nil
	}
	maxPage := cfg.MaxPageSize
	if maxPage <= 0 {
		maxPage = DefaultMaxPageSize
	}
	server := &Server{
		records:        new([RecordBufferCount]recordBuffer),
		freeRecords:    make(chan *recordBuffer, RecordBufferCount),
		svc:            svc,
		sessions:       sessions,
		origins:        cfg.AllowedOrigins,
		allowProvision: cfg.AllowProvision,
		log:            log,
		maxPage:        maxPage,
	}
	for i := range server.records {
		server.freeRecords <- &server.records[i]
	}
	return server
}

// SetHealth registers the host's health reporter.
//
// The event log also shows it beside its events, when there is a log at all -
// but the reading itself no longer depends on one existing.
func (s *Server) SetHealth(fn func() string) {
	s.health = fn
	s.log.SetHealth(eventlog.HealthFunc(fn))
}

// healthLine is the host's health, or empty if nothing was registered.
func (s *Server) healthLine() string {
	if s.health == nil {
		return ""
	}
	return s.health()
}

// logsEnabled reports whether this server records events and should serve
// /logs.
//
// Both facts have to come from the same place. The build tag alone is not
// enough: a logging build started with -logs=false has a nil log, and
// registering the route or advertising the capability from the constant would
// have the client render a Logs tab over an endpoint with nothing behind it.
func (s *Server) logsEnabled() bool { return eventlog.Enabled && s.log != nil }

// Log exposes the event log so a host can record its own events into the same
// ring - WiFi state on the board, say.
func (s *Server) Log() *eventlog.Log { return s.log }

// errorBody is the single error shape. A client can rely on `error` being a
// stable machine-readable code and `message` being the human sentence.
type errorBody struct {
	Error   string `json:"error"`
	Message string `json:"message"`
}

// writeJSONHeaders sends the status and the two headers every JSON response
// carries. Split out so the hand-written encoders can use it without going
// through the reflective path below.
func writeJSONHeaders(w http.ResponseWriter, status int) {
	setHeader(w.Header(), "Content-Type", "application/json; charset=utf-8")
	// The API is read fresh every time; a stale balance is worse than a
	// round trip.
	setHeader(w.Header(), "Cache-Control", "no-store")
	w.WriteHeader(status)
}

// encodeJSON writes one value with a hand-written encoder.
//
// The buffer is declared here, in the caller's frame, so the encoder itself
// allocates nothing - see jsonwriter.go for why that matters and what it
// cost to get right.
func encodeJSON(w http.ResponseWriter, status int, fn func(*jsonw)) {
	writeJSONHeaders(w, status)
	var mem [jsonBufSize]byte
	j := newJSONW(w, mem[:])
	fn(&j)
	_ = j.done()
}

// writeError encoders -------------------------------------------------------

// writeErrorBody sends an error without reflection. Errors are the one shape
// every single path can produce, including the ones that run when memory is
// already short.
func writeErrorBody(w http.ResponseWriter, status int, code, msg string) {
	encodeJSON(w, status, func(j *jsonw) {
		v := errorBody{Error: code, Message: msg}
		j.errorBody(&v)
	})
}

func writeRaw(w http.ResponseWriter, status int, body []byte) {
	setHeader(w.Header(), "Content-Type", "application/json; charset=utf-8")
	setHeader(w.Header(), "Cache-Control", "no-store")
	w.WriteHeader(status)
	_, _ = w.Write(body)
}

// writeError maps a domain error to a status and a code. Keeping this mapping
// in one place is what stops handlers from inventing their own vocabulary.
func writeError(w http.ResponseWriter, err error) {
	status, code := http.StatusInternalServerError, "internal"

	var insuf *ledger.InsufficientFundsError
	switch {
	case errors.Is(err, core.ErrForbidden):
		status, code = http.StatusForbidden, "forbidden"
	case errors.Is(err, core.ErrCapacity):
		status, code = http.StatusInsufficientStorage, "storage_full"
	case errors.Is(err, core.ErrBadInput):
		status, code = http.StatusBadRequest, "bad_request"
	case errors.As(err, &insuf), errors.Is(err, ledger.ErrInsufficient):
		status, code = http.StatusConflict, "insufficient_funds"
	case errors.Is(err, core.ErrUserNotFound):
		status, code = http.StatusNotFound, "user_not_found"
	case errors.Is(err, core.ErrAccountUnknown):
		status, code = http.StatusNotFound, "account_not_found"
	case errors.Is(err, core.ErrListingUnknown):
		status, code = http.StatusNotFound, "listing_not_found"
	case errors.Is(err, ledger.ErrNotFound):
		status, code = http.StatusNotFound, "transaction_not_found"
	case errors.Is(err, core.ErrListingClosed):
		status, code = http.StatusConflict, "listing_closed"
	case errors.Is(err, core.ErrOfferUnknown):
		status, code = http.StatusNotFound, "offer_not_found"
	case errors.Is(err, core.ErrOfferOrphaned):
		// 409, not 404: the offer exists, it is the thing it points at that
		// is gone. A 404 here reads as "no such offer", which sends whoever
		// is looking at it hunting for the wrong problem.
		status, code = http.StatusConflict, "offer_orphaned"
	case errors.Is(err, core.ErrOfferClosed):
		status, code = http.StatusConflict, "offer_closed"
	case errors.Is(err, core.ErrOfferSettled):
		status, code = http.StatusConflict, "offer_settled"
	case errors.Is(err, core.ErrQuoteUnknown):
		status, code = http.StatusNotFound, "quote_not_found"
	case errors.Is(err, core.ErrQuoteClosed):
		status, code = http.StatusConflict, "quote_closed"
	case errors.Is(err, core.ErrQuoteExpired):
		// 409 rather than 410: the quote is still there to look at, it just
		// cannot be traded, and a client showing a stale book needs to tell
		// those apart.
		status, code = http.StatusConflict, "quote_expired"
	case errors.Is(err, core.ErrSelfDeal):
		status, code = http.StatusBadRequest, "self_deal"
	case errors.Is(err, core.ErrUsernameTaken):
		status, code = http.StatusConflict, "username_taken"
	case errors.Is(err, core.ErrDisabled):
		status, code = http.StatusForbidden, "account_disabled"
	case errors.Is(err, ledger.ErrAlreadyReversed):
		status, code = http.StatusConflict, "already_reversed"
	case errors.Is(err, ledger.ErrReverseReversal):
		status, code = http.StatusConflict, "cannot_reverse_reversal"
	case errors.Is(err, ledger.ErrSystemAccount):
		status, code = http.StatusBadRequest, "system_account"
	case errors.Is(err, auth.ErrRateLimited):
		status, code = http.StatusTooManyRequests, "rate_limited"
	case errors.Is(err, auth.ErrTooManySessions):
		// A 503 without Retry-After, deliberately. The client retries a 503
		// only when the header tells it how long to wait, and waiting does
		// not help here: sessions free up on expiry, hours away, not in the
		// second or two a backoff covers. Sending one would turn a clear
		// refusal into three slow attempts ending the same way.
		//
		// Reaching this at all now means other users genuinely fill the
		// table - a login as the same person recycles their own oldest
		// session rather than adding to it. See newSessionLocked.
		status, code = http.StatusServiceUnavailable, "too_many_sessions"
	case errors.Is(err, auth.ErrNoSession):
		status, code = http.StatusUnauthorized, "unauthorized"
	case errors.Is(err, auth.ErrNoCode), errors.Is(err, auth.ErrPKCEMismatch),
		errors.Is(err, auth.ErrPKCEMethod), errors.Is(err, auth.ErrPKCELength),
		errors.Is(err, auth.ErrRedirectMismatch):
		status, code = http.StatusBadRequest, "invalid_grant"
	}
	writeErrorBody(w, status, code, err.Error())
}

func badRequest(w http.ResponseWriter, msg string) {
	writeErrorBody(w, http.StatusBadRequest, "bad_request", msg)
}

// decode reads a JSON body with a size limit and rejects unknown fields, so a
// misspelled key fails loudly instead of being silently ignored - which on a
// money API is the difference between a typo and a wrong transfer.
//
// A Decoder over the body, deliberately, despite a micro-benchmark showing
// json.Unmarshal to be cheaper in isolation (288 bytes against 1024).
// Replacing it with io.ReadAll plus Unmarshal measured *worse* end to end -
// 8745 bytes per request against 8183 - because ReadAll grows a buffer, and
// the growth cost more than the Decoder's internal reader saved.
//
// Recorded because the isolated figure is misleading: a layer's benchmark is
// not the layer's contribution to the whole, and on this API the whole is
// what runs out of heap.
// readBody reads a request body into a caller-owned buffer.
//
// Returns a sub-slice of buf, so the parser works over bytes the caller
// already owns and nothing is allocated for the body itself. On the board
// this buffer is the adapter's fixed per-worker request buffer, so a request
// body costs no heap at all.
func readBody(w http.ResponseWriter, r *http.Request, buf []byte) ([]byte, bool) {
	r.Body = http.MaxBytesReader(w, r.Body, MaxRequestBody)
	n := 0
	for n < len(buf) {
		read, err := r.Body.Read(buf[n:])
		n += read
		if err != nil {
			if err == io.EOF {
				break
			}
			badRequest(w, "could not read request body")
			return nil, false
		}
		if read == 0 {
			break
		}
	}
	if n == len(buf) {
		// One more byte would not fit, so the body is over the limit.
		var probe [1]byte
		if extra, _ := r.Body.Read(probe[:]); extra > 0 {
			badRequest(w, "request body is too large")
			return nil, false
		}
	}
	return buf[:n], true
}

// decodeInto parses a request body with a typed parser.
//
// Replaces the reflective json.Decoder, which measured 1056 B and 10
// allocations for a 68-byte transfer body - fifteen times the input, as ten
// scattered short-lived objects on the path every request takes. See
// jsonreader.go.
//
// The body buffer is declared here in the caller's frame, so on the desktop
// it is a stack array and on the board it comes from the adapter's fixed
// per-worker storage.
func decodeInto(w http.ResponseWriter, r *http.Request, parse func([]byte) error) bool {
	if buffered, ok := r.Body.(interface{ RemainingBody() []byte }); ok {
		body := buffered.RemainingBody()
		if len(body) > MaxDecodeBody {
			badRequest(w, "request body too large")
			return false
		}
		return parseBody(w, body, parse)
	}
	return decodeUnbuffered(w, r, parse)
}

// Keep the desktop fallback out of the buffered path: TinyGo otherwise hoists
// the escaping 1,400-byte local allocation before the interface branch.
//
//go:noinline
func decodeUnbuffered(w http.ResponseWriter, r *http.Request, parse func([]byte) error) bool {
	var buf [MaxDecodeBody]byte
	body, ok := readBody(w, r, buf[:])
	if !ok {
		return false
	}
	return parseBody(w, body, parse)
}

func parseBody(w http.ResponseWriter, body []byte, parse func([]byte) error) bool {
	if err := parse(body); err != nil {
		badRequest(w, "malformed request body: "+err.Error())
		return false
	}
	return true
}

// MaxDecodeBody bounds a request body during parsing.
//
// Smaller than MaxRequestBody, which is the desktop's limit: this is the
// stack buffer a handler declares, and it must cover the largest body the
// API accepts - a listing with an 80-character title and a 500-character
// description is about 700 bytes.
const MaxDecodeBody = 1400

// cors applies the origin policy.
//
// # Why this is a wildcard
//
// An empty AllowedOrigins means "any origin", answered with
// Access-Control-Allow-Origin: * rather than by echoing the request's Origin.
// That is the board's configuration, and it is a deliberate reading of the
// threat model rather than a shortcut.
//
// NanaCoin is a household promise tracker. Its balances have the financial
// weight of "I will give you a slice of pie if you eat your dinner", and an
// attacker who steals one has stolen a promise about pie. Against that, an
// exact-match allow list bought: a list to keep in step with every client
// deployment, a failure mode where a legitimate browser gets a 200 with no
// ACAO header and reports the board as unreachable (which cost a long
// debugging session and its own memory note), and about fifty bytes of
// response header on a device where the header buffer was already
// overflowing and silently dropping fields.
//
// What a wildcard actually exposes is narrow, because authentication is by
// bearer token and not by cookie: a hostile page cannot ride the user's
// credentials, since it does not have the token and the browser will not
// attach one for it. So the exposure is the unauthenticated endpoints only -
// /status, /logs and /provision. The first two carry no balances, no tokens
// and no passwords by construction. The third is why AllowProvision must be
// turned off once the household exists, which the board now does.
//
// An explicit list is still honoured when one is configured, for a
// deployment that wants one. Origins are matched exactly and echoed back;
// the request origin is never reflected unchecked (spec 18).
func (s *Server) cors(w http.ResponseWriter, r *http.Request) {
	origin := r.Header.Get("Origin")
	if origin == "" {
		return
	}

	if len(s.origins) == 0 {
		// No Vary: the response is identical for every origin, so there is
		// nothing for a cache to get wrong.
		setHeader(w.Header(), "Access-Control-Allow-Origin", "*")
		s.corsCommon(w)
		return
	}

	for _, allowed := range s.origins {
		if allowed == origin {
			s.log.Add(eventlog.Info, "cors-allow", origin)
			setHeader(w.Header(), "Access-Control-Allow-Origin", origin)
			// Vary matters because the response differs per origin, and a
			// shared cache must not serve one origin the headers granted
			// to another.
			addHeader(w.Header(), "Vary", "Origin")
			s.corsCommon(w)
			return
		}
	}
	// The browser will report this as a missing Access-Control-Allow-Origin,
	// which reads like a server misconfiguration rather than a policy
	// decision. Recording the origin that was refused is what turns that into
	// an answerable question.
	s.log.Add(eventlog.Warn, "cors-refuse", origin)
}

// corsCommon stages the fields that do not depend on which origin asked.
func (s *Server) corsCommon(w http.ResponseWriter) {
	setHeader(w.Header(), "Access-Control-Allow-Methods", "GET, POST, PATCH, OPTIONS")
	setHeader(w.Header(), "Access-Control-Allow-Headers", "Authorization, Content-Type, Idempotency-Key")
	// Without this the browser will not let the page read the health
	// header, which would make it useless from the client - only
	// response headers on the CORS safelist are exposed by default.
	setHeader(w.Header(), "Access-Control-Expose-Headers", "X-Nanacoin-Health")
	// Long, because the policy never changes at runtime. A preflight the
	// browser does not have to repeat is a whole request the board does not
	// have to serve.
	setHeader(w.Header(), "Access-Control-Max-Age", "86400")
}

// authenticate resolves the bearer token to a live, enabled user. A user
// disabled mid-session is rejected here, not at their token's expiry.
func (s *Server) authenticate(r *http.Request) (*users.User, error) {
	h := r.Header.Get("Authorization")
	const prefix = "Bearer "
	if len(h) <= len(prefix) || !strings.EqualFold(h[:len(prefix)], prefix) {
		return nil, auth.ErrNoSession
	}
	var sess auth.Session
	err := s.sessions.LookupInto(h[len(prefix):], &sess)
	if err != nil {
		return nil, err
	}
	u, ok := s.svc.User(sess.UserID)
	if !ok {
		return nil, auth.ErrNoSession
	}
	if !u.IsActive() {
		return nil, core.ErrDisabled
	}
	return u, nil
}

// require is the wrapper every authenticated handler uses.
func (s *Server) require(w http.ResponseWriter, r *http.Request) (*users.User, bool) {
	u, err := s.authenticate(r)
	if err != nil {
		writeError(w, err)
		return nil, false
	}
	return u, true
}

// pageSize reads a client's requested limit and clamps it to what this
// deployment can actually render. A client asking for more gets fewer, not an
// error: a shorter page is useful, and a 400 would leave the user with
// nothing.
func (s *Server) pageSize(r *http.Request, def int) int {
	n := queryInt(r, "limit", def)
	if n <= 0 {
		// ?limit=0 used to fall through to the caller's default, which on
		// the full-ledger endpoint is 100 - above the board's cap of 30. So
		// the one value that looks like "no limit" was the one that bypassed
		// the limit. Clamp the default too.
		n = def
	}
	if n > s.maxPage {
		return s.maxPage
	}
	return n
}

func queryInt(r *http.Request, key string, def int) int {
	v := r.URL.Query().Get(key)
	if v == "" {
		return def
	}
	n, err := strconv.Atoi(v)
	if err != nil || n < 0 {
		return def
	}
	return n
}
