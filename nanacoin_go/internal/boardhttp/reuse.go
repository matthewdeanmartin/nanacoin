package boardhttp

import (
	"bytes"
	"net/http"
	"net/url"
	"strings"
)

var requestHeaderKeys = []string{
	"Authorization", "Content-Type", "Idempotency-Key", "Origin",
	"Access-Control-Request-Method", "Access-Control-Request-Headers",
}

var responseHeaderKeys = []string{
	"Content-Type", "Cache-Control", "Access-Control-Allow-Origin", "Vary",
	"Access-Control-Allow-Methods", "Access-Control-Allow-Headers",
	"Access-Control-Expose-Headers", "Access-Control-Max-Age",
	"X-Nanacoin-Health", "Allow", "Retry-After",
}

// These maps and value slots belong to one admitted worker. Empty slices keep
// their backing storage while Header.Get and header staging see an absent value.
type reusableHeaders struct {
	header http.Header
	values [12][2]string
	keys   []string
}

func (h *reusableHeaders) init(keys []string) {
	h.keys = keys
	h.header = make(http.Header, len(keys))
	h.reset()
}

func (h *reusableHeaders) reset() {
	clear(h.header)
	clear(h.values[:])
	for i, key := range h.keys {
		h.header[key] = h.values[i][:0]
	}
}

type bodyReader struct {
	bytes.Reader
	data []byte
}

func (b *bodyReader) Reset(data []byte) { b.data = data; b.Reader.Reset(data) }

// RemainingBody lends the already buffered body until ServeHTTP returns.
// It consumes the reader just like reading to EOF. API parsers copy any strings
// that become persistent domain state.
func (b *bodyReader) RemainingBody() []byte {
	n := b.Len()
	data := b.data[len(b.data)-n:]
	_, _ = b.Seek(0, 2)
	return data
}

func (*bodyReader) Close() error { return nil }

func (b *buffers) init() {
	b.requestHeaders.init(requestHeaderKeys)
	b.responseHeaders.init(responseHeaderKeys)
	b.writer.header = b.responseHeaders.header
	b.writer.chunk = b.chunk
}

// Release before returning to the pool, including on malformed requests. In
// particular, idle workers must not retain auth strings, URLs or connections.
func (b *buffers) release() {
	b.served = false
	b.request = http.Request{}
	b.uri = url.URL{}
	b.body.Reset(nil)
	b.requestHeaders.reset()
	b.responseHeaders.reset()
	b.writer.exch = nil
	b.writer.n = 0
	b.writer.status = 0
	b.writer.wroteHeader = false
	b.writer.err = nil
}

// Ordinary origin-form API targets need no allocated URL object or unescaping.
// Unusual/escaped forms use the standard parser to preserve its validation and
// semantics. Strings remain owned copies, so handlers may safely retain values.
func parseTarget(dst *url.URL, target string) error {
	*dst = url.URL{}
	if len(target) > 0 && target[0] == '/' && !strings.ContainsAny(target, "%#") {
		path, query, found := strings.Cut(target, "?")
		ordinary := true
		for i := range target {
			if target[i] <= 0x20 || target[i] >= 0x7f {
				ordinary = false
				break
			}
		}
		// Other path punctuation can cause the standard parser to preserve a
		// RawPath even without percent escapes (for example /!).
		for i := range path {
			c := path[i]
			if !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || strings.ContainsRune("/-._~", rune(c))) {
				ordinary = false
				break
			}
		}
		if ordinary {
			dst.Path, dst.RawQuery = path, query
			dst.ForceQuery = found && query == ""
			return nil
		}
	}
	u, err := url.ParseRequestURI(target)
	if err != nil {
		return err
	}
	*dst = *u
	return nil
}
