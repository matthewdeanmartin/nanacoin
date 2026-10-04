package api

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

// Unicode, emoji and adversarial text, through every path.
//
// # Why this is its own file
//
// The hand-written codecs replaced a library that had been correct about text
// for fifteen years. The obvious cases - ASCII memos, plain field names -
// were covered by the first round of tests and pass trivially. The cases that
// actually break a hand-written escaper are the ones nobody types on purpose:
// emoji outside the BMP, combining marks, surrogate pairs in \u escapes, lone
// surrogates, and text whose bytes happen to look like JSON syntax.
//
// A household will type emoji into a memo on day one. Getting this wrong
// means a corrupt response the board cannot explain, so every case here is
// checked against encoding/json in both directions.

// textCases is shared by the encode, decode and byte-path tests so all three
// see the same inputs. Adding a case here exercises it everywhere.
var textCases = []struct {
	name string
	text string
}{
	{"empty", ""},
	{"ascii", "for mowing the lawn"},
	{"leading and trailing spaces", "  padded  "},

	// Emoji, which is the case a household hits immediately.
	{"emoji basic", "nice work 🎉"},
	{"emoji multiple", "🎉🎂🍰 party"},
	{"emoji only", "🎉"},
	{"emoji skin tone", "thumbs up 👍🏽"},
	{"emoji zwj family", "family 👨‍👩‍👧‍👦"},
	{"emoji flag", "flag 🇬🇧"},
	{"emoji keycap", "keycap 1️⃣"},

	// Scripts beyond Latin.
	{"cjk", "日本語のメモ"},
	{"korean", "한국어"},
	{"arabic rtl", "مرحبا بالعالم"},
	{"hebrew rtl", "שלום"},
	{"cyrillic", "Привет"},
	{"greek", "Ελληνικά"},
	{"thai", "สวัสดี"},
	{"devanagari", "नमस्ते"},

	// Combining marks and normalisation hazards.
	{"combining acute", "café"},
	{"precomposed acute", "café"},
	{"many combining", "à́̂̃"},

	// Characters that look like JSON syntax.
	{"quotes", `say "hello"`},
	{"backslash", `path\to\thing`},
	{"braces", `{"nested":"looking"}`},
	{"brackets", `[1,2,3]`},
	{"colon comma", `a:b,c:d`},
	{"all json punctuation", `{}[]",:\\`},

	// Control characters, which must become escapes.
	{"newline", "line one\nline two"},
	{"tab", "col\tcol"},
	{"carriage return", "cr\rhere"},
	{"nul", "before\x00after"},
	{"bell", "bell\x07here"},
	{"vertical tab", "vt\x0bhere"},
	{"form feed", "ff\x0chere"},
	{"escape char", "esc\x1bhere"},
	{"every control", "\x01\x02\x03\x04\x05\x06\x07\x08\x0b\x0c\x0e\x0f\x1e\x1f"},

	// Boundary-ish lengths, to catch buffer-edge bugs in the writer's
	// flush logic.
	{"exactly 191 bytes", strings.Repeat("a", 191)},
	{"exactly 192 bytes", strings.Repeat("a", 192)},
	{"exactly 193 bytes", strings.Repeat("a", 193)},
	{"longer than the buffer", strings.Repeat("b", 500)},
	{"long with emoji at the end", strings.Repeat("c", 300) + "🎉"},
	{"long with emoji at the start", "🎉" + strings.Repeat("d", 300)},
	{"emoji spanning the buffer edge", strings.Repeat("e", 190) + "🎉" + strings.Repeat("f", 10)},
	{"escapes spanning the buffer edge", strings.Repeat("g", 190) + `"\"` + strings.Repeat("h", 10)},
}

// Encoding must match encoding/json byte for byte, for every case.
func TestEncodeTextMatchesStdlib(t *testing.T) {
	for _, tc := range textCases {
		t.Run(tc.name, func(t *testing.T) {
			want, err := json.Marshal(tc.text)
			if err != nil {
				t.Fatalf("stdlib marshal: %v", err)
			}

			var got bytes.Buffer
			var mem [jsonBufSize]byte
			j := newJSONW(&got, mem[:])
			j.str(tc.text)
			if err := j.done(); err != nil {
				t.Fatalf("encode: %v", err)
			}

			if got.String() != string(want) {
				t.Errorf("encoders disagree\n  stdlib: %s\n  ours:   %s",
					want, got.String())
			}
		})
	}
}

// The byte-taking writer must agree with the string-taking one.
//
// strBytes is what the render path uses, so a divergence here would mean
// list responses escaping differently from single-object ones - the kind of
// bug that shows up as one corrupt row in a table.
func TestEncodeTextBytesMatchesString(t *testing.T) {
	for _, tc := range textCases {
		t.Run(tc.name, func(t *testing.T) {
			var viaString, viaBytes bytes.Buffer

			var m1 [jsonBufSize]byte
			j1 := newJSONW(&viaString, m1[:])
			j1.str(tc.text)
			if err := j1.done(); err != nil {
				t.Fatalf("string encode: %v", err)
			}

			var m2 [jsonBufSize]byte
			j2 := newJSONW(&viaBytes, m2[:])
			j2.strBytes([]byte(tc.text))
			if err := j2.done(); err != nil {
				t.Fatalf("bytes encode: %v", err)
			}

			if viaString.String() != viaBytes.String() {
				t.Errorf("str and strBytes disagree\n  str:      %s\n  strBytes: %s",
					viaString.String(), viaBytes.String())
			}
		})
	}
}

// Every case must survive a full round trip through our encoder and our
// parser, arriving unchanged.
func TestTextRoundTripsThroughOurCodecs(t *testing.T) {
	for _, tc := range textCases {
		t.Run(tc.name, func(t *testing.T) {
			// Encode as a transfer body, parse it back.
			var body bytes.Buffer
			var mem [jsonBufSize]byte
			j := newJSONW(&body, mem[:])
			j.objOpen()
			j.fStr("to", "account-a")
			j.fInt64("amount", 1)
			j.fStr("memo", tc.text)
			j.objClose()
			if err := j.done(); err != nil {
				t.Fatalf("encode: %v", err)
			}

			var got transferRequest
			if err := parseTransferRequest(body.Bytes(), &got); err != nil {
				t.Fatalf("parse %s: %v", body.String(), err)
			}
			if got.Memo != tc.text {
				t.Errorf("round trip changed the text\n  sent: %q\n  got:  %q",
					tc.text, got.Memo)
			}
		})
	}
}

// Our parser must accept what encoding/json produces, which is the format a
// browser actually sends.
func TestParseStdlibEncodedText(t *testing.T) {
	for _, tc := range textCases {
		t.Run(tc.name, func(t *testing.T) {
			body, err := json.Marshal(transferRequest{
				To: "account-a", Amount: 1, Memo: tc.text,
			})
			if err != nil {
				t.Fatalf("stdlib marshal: %v", err)
			}

			var got transferRequest
			if err := parseTransferRequest(body, &got); err != nil {
				t.Fatalf("parse %s: %v", body, err)
			}
			if got.Memo != tc.text {
				t.Errorf("parsing stdlib output changed the text\n  want: %q\n  got:  %q",
					tc.text, got.Memo)
			}
		})
	}
}

// \u escapes, including the surrogate pairs a browser uses for emoji.
//
// A browser sending an emoji in a JSON body may send the raw UTF-8 bytes or a
// surrogate pair like 🎉. Both have to work, and the pair is the
// one a hand-written parser gets wrong.
func TestParseUnicodeEscapes(t *testing.T) {
	// The escape sequences are built with an explicit backslash constant
	// rather than written literally.
	//
	// Not stylistic paranoia: the first version of this test had its escapes
	// decoded before they reached the file, so `BS+"u0022"` arrived as a bare
	// quote and the case was silently checking nothing. Constructing them at
	// runtime makes that class of mistake impossible.
	const bs = `\`

	esc := func(hex string) string { return bs + "u" + hex }

	cases := []struct {
		name string
		body string
		want string
	}{
		{"bmp escape", `{"to":"a","amount":1,"memo":"caf` + esc("00e9") + `"}`, "caf\u00e9"},
		{"cjk escape", `{"to":"a","amount":1,"memo":"` + esc("65e5") + esc("672c") + `"}`, "\u65e5\u672c"},

		// These decode to JSON-significant bytes, which is exactly where a
		// hand-written parser goes wrong: a decoded quote must not be taken
		// for the string terminator, and a decoded backslash must not start a
		// new escape.
		{"escaped quote", `{"to":"a","amount":1,"memo":"` + esc("0022") + `quoted` + esc("0022") + `"}`, `"quoted"`},
		{"escaped backslash", `{"to":"a","amount":1,"memo":"` + esc("005c") + `"}`, bs},
		{"escaped newline", `{"to":"a","amount":1,"memo":"a` + esc("000a") + `b"}`, "a\nb"},
		{"escaped tab", `{"to":"a","amount":1,"memo":"a` + esc("0009") + `b"}`, "a\tb"},
		{"escaped nul", `{"to":"a","amount":1,"memo":"a` + esc("0000") + `b"}`, "a\x00b"},

		{"mixed raw and escaped", `{"to":"a","amount":1,"memo":"caf` + esc("00e9") + ` \u65e5\u672c 🎉"}`, "caf\u00e9 \u65e5\u672c 🎉"},
		{"escape at string end", `{"to":"a","amount":1,"memo":"end` + esc("00e9") + `"}`, "end\u00e9"},
		{"only escapes", `{"to":"a","amount":1,"memo":"` + esc("0041") + esc("0042") + esc("0043") + `"}`, "ABC"},
		{"escape then emoji", `{"to":"a","amount":1,"memo":"` + esc("00e9") + `🎉"}`, "\u00e9🎉"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			var got transferRequest
			if err := parseTransferRequest([]byte(tc.body), &got); err != nil {
				t.Fatalf("parse %s: %v", tc.body, err)
			}
			if got.Memo != tc.want {
				t.Errorf("want %q, got %q", tc.want, got.Memo)
			}

			// And stdlib must agree, so this is not just our own opinion
			// about what the escape means.
			var std transferRequest
			if err := json.Unmarshal([]byte(tc.body), &std); err != nil {
				t.Fatalf("stdlib parse %s: %v", tc.body, err)
			}
			if std.Memo != got.Memo {
				t.Errorf("we disagree with stdlib\n  ours:   %q\n  stdlib: %q",
					got.Memo, std.Memo)
			}
		})
	}
}

// Surrogate pairs deserve their own test, because they are the one place our
// parser knowingly differs.
//
// encoding/json combines a 🎉 pair into U+1F389 🎉. Our parser
// decodes each half separately, so a lone or paired surrogate becomes U+FFFD.
// That is a deliberate limitation - combining pairs means carrying state
// across escapes - and it is recorded here rather than left to be discovered.
//
// It is safe in practice because a browser sending JSON from fetch() emits
// raw UTF-8 for emoji, not escapes; the pair form comes from servers that
// ASCII-escape their output, which nothing in this system does.
func TestSurrogatePairsAreAKnownLimitation(t *testing.T) {
	body := `{"to":"a","amount":1,"memo":"🎉"}`

	var std transferRequest
	if err := json.Unmarshal([]byte(body), &std); err != nil {
		t.Fatalf("stdlib parse: %v", err)
	}

	var got transferRequest
	if err := parseTransferRequest([]byte(body), &got); err != nil {
		t.Fatalf("parse: %v", err)
	}

	t.Logf("surrogate pair \\ud83c\\udf89:\n  stdlib: %q\n  ours:   %q",
		std.Memo, got.Memo)

	if std.Memo != "🎉" {
		t.Errorf("stdlib no longer combines surrogate pairs: %q", std.Memo)
	}
	// Ours must at least not corrupt the rest of the document or crash.
	if got.To != "a" || got.Amount != 1 {
		t.Errorf("a surrogate pair broke the surrounding fields: %+v", got)
	}
	// Raw UTF-8, which is what a browser actually sends, must be exact.
	var raw transferRequest
	if err := parseTransferRequest([]byte(`{"to":"a","amount":1,"memo":"🎉"}`), &raw); err != nil {
		t.Fatalf("parse raw: %v", err)
	}
	if raw.Memo != "🎉" {
		t.Errorf("raw UTF-8 emoji must be exact, got %q", raw.Memo)
	}
}

// Invalid UTF-8 must not crash or truncate the rest of the document.
//
// The board should never produce this, but a hostile client can send it, and
// the torture suite does.
func TestInvalidUTF8DoesNotBreakParsing(t *testing.T) {
	bodies := [][]byte{
		append([]byte(`{"to":"a","amount":1,"memo":"`), append([]byte{0xFF, 0xFE}, []byte(`"}`)...)...),
		append([]byte(`{"to":"a","amount":1,"memo":"`), append([]byte{0xC3}, []byte(`"}`)...)...),
		append([]byte(`{"to":"a","amount":1,"memo":"`), append([]byte{0xED, 0xA0, 0x80}, []byte(`"}`)...)...),
	}
	for i, body := range bodies {
		var got transferRequest
		err := parseTransferRequest(body, &got)
		// Either outcome is acceptable; a panic or a hang is not.
		t.Logf("case %d: err=%v to=%q amount=%d", i, err, got.To, got.Amount)
		if err == nil && got.To != "a" {
			t.Errorf("case %d: accepted but mangled the other fields: %+v", i, got)
		}
	}
}

// Text that fills or overflows the encoder's buffer must still come out
// correct, since the flush path is where an off-by-one would hide.
func TestLongTextFlushesCorrectly(t *testing.T) {
	for _, n := range []int{0, 1, 191, 192, 193, 383, 384, 385, 1000, 4000} {
		t.Run(strings.Repeat("x", 0)+itoa(n), func(t *testing.T) {
			text := strings.Repeat("x", n)
			want, _ := json.Marshal(text)

			var got bytes.Buffer
			var mem [jsonBufSize]byte
			j := newJSONW(&got, mem[:])
			j.str(text)
			if err := j.done(); err != nil {
				t.Fatalf("encode: %v", err)
			}
			if got.String() != string(want) {
				t.Errorf("length %d encoded wrong\n  want %d bytes\n  got  %d bytes",
					n, len(want), got.Len())
			}
		})
	}
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var b [20]byte
	i := len(b)
	for n > 0 {
		i--
		b[i] = byte('0' + n%10)
		n /= 10
	}
	return string(b[i:])
}
