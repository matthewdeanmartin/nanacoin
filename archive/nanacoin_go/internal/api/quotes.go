package api

import (
	"net/http"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Foreign exchange endpoints.
//
// The book is readable by anyone with a session, because a rate is public by
// nature - a market nobody can see is not a market. Taking a quote is
// idempotency-keyed like every other money-moving endpoint.

type quoteRequest struct {
	Side         string        `json:"side"`
	CentsPerCoin ledger.Amount `json:"cents_per_coin"`
	Coins        ledger.Amount `json:"coins"`
	ExpiresAt    int64         `json:"expires_at"`
}

type issueUSDRequest struct {
	To     ledger.AccountID `json:"to"`
	Cents  ledger.Amount    `json:"cents"`
	Reason string           `json:"reason"`
}

// handleListQuotes streams the book, best rate first.
//
// Streamed through a pooled record buffer like every other list endpoint. This
// one is polled: the client watches for a good rate, so it is the endpoint
// most likely to be called in a loop, and an allocating version would be the
// worst possible place for one.
func (s *Server) handleListQuotes(w http.ResponseWriter, r *http.Request) {
	if _, ok := s.require(w, r); !ok {
		return
	}
	now := s.svc.Now()

	record := <-s.freeRecords
	defer func() { s.freeRecords <- record }()

	sw := beginStream(w, http.StatusOK)
	sw.array("quotes", func(add func(func(*jsonw))) {
		n := lockedNamer{s.svc}
		s.svc.EachQuote(func(q *marketplace.Quote) bool {
			qv := n.quote(q, now)
			return record.prepare(sw, func(j *jsonw) { j.quoteView(&qv) })
		}, func() bool { return record.send(sw, add) })
	})
	if err := sw.end(); err != nil {
		s.log.Add(eventlog.Error, "stream-failed", "GET /quotes: "+err.Error())
	}
}

// handlePostQuote advertises a rate. No money moves.
func (s *Server) handlePostQuote(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req quoteRequest
	if !decodeInto(w, r, func(b []byte) error { return parseQuoteRequest(b, &req) }) {
		return
	}

	q, err := s.svc.PostQuote(
		actor,
		marketplace.ParseQuoteSide(req.Side),
		req.CentsPerCoin,
		req.Coins,
		req.ExpiresAt,
	)
	if err != nil {
		writeError(w, err)
		return
	}
	n := namer{s.svc}
	v := n.quote(q, s.svc.Now())
	encodeJSON(w, http.StatusCreated, func(j *jsonw) { j.quoteView(&v) })
}

// handleQuoteRoute serves the actions on one quote.
func (s *Server) handleQuoteRoute(w http.ResponseWriter, r *http.Request) {
	if base, ok := trimSuffix(r.URL.Path, "take"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleTakeQuote(w, r, ledger.QuoteID(lastSegment(base)))
		})(w, r)
		return
	}
	if base, ok := trimSuffix(r.URL.Path, "cancel"); ok {
		s.only("POST", func(w http.ResponseWriter, r *http.Request) {
			s.handleCancelQuote(w, r, ledger.QuoteID(lastSegment(base)))
		})(w, r)
		return
	}
	s.only("GET", func(w http.ResponseWriter, r *http.Request) {
		s.handleGetQuote(w, r, ledger.QuoteID(lastSegment(r.URL.Path)))
	})(w, r)
}

func (s *Server) handleGetQuote(w http.ResponseWriter, r *http.Request, id ledger.QuoteID) {
	if _, ok := s.require(w, r); !ok {
		return
	}
	q, found := s.svc.Quote(id)
	if !found {
		writeErrorBody(w, http.StatusNotFound, "quote_not_found", "no such quote")
		return
	}
	n := namer{s.svc}
	v := n.quote(q, s.svc.Now())
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.quoteView(&v) })
}

// handleTakeQuote executes a trade at the quoted rate.
func (s *Server) handleTakeQuote(w http.ResponseWriter, r *http.Request, id ledger.QuoteID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}

	body, err := s.svc.IdempotentInto(actor.ID, "take-quote", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		q, coin, cash, err := s.svc.TakeQuote(actor, id, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeTradeResult(n, q, coin, cash, s.svc.Now())
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

func (s *Server) handleCancelQuote(w http.ResponseWriter, r *http.Request, id ledger.QuoteID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	q, err := s.svc.CancelQuote(actor, id)
	if err != nil {
		writeError(w, err)
		return
	}
	n := namer{s.svc}
	v := n.quote(q, s.svc.Now())
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.quoteView(&v) })
}

// handleIssueUSD brings dollars into the household. Nana only, enforced in the
// service.
func (s *Server) handleIssueUSD(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req issueUSDRequest
	if !decodeInto(w, r, func(b []byte) error { return parseIssueUSDRequest(b, &req) }) {
		return
	}

	bufr := <-s.freeRecords
	defer s.releaseRecord(bufr)
	n := namer{s.svc}

	body, err := s.svc.IdempotentInto(actor.ID, "issue-usd", r.Header.Get("Idempotency-Key"), bufr.data[:], func() ([]byte, error) {
		txn, err := s.svc.IssueUSD(actor, req.To, req.Cents, req.Reason, &bufr.result)
		if err != nil {
			return nil, err
		}
		return bufr.encodeTransaction(n, txn)
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}
