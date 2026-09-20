package api

import (
	"encoding/json"
	"net/http/httptest"
	"testing"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
)

func TestMachineDiagnosticsContract(t *testing.T) {
	h := newHarness(t)
	s := NewServer(h.svc, auth.NewStore(auth.Options{}), Config{AllowedOrigins: []string{"https://allowed.test"}})
	s.SetDiagnostics(func() Diagnostics {
		return Diagnostics{LastBoot: "power on", Samples: 3, UptimeSeconds: 12,
			Machine: MachineDiagnostics{Enabled: true, SampledAtMS: 12345, Total: 10000, Free: 5000, Count: 8}}
	})
	s.SetMachineInfo(func() MachineInfo { return MachineInfo{Platform: "ESP32-S3 / TinyGo", Cores: 2, SharedBytes: 16} })
	for _, path := range []string{"/api/v1/diag", "/api/v1/diag/static"} {
		r := httptest.NewRequest("GET", path, nil)
		r.Header.Set("Origin", "https://allowed.test")
		w := httptest.NewRecorder()
		s.Handler().ServeHTTP(w, r)
		want := 200
		if !DiagEnabled {
			want = 404
		}
		if w.Code != want {
			t.Fatalf("%s: %d %s", path, w.Code, w.Body.String())
		}
		if !DiagEnabled {
			continue
		}
		if w.Header().Get("Cache-Control") != "no-store" {
			t.Fatal("diagnostics must not be cached")
		}
		if w.Header().Get("Access-Control-Allow-Origin") != "https://allowed.test" {
			t.Fatal("approved browser cannot read diagnostics")
		}
		var data map[string]any
		if err := json.Unmarshal(w.Body.Bytes(), &data); err != nil {
			t.Fatal(err)
		}
		if path == "/api/v1/diag" {
			if data["free_heap"] != float64(5000) || data["sampler_core"] != float64(0) || data["last_boot"] != "power on" {
				t.Fatal(data)
			}
			if value, ok := data["largest_free_block"]; !ok || value != nil {
				t.Fatal("unknown largest block must be null")
			}
		} else if data["active_cores"] != float64(1) || data["partitions_available"] != false {
			t.Fatal(data)
		}
		for _, method := range []string{"POST", "DELETE"} {
			w = httptest.NewRecorder()
			s.Handler().ServeHTTP(w, httptest.NewRequest(method, path, nil))
			if w.Code != 405 {
				t.Fatalf("mutation accepted: %d", w.Code)
			}
		}
		r = httptest.NewRequest("GET", path, nil)
		r.Header.Set("Origin", "https://evil.test")
		w = httptest.NewRecorder()
		s.Handler().ServeHTTP(w, r)
		if w.Header().Get("Access-Control-Allow-Origin") != "" {
			t.Fatal("unapproved origin received CORS permission")
		}
	}
}

func TestMachineInfoRequiresHostProvider(t *testing.T) {
	h := newHarness(t)
	w := httptest.NewRecorder()
	h.srv.ServeHTTP(w, httptest.NewRequest("GET", "/api/v1/diag/static", nil))
	if w.Code != 404 {
		t.Fatalf("no provider: got %d", w.Code)
	}
}

func TestMachineDiagnosticsEncodingDoesNotAllocate(t *testing.T) {
	d := Diagnostics{Machine: MachineDiagnostics{Enabled: true, Total: 400000, Free: 8000}}
	// Exclude host closure/interface escape of fixed scratch; measure whether
	// encoding itself grows storage after initialization.
	var mem [192]byte
	j := newJSONW(&allocSink, mem[:])
	if n := testing.AllocsPerRun(100, func() {
		j.n = 0
		j.err = nil
		j.diagnostics(&d)
		j.flush()
	}); n != 0 {
		t.Fatalf("allocations: %v", n)
	}
}
