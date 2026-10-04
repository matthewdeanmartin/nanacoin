package boardhttp

import (
	"bytes"
	"errors"
	"io"
	"net/http"
	"net/url"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/soypat/lneto"
	"github.com/soypat/lneto/http/httphi"
)

type testConn struct {
	input  *strings.Reader
	output bytes.Buffer
}

func (c *testConn) Read(p []byte) (int, error)  { return c.input.Read(p) }
func (c *testConn) Write(p []byte) (int, error) { return c.output.Write(p) }
func (*testConn) Close() error                  { return nil }

func exchangeRequest(t *testing.T, h *Handler, request string) string {
	t.Helper()
	c := &testConn{input: strings.NewReader(request)}
	e := new(httphi.Exchange)
	e.Configure(httphi.ExchangeConfig{RawBuf: make([]byte, 4096), RequestBufferLim: 2048, NumHeaderKVCap: 32})
	if !e.Acquire(c) {
		t.Fatal("acquire")
	}
	defer e.Release()
	m := new(httphi.MuxSlice)
	m.Handle("GET /test", h.serve)
	m.Handle("POST /test", h.serve)
	if err := httphi.Handle(e, m, func(uint) time.Duration { return lneto.BackoffFlagNop }); err != nil {
		t.Fatal(err)
	}
	return c.output.String()
}

func TestWorkerReuseDoesNotLeakCredentialsOrResponseState(t *testing.T) {
	var retained []string
	secret := true
	h := New(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if secret {
			retained = append(retained, r.Header.Get("Authorization"))
			w.Header().Set("X-Private", "private-response")
			w.WriteHeader(201)
		} else {
			if r.Header.Get("Authorization") != "" || r.Header.Get("Origin") != "" || r.URL.RawQuery != "" || r.ContentLength != 0 {
				t.Errorf("previous request leaked: %v", r)
			}
		}
		io.WriteString(w, "body")
	}), nil)
	for i := 0; i < Workers; i++ {
		exchangeRequest(t, h, "GET /test?secret=yes HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer secret\r\nOrigin: http://private\r\n\r\n")
	}
	secret = false
	for i := 0; i < Workers*3; i++ {
		got := exchangeRequest(t, h, "GET /test HTTP/1.1\r\nHost: local\r\n\r\n")
		if !strings.HasPrefix(got, "HTTP/1.1 200") || strings.Contains(got, "private-response") || !strings.HasSuffix(got, "body") {
			t.Fatal(got)
		}
	}
	for _, s := range retained {
		if s != "Bearer secret" {
			t.Fatal("retained string mutated")
		}
	}
	for i := 0; i < Workers; i++ {
		b := <-h.free
		if b.request.Body != nil || b.request.URL != nil || b.writer.exch != nil || b.body.Len() != 0 {
			t.Fatal("idle worker retains request")
		}
		for _, v := range b.requestHeaders.values {
			if v != [2]string{} {
				t.Fatal("idle worker retains headers")
			}
		}
	}
}

func TestShortBodyRejectedAndWorkerReusable(t *testing.T) {
	calls := 0
	h := New(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { calls++; w.WriteHeader(204) }), nil)
	for i := 0; i < Workers*2; i++ {
		got := exchangeRequest(t, h, "POST /test HTTP/1.1\r\nHost: local\r\nContent-Length: 8\r\n\r\nabc")
		if !strings.HasPrefix(got, "HTTP/1.1 400") {
			t.Fatal(got)
		}
	}
	if calls != 0 || len(h.free) != Workers {
		t.Fatal("bad body entered handler or leaked worker")
	}
	got := exchangeRequest(t, h, "GET /test HTTP/1.1\r\nHost: local\r\n\r\n")
	if !strings.HasPrefix(got, "HTTP/1.1 204") || calls != 1 {
		t.Fatal(got)
	}
}

func TestConcurrentWorkersOwnIndependentState(t *testing.T) {
	entered := make(chan struct{}, Workers)
	release := make(chan struct{})
	h := New(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		before := r.Header.Get("Authorization")
		entered <- struct{}{}
		<-release
		if r.Header.Get("Authorization") != before {
			t.Error("request overwritten while serving")
		}
		io.WriteString(w, before)
	}), nil)
	var wg sync.WaitGroup
	for i := 0; i < Workers; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			token := strings.Repeat("x", i+1)
			got := exchangeRequest(t, h, "GET /test HTTP/1.1\r\nHost: local\r\nAuthorization: "+token+"\r\n\r\n")
			if !strings.HasSuffix(got, token) {
				t.Error(got)
			}
		}(i)
	}
	for i := 0; i < Workers; i++ {
		select {
		case <-entered:
		case <-time.After(time.Second * 5):
			t.Fatal("worker blocked")
		}
	}
	close(release)
	wg.Wait()
	if len(h.free) != Workers {
		t.Fatal("worker leaked")
	}
}

func TestReleaseClearsFailedResponse(t *testing.T) {
	b := &buffers{chunk: make([]byte, 16)}
	b.init()
	b.writer.err = errors.New("broken socket")
	b.writer.n = 15
	b.writer.wroteHeader = true
	b.writer.status = 500
	b.release()
	if b.writer.err != nil || b.writer.n != 0 || b.writer.wroteHeader || b.writer.status != 0 {
		t.Fatal("failure retained")
	}
}

func FuzzTargetMatchesStandardParser(f *testing.F) {
	for _, s := range []string{"/test", "/test?a=1", "/test?", "/a%2fb?q=%20", "//host/path", "*", "http://host/path", "/test#frag", "/bad\x00"} {
		f.Add(s)
	}
	f.Fuzz(func(t *testing.T, s string) {
		var got url.URL
		err := parseTarget(&got, s)
		want, we := url.ParseRequestURI(s)
		if (err == nil) != (we == nil) {
			t.Fatalf("error mismatch for %q: %v / %v", s, err, we)
		}
		if err == nil && !reflect.DeepEqual(&got, want) {
			t.Fatalf("%q: %#v != %#v", s, &got, want)
		}
	})
}

func TestOrdinaryTargetDoesNotAllocate(t *testing.T) {
	var u url.URL
	if n := testing.AllocsPerRun(100, func() {
		if err := parseTarget(&u, "/api/v1/status?limit=30"); err != nil {
			panic(err)
		}
	}); n != 0 {
		t.Fatal(n)
	}
}

func TestBufferedBodyConsumesOnlyUnreadBytes(t *testing.T) {
	var b bodyReader
	b.Reset([]byte("abcdef"))
	var prefix [2]byte
	_, _ = b.Read(prefix[:])
	if string(b.RemainingBody()) != "cdef" || b.Len() != 0 || len(b.RemainingBody()) != 0 {
		t.Fatal("incorrect buffered read position")
	}
	b.Reset(nil)
	if b.data != nil {
		t.Fatal("retained body")
	}
}

func BenchmarkAdapterRead(b *testing.B) {
	h := New(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { io.WriteString(w, "{\"ok\":true}") }), nil)
	m := new(httphi.MuxSlice)
	m.Handle("GET /test", h.serve)
	c := &testConn{input: new(strings.Reader)}
	e := new(httphi.Exchange)
	e.Configure(httphi.ExchangeConfig{RawBuf: make([]byte, 4096), RequestBufferLim: 2048, NumHeaderKVCap: 32})
	run := func() {
		c.input.Reset("GET /test HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer test\r\n\r\n")
		c.output.Reset()
		if !e.Acquire(c) {
			b.Fatal("acquire")
		}
		if err := httphi.Handle(e, m, func(uint) time.Duration { return lneto.BackoffFlagNop }); err != nil {
			b.Fatal(err)
		}
		e.Release()
	}
	run()
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		run()
	}
}
