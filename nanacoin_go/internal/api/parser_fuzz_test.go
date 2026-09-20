package api

import (
	"encoding/json"
	"reflect"
	"testing"
)

// Generate requests independently of the hand-written parser.
func FuzzTransferParserAgainstJSON(f *testing.F) {
	f.Add("account-alice", int64(1), "hello")
	f.Add("\"\\", int64(9223372036854775807), "\n\t\u2603")
	f.Fuzz(func(t *testing.T, to string, amount int64, memo string) {
		if len(to)+len(memo) > 2048 {
			return
		}
		body, e := json.Marshal(map[string]any{"to": to, "amount": amount, "memo": memo})
		if e != nil {
			t.Fatal(e)
		}
		var want, got transferRequest
		if e = json.Unmarshal(body, &want); e != nil {
			return
		}
		if e = parseTransferRequest(body, &got); e != nil {
			t.Fatalf("valid JSON rejected: %s: %v", body, e)
		}
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("got %#v want %#v", got, want)
		}
	})
}
