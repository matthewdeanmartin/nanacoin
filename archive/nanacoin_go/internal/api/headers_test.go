package api

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
)

type bufferedTestBody struct {
	body       []byte
	readCalled bool
}

func (b *bufferedTestBody) Read([]byte) (int, error) { b.readCalled = true; return 0, io.EOF }
func (*bufferedTestBody) Close() error               { return nil }
func (b *bufferedTestBody) RemainingBody() []byte    { v := b.body; b.body = nil; return v }

func TestDecodeUsesExistingBoardBody(t *testing.T) {
	b := &bufferedTestBody{body: []byte(`{"amount":7}`)}
	r := httptest.NewRequest("POST", "/", nil)
	r.Body = b
	w := httptest.NewRecorder()
	var amount int
	if !decodeInto(w, r, func(v []byte) error {
		if string(v) != `{"amount":7}` {
			t.Fatal(string(v))
		}
		amount = 7
		return nil
	}) || b.readCalled || amount != 7 || b.body != nil {
		t.Fatal("did not consume existing buffer")
	}
	b.body = make([]byte, MaxDecodeBody+1)
	if decodeInto(w, r, func([]byte) error { t.Fatal("oversized body parsed"); return nil }) {
		t.Fatal("oversized body accepted")
	}
}

func TestHeaderReusePreservesSetAndAddSemantics(t *testing.T) {
	var slots [2]string
	h := http.Header{"Vary": slots[:0]}
	if n := testing.AllocsPerRun(100, func() {
		h["Vary"] = slots[:0]
		addHeader(h, "Vary", "Origin")
		addHeader(h, "Vary", "Accept")
		setHeader(h, "Vary", "Authorization")
	}); n != 0 {
		t.Fatal(n)
	}
	if len(h.Values("Vary")) != 1 || h.Get("Vary") != "Authorization" {
		t.Fatal(h)
	}
	setHeader(h, "content-type", "application/json")
	if h.Get("Content-Type") != "application/json" {
		t.Fatal(h)
	}
}
