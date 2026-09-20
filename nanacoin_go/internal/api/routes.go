package api

import (
	"io"
	"net/http"
	"strings"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Handler builds the router.
//
// The routes are registered with classic path-only patterns and the method is
// checked inside each handler, rather than using Go 1.22's "GET /path" pattern
// syntax. That syntax is much nicer to read, but TinyGo's net/http does not
// implement it: every method-prefixed pattern silently fails to match and the
// whole API 404s on the board. Since the same handler has to serve both
// targets, the router uses the subset both understand.
//
// Path variables are extracted with trailing-slash patterns and lastSegment
// for the same reason - r.PathValue depends on the same missing machinery.
func (s *Server) Handler() http.Handler {
	mux := http.NewServeMux()

	const v1 = "/api/v1"

	mux.HandleFunc(v1+"/status", s.only("GET", s.handleStatus))
	if s.logsEnabled() {
		mux.HandleFunc(v1+"/logs", s.only("GET", s.handleLogs))
	}
	if DiagEnabled {
		mux.HandleFunc(v1+"/diag", s.only("GET", s.handleDiag))
		mux.HandleFunc(v1+"/diag/static", s.only("GET", s.handleDiagStatic))
	}
	mux.HandleFunc(v1+"/provision", s.only("POST", s.handleProvision))

	mux.HandleFunc(v1+"/auth/authorize", s.only("POST", s.handleAuthorize))
	mux.HandleFunc(v1+"/auth/token", s.only("POST", s.handleToken))
	mux.HandleFunc(v1+"/auth/logout", s.only("POST", s.handleLogout))

	mux.HandleFunc(v1+"/me", s.only("GET", s.handleMe))

	// Collection and item share a prefix, so one handler dispatches on
	// whether a path segment follows.
	mux.HandleFunc(v1+"/users", s.byMethod(map[string]http.HandlerFunc{
		"GET":  s.handleListUsers,
		"POST": s.handleCreateUser,
	}))
	mux.HandleFunc(v1+"/users/", s.only("PATCH", s.handleUpdateUser))

	mux.HandleFunc(v1+"/accounts/", s.only("GET", s.handleAccountRoute))

	mux.HandleFunc(v1+"/transfers", s.only("POST", s.handleTransfer))

	mux.HandleFunc(v1+"/transactions", s.only("GET", s.handleAllTransactions))
	mux.HandleFunc(v1+"/transactions/", s.handleTransactionRoute)

	mux.HandleFunc(v1+"/admin/issue", s.only("POST", s.handleIssue))
	mux.HandleFunc(v1+"/admin/retire", s.only("POST", s.handleRetire))
	mux.HandleFunc(v1+"/admin/issue-usd", s.only("POST", s.handleIssueUSD))
	mux.HandleFunc(v1+"/admin/config", s.byMethod(map[string]http.HandlerFunc{
		"GET":   s.handleGetConfig,
		"PATCH": s.handleSetConfig,
	}))

	mux.HandleFunc(v1+"/listings", s.byMethod(map[string]http.HandlerFunc{
		"GET":  s.handleListListings,
		"POST": s.handleCreateListing,
	}))
	mux.HandleFunc(v1+"/listings/", s.handleListingRoute)

	mux.HandleFunc(v1+"/quotes", s.byMethod(map[string]http.HandlerFunc{
		"GET":  s.handleListQuotes,
		"POST": s.handlePostQuote,
	}))
	mux.HandleFunc(v1+"/quotes/", s.handleQuoteRoute)

	mux.HandleFunc(v1+"/offers", s.only("GET", s.handleListOffers))
	mux.HandleFunc(v1+"/offers/", s.handleOfferRoute)

	return s.middleware(mux)
}

// only wraps a handler so that it answers one method and rejects the rest with
// 405 rather than running.
func (s *Server) only(method string, h http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method != method {
			setHeader(w.Header(), "Allow", method)
			writeErrorBody(w, http.StatusMethodNotAllowed, "method_not_allowed",
				"this endpoint accepts "+method)
			return
		}
		h(w, r)
	}
}

// byMethod dispatches one path across several methods.
func (s *Server) byMethod(handlers map[string]http.HandlerFunc) http.HandlerFunc {
	allow := make([]string, 0, len(handlers))
	for m := range handlers {
		allow = append(allow, m)
	}
	allowed := strings.Join(allow, ", ")

	return func(w http.ResponseWriter, r *http.Request) {
		if h, ok := handlers[r.Method]; ok {
			h(w, r)
			return
		}
		setHeader(w.Header(), "Allow", allowed)
		writeErrorBody(w, http.StatusMethodNotAllowed, "method_not_allowed",
			"this endpoint accepts "+allowed)
	}
}

// lastSegment returns the final path segment, which is the {id} in every
// item route this API has. It replaces r.PathValue, which TinyGo lacks.
func lastSegment(path string) string {
	if i := strings.LastIndex(path, "/"); i >= 0 {
		return path[i+1:]
	}
	return path
}

// trimSuffix reports the path with a known trailing action removed, and
// whether it was there. Used to tell /transactions/{id} from
// /transactions/{id}/reverse without a pattern language.
func trimSuffix(path, action string) (string, bool) {
	suffix := "/" + action
	if strings.HasSuffix(path, suffix) {
		return strings.TrimSuffix(path, suffix), true
	}
	return path, false
}

// handleAccountRoute serves /accounts/{id} and /accounts/{id}/transactions.
func (s *Server) handleAccountRoute(w http.ResponseWriter, r *http.Request) {
	if base, ok := trimSuffix(r.URL.Path, "transactions"); ok {
		s.handleAccountTransactions(w, r, ledger.AccountID(lastSegment(base)))
		return
	}
	s.handleAccount(w, r, ledger.AccountID(lastSegment(r.URL.Path)))
}

// handleTransactionRoute serves /transactions/{id} and
// /transactions/{id}/reverse.
func (s *Server) handleTransactionRoute(w http.ResponseWriter, r *http.Request) {
	if base, ok := trimSuffix(r.URL.Path, "reverse"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleReverse(w, r, ledger.TransactionID(lastSegment(base)))
		})(w, r)
		return
	}
	s.only("GET", func(w http.ResponseWriter, r *http.Request) {
		s.handleTransaction(w, r, ledger.TransactionID(lastSegment(r.URL.Path)))
	})(w, r)
}

// handleListingRoute serves /listings/{id} plus its purchase and cancel
// actions.
func (s *Server) handleListingRoute(w http.ResponseWriter, r *http.Request) {
	if base, ok := trimSuffix(r.URL.Path, "purchase"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handlePurchase(w, r, ledger.ListingID(lastSegment(base)))
		})(w, r)
		return
	}
	if base, ok := trimSuffix(r.URL.Path, "cancel"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleCancelListing(w, r, ledger.ListingID(lastSegment(base)))
		})(w, r)
		return
	}
	if base, ok := trimSuffix(r.URL.Path, "offers"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleMakeOffer(w, r, ledger.ListingID(lastSegment(base)))
		})(w, r)
		return
	}
	id := ledger.ListingID(lastSegment(r.URL.Path))
	s.byMethod(map[string]http.HandlerFunc{
		"GET":   func(w http.ResponseWriter, r *http.Request) { s.handleListing(w, r, id) },
		"PATCH": func(w http.ResponseWriter, r *http.Request) { s.handleUpdateListing(w, r, id) },
	})(w, r)
}

// handleOfferRoute serves the actions on one offer. Like every other route
// here it dispatches on a trailing segment rather than a pattern language,
// because TinyGo's net/http has none.
func (s *Server) handleOfferRoute(w http.ResponseWriter, r *http.Request) {
	if base, ok := trimSuffix(r.URL.Path, "accept"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleAcceptOffer(w, r, ledger.OfferID(lastSegment(base)))
		})(w, r)
		return
	}
	// "unaccept" rather than "reverse": reversing is a ledger operation Nana
	// does to any transaction, and this is a narrower thing - either party
	// undoing a deal inside its settlement window.
	if base, ok := trimSuffix(r.URL.Path, "unaccept"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleUnacceptOffer(w, r, ledger.OfferID(lastSegment(base)))
		})(w, r)
		return
	}
	if base, ok := trimSuffix(r.URL.Path, "decline"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleCloseOffer(w, r, ledger.OfferID(lastSegment(base)), marketplace.OfferDeclined)
		})(w, r)
		return
	}
	if base, ok := trimSuffix(r.URL.Path, "withdraw"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleCloseOffer(w, r, ledger.OfferID(lastSegment(base)), marketplace.OfferWithdrawn)
		})(w, r)
		return
	}
	writeErrorBody(w, http.StatusNotFound, "not_found", "no such offer action")
}

// middleware applies CORS to every response and answers preflights before
// routing. Preflight is handled here rather than per-route because a browser
// sends OPTIONS to paths the mux would otherwise reject.
func (s *Server) middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		s.cors(w, r)
		if r.Method == http.MethodOptions {
			// Only a request the policy actually allowed gets a 204; an
			// origin that is not on the list gets no CORS headers above,
			// and the browser blocks it.
			s.log.Add(eventlog.Info, "preflight", r.Method+" "+r.URL.Path)
			w.WriteHeader(http.StatusNoContent)
			return
		}

		// Record the status every request got. This is the log's main job: a
		// request that is absent here never reached the server, and one that
		// is present with a status was answered - two conclusions a browser
		// error message cannot distinguish.
		//
		// The log endpoint is skipped, or reading the log would fill it.
		if strings.HasSuffix(r.URL.Path, "/logs") || strings.HasSuffix(r.URL.Path, "/diag") || strings.HasSuffix(r.URL.Path, "/diag/static") {
			next.ServeHTTP(w, r)
			return
		}

		// Put the host's health on the response itself, before the handler
		// writes it. This is the only diagnostic that survives a crash: once
		// the board is out of memory it cannot serve /logs either, so the
		// headers of the last request that *did* succeed are the final
		// reading anyone gets.
		//
		// A header rather than the body, so it is on every response - including
		// the ones whose bodies are defined shapes that must not grow a
		// diagnostic field.
		if h := s.healthLine(); h != "" {
			setHeader(w.Header(), "X-Nanacoin-Health", h)
		}

		rec := &statusRecorder{ResponseWriter: w, status: http.StatusOK}
		next.ServeHTTP(rec, r)

		level := eventlog.Info
		switch {
		case rec.status >= 500:
			level = eventlog.Error
		case rec.status >= 400:
			level = eventlog.Warn
		}
		s.log.Request(level, rec.status, r.Method, r.URL.Path)
	})
}

// statusRecorder remembers the status a handler wrote, so the middleware can
// log it. It forwards everything else untouched.
type statusRecorder struct {
	http.ResponseWriter
	status      int
	wroteHeader bool
}

func (r *statusRecorder) WriteHeader(status int) {
	if !r.wroteHeader {
		r.status = status
		r.wroteHeader = true
	}
	r.ResponseWriter.WriteHeader(status)
}

func (r *statusRecorder) Write(p []byte) (int, error) {
	// A handler that writes without WriteHeader has implicitly sent 200,
	// which is already the zero value here.
	r.wroteHeader = true
	return r.ResponseWriter.Write(p)
}

func (r *statusRecorder) WriteString(s string) (int, error) {
	r.wroteHeader = true
	return io.WriteString(r.ResponseWriter, s)
}
