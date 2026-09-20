package api

import "net/http"

// Reuse value slots supplied by the board's worker-owned headers. On the
// desktop these behave like Header.Set/Add, allocating only when capacity is
// absent. Keep the public ResponseWriter contract for middleware and tests.
func setHeader(h http.Header, key, value string) {
	key = http.CanonicalHeaderKey(key)
	v := h[key]
	if cap(v) == 0 {
		v = make([]string, 1)
	} else {
		v = v[:1]
	}
	v[0] = value
	h[key] = v
}

func addHeader(h http.Header, key, value string) {
	key = http.CanonicalHeaderKey(key)
	h[key] = append(h[key], value)
}
