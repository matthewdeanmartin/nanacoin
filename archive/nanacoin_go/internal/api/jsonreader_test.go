package api

import (
	"encoding/json"
	"reflect"
	"testing"
)

// The hand-written parsers must accept exactly what encoding/json accepts,
// and produce the same struct.
//
// This is the safety net for dropping reflection on the input side. A field
// added to a request struct and not added to its parser is *silently
// ignored* - on a money API that is worse than a crash, because a transfer
// would go through with a field the client set and the server never saw.
//
// So every request type is parsed both ways and compared, with full bodies,
// partial bodies, and the awkward values a person can type.
func TestParsersMatchStdlib(t *testing.T) {
	cases := []struct {
		name  string
		body  string
		parse func([]byte) (any, error)
		std   func([]byte) (any, error)
	}{
		{
			"provision/full",
			`{"username":"nana","display_name":"Nana","password":"nana-pin","household_name":"The House"}`,
			func(b []byte) (any, error) {
				var v provisionRequest
				return &v, parseProvisionRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v provisionRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"provision/partial",
			`{"username":"nana","password":"pin"}`,
			func(b []byte) (any, error) {
				var v provisionRequest
				return &v, parseProvisionRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v provisionRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"transfer",
			`{"to":"account-grHEnttNRZQA","amount":5,"memo":"for mowing the lawn"}`,
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, parseTransferRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"transfer/negative and escapes",
			`{"to":"a","amount":-42,"memo":"say \"hi\"\nand \\ that"}`,
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, parseTransferRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"transfer/unicode escape",
			`{"to":"a","amount":1,"memo":"café 日本"}`,
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, parseTransferRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"transfer/raw utf8",
			`{"to":"a","amount":1,"memo":"café 日本 🎉"}`,
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, parseTransferRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"createUser/with grant",
			`{"username":"alice","display_name":"Alice","password":"pin","role":"user","grant":false}`,
			func(b []byte) (any, error) {
				var v createUserRequest
				return &v, parseCreateUserRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v createUserRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"createUser/no grant field",
			`{"username":"alice","password":"pin"}`,
			func(b []byte) (any, error) {
				var v createUserRequest
				return &v, parseCreateUserRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v createUserRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"updateUser/all optionals",
			`{"display_name":"New","status":"DISABLED","role":"nana","password":"newpin"}`,
			func(b []byte) (any, error) {
				var v updateUserRequest
				return &v, parseUpdateUserRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v updateUserRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"updateUser/one optional",
			`{"display_name":"Just the name"}`,
			func(b []byte) (any, error) {
				var v updateUserRequest
				return &v, parseUpdateUserRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v updateUserRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"createListing",
			`{"title":"Switch hour","description":"Uninterrupted","price":10,"kind":"currency","currency":"USD","minor_units":500}`,
			func(b []byte) (any, error) {
				var v createListingRequest
				return &v, parseCreateListingRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v createListingRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"updateListing/price only",
			`{"price":42}`,
			func(b []byte) (any, error) {
				var v updateListingRequest
				return &v, parseUpdateListingRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v updateListingRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"setConfig",
			`{"household_name":"H","initial_grant":100,"currency":"NanaCoin"}`,
			func(b []byte) (any, error) {
				var v setConfigRequest
				return &v, parseSetConfigRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v setConfigRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"authorize",
			`{"username":"nana","password":"pin","code_challenge":"abc","code_challenge_method":"S256","redirect_uri":"http://localhost:4200/cb"}`,
			func(b []byte) (any, error) {
				var v authorizeRequest
				return &v, parseAuthorizeRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v authorizeRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"whitespace everywhere",
			"{ \"to\" : \"a\" , \"amount\" : 7 , \"memo\" : \"x\" }",
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, parseTransferRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
		{
			"empty object",
			`{}`,
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, parseTransferRequest(b, &v)
			},
			func(b []byte) (any, error) {
				var v transferRequest
				return &v, json.Unmarshal(b, &v)
			},
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			ours, ourErr := tc.parse([]byte(tc.body))
			theirs, theirErr := tc.std([]byte(tc.body))

			if (ourErr != nil) != (theirErr != nil) {
				t.Fatalf("error disagreement\n  ours:   %v\n  stdlib: %v",
					ourErr, theirErr)
			}
			if ourErr != nil {
				return
			}
			if !reflect.DeepEqual(ours, theirs) {
				t.Errorf("parsers disagree\n  ours:   %+v\n  stdlib: %+v",
					ours, theirs)
			}
		})
	}
}

// Unknown fields must be refused, matching DisallowUnknownFields.
//
// A misspelled key on a money API has to fail loudly: "amonut" silently
// ignored means a transfer of zero, which is a bug report nobody can explain.
func TestParsersRejectUnknownFields(t *testing.T) {
	cases := []struct {
		name  string
		body  string
		parse func([]byte) error
	}{
		{"transfer", `{"to":"a","amount":1,"sneaky":"x"}`,
			func(b []byte) error { var v transferRequest; return parseTransferRequest(b, &v) }},
		{"transfer/typo", `{"to":"a","amonut":1}`,
			func(b []byte) error { var v transferRequest; return parseTransferRequest(b, &v) }},
		{"createUser", `{"username":"a","admin":true}`,
			func(b []byte) error { var v createUserRequest; return parseCreateUserRequest(b, &v) }},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if err := tc.parse([]byte(tc.body)); err == nil {
				t.Error("unknown field accepted, want refusal")
			}
		})
	}
}

// Malformed input must fail cleanly rather than panicking or looping.
//
// The board answers these from the torture suite's abuse phase, so they run
// on real hardware with a nearly full heap - a parser that panics here takes
// the whole board down.
func TestParsersRejectMalformed(t *testing.T) {
	bodies := []string{
		``,
		`not json`,
		`{`,
		`}`,
		`{"to"`,
		`{"to":}`,
		`{"to":"a"`,
		`{"to":"a",}`,
		`{"amount":}`,
		`{"amount":"not a number"}`,
		`{"amount":1}{"amount":2}`,
		`[1,2,3]`,
		`{"to":"unterminated`,
		`{"to":"bad escape \q"}`,
		`{"to":"\u00"}`,
		`null`,
		`{"to":` + string(rune(0)) + `}`,
	}
	for _, body := range bodies {
		t.Run(body, func(t *testing.T) {
			var v transferRequest
			// Must not panic. An error is the expected outcome; the test is
			// that we get one rather than a crash.
			if err := parseTransferRequest([]byte(body), &v); err == nil {
				t.Errorf("malformed body %q accepted", body)
			}
		})
	}
}

// Deep nesting must be refused rather than driving the parser into unbounded
// recursion.
func TestParserRefusesDeepNesting(t *testing.T) {
	body := []byte(`{"to":` + repeat(`{"a":`, 64) + `1` + repeat(`}`, 64) + `}`)
	var v transferRequest
	if err := parseTransferRequest(body, &v); err == nil {
		t.Error("deeply nested body accepted")
	}
}

func repeat(s string, n int) string {
	out := make([]byte, 0, len(s)*n)
	for i := 0; i < n; i++ {
		out = append(out, s...)
	}
	return string(out)
}

// What parsing costs, against the decoder it replaced.
var parseSink transferRequest

func BenchmarkParseTransfer(b *testing.B) {
	body := []byte(`{"to":"account-grHEnttNRZQA","amount":5,"memo":"for mowing the lawn"}`)

	b.Run("handwritten", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			parseSink = transferRequest{}
			_ = parseTransferRequest(body, &parseSink)
		}
	})
	b.Run("encoding-json", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			parseSink = transferRequest{}
			_ = json.Unmarshal(body, &parseSink)
		}
	})
}
