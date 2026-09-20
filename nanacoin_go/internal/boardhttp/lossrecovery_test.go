package boardhttp

import (
	"os"
	"strings"
	"testing"

	"github.com/soypat/lneto/tcp"
)

// Loss recovery must be enabled on every pooled connection.
//
// # Why this matters
//
// xnet.NewTCPPool in upstream lneto v0.3.2 builds each tcp.Conn without
// setting ConnConfig.LossRecovery, and tcp.ConnConfig documents that leaving
// that field nil disables loss recovery entirely. The consequence on a weak
// link is that a dropped outgoing segment is never retransmitted on a timer:
// the connection waits for an ACK that cannot arrive.
//
// That is the likeliest explanation for why this board struggles from a shelf
// where MicroPython is fine. lwIP has had an RTO since forever; this pool has
// had one available but switched off. The algorithm ships in the same module
// - tcp.RTO, RFC 6298, with its own tests - it is simply never wired up.
//
// patches/apply.ps1 wires it. This test checks the patch is present, because
// building against unpatched lneto produces a board that silently lacks the
// property, and silence is the whole problem with this class of bug.
//
// # Why it inspects source rather than behaviour
//
// The obvious test - configure a pool, ask a connection whether it has loss
// recovery - cannot be written from outside the tcp package. Handler.loss is
// unexported, and NextDeadline returns 0 both for "no loss recovery" and for
// "timer not currently armed", so it cannot tell the two apart on an idle
// connection. Driving a real connection to the point where the difference
// shows means a full TCP exchange with induced packet loss, which is an
// integration test against a network, not a unit test.
//
// So this checks the thing that is actually checkable: that the vendored
// pool contains the wiring. Crude, and honest about being crude.
func TestPooledConnectionsHaveLossRecovery(t *testing.T) {
	const poolPath = "../../third_party/lneto/x/xnet/tcppool.go"

	src, err := os.ReadFile(poolPath)
	if err != nil {
		t.Skipf("vendored lneto not present (%v) - run patches/apply.ps1 "+
			"before building for the board", err)
	}

	got := string(src)
	for _, want := range []string{
		"LossRecovery:",
		"Nanotime:",
		"tcp.RTO",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("vendored tcppool.go does not mention %q - the loss "+
				"recovery patch is missing. Delete third_party/lneto and "+
				"re-run patches/apply.ps1, or every lost segment will stall "+
				"a connection instead of being retransmitted.", want)
		}
	}
}

// RTO must remain usable as a LossRecovery, and the fields the patch sets must
// still exist with the types it assigns. A compile-time assertion, so an
// lneto upgrade that reshaped any of this fails here - at `go test` on a
// desktop - rather than silently producing a board that has lost the
// property.
var (
	_ tcp.LossRecovery = (*tcp.RTO)(nil)
	_                  = tcp.ConnConfig{
		LossRecovery: (*tcp.RTO)(nil),
		Nanotime:     func() int64 { return 0 },
	}
)
