package api

import (
	"net/http"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/eventlog"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/ledger"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/marketplace"
)

// Offer endpoints.
//
// The money-moving ones - accept and unaccept - are idempotency-keyed like
// every other money-moving endpoint, so a retry after a dropped connection
// returns the original result rather than accepting twice.

type offerRequest struct {
	Amount  ledger.Amount `json:"amount"`
	Message string        `json:"message"`
}

type unacceptRequest struct {
	Reason string `json:"reason"`
}

// handleListOffers streams the offers the caller may see.
//
// Streamed through a pooled record buffer like every other list endpoint. The
// first version built a slice of offers and a slice of views per request, and
// the board ran out of memory after a handful of calls - each view carries
// strings copied out of the arena, so a full table was 32 of them live at
// once on a device with about 8 KB of headroom.
func (s *Server) handleListOffers(w http.ResponseWriter, r *http.Request) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	now := s.svc.Now()

	record := <-s.freeRecords
	defer func() { s.freeRecords <- record }()

	sw := beginStream(w, http.StatusOK)
	sw.array("offers", func(add func(func(*jsonw))) {
		n := lockedNamer{s.svc}
		s.svc.EachOffer(actor, func(o *marketplace.Offer) bool {
			ov := n.offer(o, n.listingTitle(o.Listing), now)
			return record.prepare(sw, func(j *jsonw) { j.offerView(&ov) })
		}, func() bool { return record.send(sw, add) })
	})
	if err := sw.end(); err != nil {
		s.log.Add(eventlog.Error, "stream-failed", "GET /offers: "+err.Error())
	}
}

// listingTitle captions an offer with what it is for. An offer whose listing
// has been recycled out of the table still renders, with an empty title,
// rather than failing the whole list.
//
// The unlocked form, for the single-offer handlers. The streaming list uses
// lockedNamer.listingTitle, because it is already inside the service lock.
func (s *Server) listingTitle(id ledger.ListingID) string {
	if l, ok := s.svc.Listing(id); ok {
		return l.Title
	}
	return ""
}

// handleMakeOffer proposes a deal. No money moves.
func (s *Server) handleMakeOffer(w http.ResponseWriter, r *http.Request, listing ledger.ListingID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req offerRequest
	if !decodeInto(w, r, func(b []byte) error { return parseOfferRequest(b, &req) }) {
		return
	}

	offer, err := s.svc.MakeOffer(actor, listing, req.Amount, req.Message)
	if err != nil {
		writeError(w, err)
		return
	}
	n := namer{s.svc}
	v := n.offer(offer, s.listingTitle(offer.Listing), s.svc.Now())
	encodeJSON(w, http.StatusCreated, func(j *jsonw) { j.offerView(&v) })
}

// handleAcceptOffer moves the money and closes the listing.
func (s *Server) handleAcceptOffer(w http.ResponseWriter, r *http.Request, id ledger.OfferID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}

	body, err := s.svc.IdempotentInto(actor.ID, "accept-offer", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		offer, txn, err := s.svc.AcceptOffer(actor, id, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeOfferResult(n, offer, txn, s.listingTitle(offer.Listing), s.svc.Now())
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusCreated, body)
}

// handleUnacceptOffer undoes an acceptance inside its settlement window.
func (s *Server) handleUnacceptOffer(w http.ResponseWriter, r *http.Request, id ledger.OfferID) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}
	var req unacceptRequest
	if !decodeInto(w, r, func(b []byte) error { return parseUnacceptRequest(b, &req) }) {
		return
	}

	b := <-s.freeRecords
	defer s.releaseRecord(b)
	n := namer{s.svc}

	body, err := s.svc.IdempotentInto(actor.ID, "unaccept-offer", r.Header.Get("Idempotency-Key"), b.data[:], func() ([]byte, error) {
		offer, txn, err := s.svc.UnacceptOffer(actor, id, req.Reason, &b.result)
		if err != nil {
			return nil, err
		}
		return b.encodeOfferResult(n, offer, txn, s.listingTitle(offer.Listing), s.svc.Now())
	})
	if err != nil {
		writeError(w, err)
		return
	}
	writeRaw(w, http.StatusOK, body)
}

// handleCloseOffer declines or withdraws, depending on the action.
func (s *Server) handleCloseOffer(
	w http.ResponseWriter,
	r *http.Request,
	id ledger.OfferID,
	to marketplace.OfferStatus,
) {
	actor, ok := s.require(w, r)
	if !ok {
		return
	}

	var offer *marketplace.Offer
	var err error
	if to == marketplace.OfferWithdrawn {
		offer, err = s.svc.WithdrawOffer(actor, id)
	} else {
		offer, err = s.svc.DeclineOffer(actor, id)
	}
	if err != nil {
		writeError(w, err)
		return
	}

	n := namer{s.svc}
	v := n.offer(offer, s.listingTitle(offer.Listing), s.svc.Now())
	encodeJSON(w, http.StatusOK, func(j *jsonw) { j.offerView(&v) })
}
