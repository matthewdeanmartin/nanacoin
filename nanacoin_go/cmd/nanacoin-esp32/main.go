//go:build tinygo

// Command nanacoin-esp32 is NanaCoin running on the board.
//
// The economic logic is not duplicated here. This file brings up WiFi, chooses
// a storage backend, and serves internal/api's handler - the same handler the
// desktop build serves - over espradio's allocation-free HTTP server.
//
// It does not use net/http. An earlier version did, and that board answered
// about twenty requests before its listener stopped accepting connections;
// see internal/boardhttp for the measurement and the reason.
//
// WiFi credentials are compiled in:
//
//	tinygo flash -target=esp32s3-generic \
//	  -ldflags="-X main.ssid=YourSSID -X main.password=YourPassword" \
//	  ./cmd/nanacoin-esp32
package main

import (
	"io"
	"net/netip"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/soypat/lneto"
	"github.com/soypat/lneto/http/httphi"
	"github.com/soypat/lneto/tcp"
	"github.com/soypat/lneto/x/xnet"
	"tinygo.org/x/espradio"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/api"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/auth"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/boardhttp"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/boardmdns"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/storage/memory"
)

// Set at link time with -ldflags "-X main.ssid=... -X main.password=...".
var (
	ssid     string
	password string
)

// Keep DHCP and mDNS names consistent, and distinct from the MicroPython
// static web board's nanacoin.local.
const hostname = boardmdns.Hostname

const (
	listenPort = 80

	// pollTime is how long the stack loop sleeps when there was nothing to
	// send or receive. Short enough to keep latency low, long enough that an
	// idle board is not spinning.
	pollTime = 5 * time.Millisecond

	// maxConns is the router's worker count, and must equal
	// boardhttp.Workers - see the comment there. It was 4 against 2 buffer
	// sets, which meant two goroutines existed only to block on a channel
	// while holding a stack the conservative collector had to scan.
	maxConns = boardhttp.Workers

	// poolConns is the TCP pool size, larger than maxConns because a
	// connection holds its slot after its request is answered, until the peer
	// closes it and closingTimeout expires.
	//
	// 5 is measured, and the measurement that set it is worth keeping.
	//
	// At 8 slots the pool held 24 KB - more than twice the ~10 KB the board
	// has free after setup - and a single browsing user, whose page issues
	// five requests at once, drove the heap to 672 bytes and left the board
	// unresponsive until it was physically reset. It did not recover on its
	// own after twenty minutes.
	//
	// The earlier note said 8 "serves hundreds of requests before a burst
	// outruns reclamation, and it recovers by itself". That held for
	// sequential traffic; it does not survive the concurrent burst a single
	// page load produces, which is the traffic this board actually gets.
	//
	// 5 covers one page's five parallel reads with nothing spare, which is
	// the honest ceiling: beyond it the listener refuses with an RST, and a
	// refusal the client can see beats a timeout it cannot explain.
	poolConns = 5

	// requestHeaderBuffer holds one request's header fields. A browser sends
	// around twenty, one of them a bearer token, so 1 kB is comfortable.
	requestHeaderBuffer = 1 << 10

	// responseHeaderBuffer holds the staged response fields, and is set
	// explicitly rather than through DefaultRouterConfig because that helper
	// caps it at 128 bytes - far too small here, and it fails by silently
	// dropping whichever field did not fit.
	//
	// The measured worst case is about 441 bytes of field text before
	// httphi's own per-field framing:
	//
	//	Access-Control-Allow-Origin: *                           ~32
	//	Access-Control-Allow-Methods: GET, POST, PATCH, OPTIONS   ~52
	//	Access-Control-Allow-Headers: Authorization, ...          ~74
	//	Access-Control-Expose-Headers: X-Nanacoin-Health          ~46
	//	Access-Control-Max-Age, Content-Type, Cache-Control       ~97
	//	Connection: close                                         ~19
	//	X-Nanacoin-Health (fixed width now)                      ~121
	//
	// 512 was marginal against that, and the health header was variable
	// width - so the buffer was crossed exactly when the heap numbers grew
	// an extra digit. That is why dropped headers looked like a symptom of
	// low memory rather than a fixed buffer being slightly too small, and
	// why the health reading vanished from the responses where it mattered
	// most. healthLine is fixed-width now, so this no longer varies.
	responseHeaderBuffer = 1 << 10

	// establishedTimeout bounds how long a connection may sit acquired but
	// not yet established - a half-open handshake. It is not an idle timeout.
	establishedTimeout = 5 * time.Second

	// closingTimeout bounds how long a closing connection occupies its pool
	// slot before being reclaimed, which makes it the reclaim rate.
	//
	// Measured, because both directions fail: at 2s a burst of short-lived
	// connections outran reclamation and the board stopped accepting after
	// ~475 requests until it drained. At 250ms it stopped accepting
	// immediately - shorter than the round trip a closing handshake needs, so
	// slots were being torn down under connections still finishing. 1s is
	// comfortably above a LAN round trip and four times the reclaim rate that
	// failed; the larger pool is what absorbs the burst now.
	closingTimeout = time.Second

	// connDeadline fails a peer that connects and then stalls mid-request,
	// rather than letting it hold a router goroutine indefinitely.
	connDeadline = 10 * time.Second
)

// defaultOrigins is empty, which the API layer reads as "answer every origin
// with Access-Control-Allow-Origin: *".
//
// It used to be an exact list, and maintaining that list was a recurring
// source of a failure that looks nothing like what it is: a browser not on
// the list gets a 200 with no ACAO header and reports the board as
// unreachable. See the reasoning on api.Server.cors for why a wildcard is the
// right call for a household pie-promise tracker, and note that it depends on
// provisioning being closed once the household exists - which serve() does.
//
// An explicit list can still be set at link time through main.origins.
var defaultOrigins []string

// origins can be set at link time to replace the list above, so that adding a
// deployment does not mean editing this file:
//
//	-ldflags="-X main.origins=https://nanacoin.example.net,http://localhost:4200"
var origins string

// allowedOrigins is the effective list. Origins are matched exactly and echoed
// back; the request origin is never reflected unchecked.
func allowedOrigins() []string {
	if origins == "" {
		return defaultOrigins
	}
	var out []string
	for _, o := range strings.Split(origins, ",") {
		if o = strings.TrimSpace(o); o != "" {
			out = append(out, o)
		}
	}
	return out
}

func main() {
	// First, before anything else allocates or logs: read the note the
	// previous run left about how it died. See blackbox.go - the contents
	// live in SRAM that survives a reset but not a power cycle, and anything
	// that runs before this could overwrite them.
	recoverBlackBox()

	// The USB serial console takes a moment to attach; without this the first
	// few messages go nowhere and a boot problem looks like a dead board.
	time.Sleep(2 * time.Second)

	println("NanaCoin starting")
	bootReport()
	authBootCheck()

	// TODO: swap this for a flash-backed journal once the ESP32 partition API
	// is wired up. The interface is the whole point of the storage package -
	// only this line changes, and nothing in ledger, core or api knows the
	// difference.
	//
	// Until then the board forgets everything on reboot, which is fine for
	// trying it on the desk and useless for real money.
	//
	// Discarding, not retaining. The service rebuilds full state in RAM as
	// each record is applied, so keeping the encoded record as well stored
	// every mutation twice - and the second copy had no possible reader,
	// because a Replay only happens at boot and RAM does not survive one.
	// That made every write a permanent leak, which is most of why the board
	// died after a handful of users and listings.
	journal := memory.NewDiscarding()

	svc, err := core.New(journal, core.Options{})
	if err != nil {
		fail("opening ledger: " + err.Error())
	}

	stack, addr := connectWiFi()

	// Reserve auth slots once, but budget them alongside the four workers.
	// Full package ceilings cost 9,088 bytes and caused a measured startup-
	// baseline of ~4 KiB, followed by OOM on the fifth status connection.
	// 32 sessions cover two devices per maximum 16-person household.
	sessions := auth.NewStore(auth.Options{
		SessionCapacity: 32, CodeCapacity: 8, FailureCapacity: 16,
		// Sessions are RAM-only and die with the board. A shorter lifetime
		// than the desktop default, because a household tablet left logged in
		// is the likelier risk here than the inconvenience of logging in
		// again.
		TokenTTL: 4 * time.Hour,
	})

	srv := api.NewServer(svc, sessions, api.Config{
		AllowedOrigins: allowedOrigins(),
		AllowProvision: true,
		// Cap list responses hard. Rendering fifty transactions here caused
		// "fatal error: out of memory": the response buffer grows by
		// doubling, so producing N bytes transiently holds 2N. Fifteen
		// transactions is a useful page on a phone and one the board can
		// actually build.
		MaxPageSize: boardhttp.MaxPageSize,
	})

	// Serve the heap through /logs, so it can be read from a browser rather
	// than only over USB. The board's memory was the crucial number while
	// diagnosing the out-of-memory and it was the one thing the logs page
	// could not show.
	srv.SetHealth(healthLine)

	// Report the previous run's fate through the API as well as the console.
	// This is the one that matters: the board lives downstairs by the router
	// with no cable attached, so a crash record that can only be read over
	// USB is a crash record nobody reads.
	srv.SetDiagnostics(boardDiagnostics)
	srv.SetMachineInfo(boardMachineInfo)

	// The same handler the desktop serves, adapted onto httphi. The origin
	// list is passed separately so the adapter can answer requests the
	// handler never sees - see boardhttp.stageCORSForRejected.
	board := boardhttp.New(srv.Handler(), allowedOrigins())
	board.Observe(boardObserver{})

	var mux httphi.MuxSlice
	board.Register(&mux)

	// Configure allocates every buffer and goroutine the router will ever
	// use. After this point serving a request costs nothing, which is the
	// whole reason for not using net/http here.
	var router httphi.Router
	cfg := httphi.RouterConfig{
		FixedNumGoroutines:          maxConns,
		RequestHeaderBufferSize:     requestHeaderBuffer,
		ResponseHeaderMinBufferSize: responseHeaderBuffer,
		// One slot per 32 bytes of request buffer, matching what
		// DefaultRouterConfig would have derived. The path values the mux
		// needs are taken from the mux itself by Configure.
		RequestNumHeaderKVCap: requestHeaderBuffer / 32,
	}
	if err := router.Configure(&mux, cfg); err != nil {
		fail("configuring router: " + err.Error())
	}
	defer router.Shutdown()

	status := svc.Status()
	println("household:", status.Household)
	println("users:", status.Users, "transactions:", status.Transactions)
	if !status.Provisioned {
		println("not provisioned - POST /api/v1/provision to create the first Nana")
	}

	// State the CORS policy plainly at boot. A browser the policy excludes
	// gets a 200 with no Access-Control-Allow-Origin and reports the board as
	// unreachable, which looks nothing like a configuration problem from the
	// outside.
	if o := allowedOrigins(); len(o) == 0 {
		println("browser origins: any (Access-Control-Allow-Origin: *)")
	} else {
		println("allowed browser origins:")
		for _, origin := range o {
			println("  " + origin)
		}
	}

	// Watch the link. Nothing else notices a lost association: espradio
	// raises the event and clears its own flag, but the flag is not exported
	// and no one consumes the event - so a board that drops off the network
	// stays up answering nothing until it is power-cycled.
	go watchLink()

	reportRadio()
	println("listening on http://" + addr.String())

	serve(stack, &router)
}

// serve accepts connections forever and hands each to the router.
//
// Accepting is this program's job; httphi only serves. A connection the router
// cannot take - every worker busy and the queue full - is closed rather than
// queued, which is the backpressure that keeps memory bounded under load.
func serve(stack *espradio.Stack, router *httphi.Router) {
	pool, err := xnet.NewTCPPool(xnet.TCPPoolConfig{
		PoolSize:  poolConns,
		QueueSize: 3,
		// Sized for this API's traffic: requests are a few hundred bytes of
		// JSON, responses up to a few kB for Nana's ledger.
		// Two streaming chunks per connection. Four workers plus the fixed
		// ledger exhausted RAM with 4 KiB per connection; 2 KiB saves 16 KiB
		// across the eight slots without reducing connection concurrency.
		TxBufSize:          2 << 10,
		RxBufSize:          1 << 10,
		EstablishedTimeout: establishedTimeout,
		ClosingTimeout:     closingTimeout,
		NewBackoff:         func() lneto.BackoffStrategy { return pollBackoff },
	})
	if err != nil {
		fail("creating tcp pool: " + err.Error())
	}

	// Take the heap baseline here, not at the end of main.
	//
	// It used to be taken before this pool existed, which left roughly 45 kB
	// of startup allocation - poolConns x (TxBufSize + RxBufSize) is 24 kB on
	// its own, plus the connection objects - outside the baseline and
	// therefore counted as if serving had caused it. That is what made the
	// first request look like it cost 45 kB and sent a long investigation
	// after a per-request leak that was really late startup.
	//
	// Every later delta is now against a board that is genuinely finished
	// setting up.
	// Work out the allocator's block size before anything depends on it.
	calibrateBlockSize()

	{
		var m runtime.MemStats
		runtime.ReadMemStats(&m)
		baselineInuse = m.HeapInuse
	}
	println("heap block size:", int(blockSize), "bytes")
	reportMemory("after setup")

	var listener tcp.Listener
	if err := listener.Reset(listenPort, pool); err != nil {
		fail("resetting listener: " + err.Error())
	}
	if err := stack.LnetoStack().RegisterListenerTCP(&listener); err != nil {
		fail("registering listener: " + err.Error())
	}

	var accepted, refused int
	for {
		// Reap closed and half-open connections every pass, not only when
		// idle. Under a steady stream of requests the idle branch below is
		// never reached, so reaping there would let the pool fill up and the
		// board would stop accepting while still on the network - which is
		// precisely how the net/http version failed.
		pool.CheckTimeouts()

		if listener.NumberOfReadyToAccept() == 0 {
			note(PhaseIdle, RouteUnknown)
			time.Sleep(pollTime)
			continue
		}

		note(PhaseAccept, RouteUnknown)
		conn, _, err := listener.TryAccept()
		if err != nil {
			println("accept failed:", err.Error())
			time.Sleep(time.Second)
			continue
		}

		conn.SetDeadline(time.Now().Add(connDeadline))
		if err := router.Handle(conn); err != nil {
			// Every router worker is busy and its queue is full.
			//
			// This used to just close the connection, which from the client
			// looks exactly like a crash: "remote end closed connection" is
			// what a dead board produces too. Measured under chaotic load,
			// 42 of 44 apparent failures were this - the board working
			// correctly and being reported as broken.
			//
			// So say it in HTTP. A 503 with Retry-After is a refusal a
			// client can act on, and it is the difference between "the board
			// is at capacity" and "the board is gone" in every log that
			// follows.
			refused++
			writeBusyResponse(conn)
			conn.Close()
		}

		// An accepted-connection count on the console is the only way to
		// tell "still working" from "silently stopped accepting" without a
		// debugger. The previous net/http build went quiet at request 20 with
		// no indication, which cost a long time to pin down.
		//
		// Connections, not requests: a keep-alive client serves many requests
		// down one connection, so this number is far lower than the request
		// count and must not be read as one.
		accepted++
		noteServed(uint32(accepted))

		// Collect between connections.
		//
		// This was removed once, on the reasoning that TinyGo's collector is
		// non-moving so running it more often cannot help fragmentation. That
		// reasoning was right and the conclusion was wrong: it confused "cannot
		// compact" with "is not needed".
		//
		// Measured on the board with it removed, serving nothing but /status
		// against an empty ledger:
		//
		//	inuse 199024 -> 231232 over ten requests  (+2176 B each)
		//	objects  369 -> 1055                      (+46 each)
		//	gc      0000                              (never ran)
		//
		// The collector had not run once while the heap climbed toward
		// exhaustion, so ~24 more requests would have finished it - which is
		// exactly how a burst of reads took the board off the network.
		//
		// Why it does not run on its own: the allocator collects when it cannot
		// satisfy an allocation from the free list, and with ~50 kB still free
		// every small request is satisfied immediately. On a desktop that is
		// efficient. Here it means the heap is consumed before the collector is
		// ever consulted.
		//
		// Between connections is the cheapest possible moment: no request is in
		// flight, so the pause costs nobody any latency.
		runtime.GC()

		// Every connection, not every tenth. The failure being hunted
		// happens within a handful of requests, so sampling coarsely is how
		// it gets missed - and one line per connection is affordable when a
		// household makes a few dozen requests a day.
		// Keep the reading where a later request can fetch it. This is the
		// only way to learn what the heap looked like just before a stall:
		// the board cannot report its own death, but the sample from the
		// connection before it survives in RAM and /diag serves it.
		recordHeap(readHeap())

		reportMemory("conn " + itoa(accepted) + " refused " + itoa(refused))
		if accepted%10 == 0 {
			reportRadio()
		}
	}
}

var pollBackoff = lneto.BackoffStrategy(func(_ uint) time.Duration {
	return pollTime
})

// busyResponse is the refusal sent when no router worker can take a
// connection.
//
// Written as a constant rather than built per refusal: this path runs when
// the board is already at capacity, which is the worst possible moment to
// allocate. The CORS header is a wildcard to match the rest of the API, so a
// browser can actually read the status rather than reporting a CORS error.
const busyResponse = "HTTP/1.1 503 Service Unavailable\r\n" +
	"Content-Type: application/json; charset=utf-8\r\n" +
	"Access-Control-Allow-Origin: *\r\n" +
	"Retry-After: 1\r\n" +
	"Connection: close\r\n" +
	"Content-Length: 75\r\n" +
	"\r\n" +
	`{"error":"busy","message":"the board is at capacity; please retry shortly"}`

// writeBusyResponse answers a connection the router could not take.
//
// Best effort: the peer may already be gone, and there is nothing useful to
// do if the write fails. What matters is that a client which *is* listening
// gets a status instead of a closed socket.
// Takes the narrow interface rather than net.Conn: lneto's *tcp.Conn has
// SetWriteDeadline and Write but not LocalAddr, so it is not a net.Conn.
// Naming only what is used keeps this working whatever the stack provides.
func writeBusyResponse(conn interface {
	Write([]byte) (int, error)
	SetWriteDeadline(time.Time) error
}) {
	_ = conn.SetWriteDeadline(time.Now().Add(time.Second))
	_, _ = io.WriteString(conn, busyResponse)
}

// connectWiFi brings up the radio and joins the household network, returning
// the stack and the address DHCP assigned.
//
// The bring-up is done a step at a time rather than through netlink's
// NetConnect, because this build needs the lneto stack directly to register a
// TCP listener - netlink keeps it behind an unexported field.
//
// Two steps fail intermittently at this range and are retried: the WPA2
// four-way handshake, and DHCP. Neither failure means the passphrase is wrong
// or the network is absent, so neither is fatal. The association retry is
// unlimited, because the access point drops out of range for minutes at a time
// and a board that gave up would need a power cycle to come back.
func connectWiFi() (*espradio.Stack, netip.Addr) {
	if err := espradio.Enable(espradio.Config{Logging: espradio.LogLevelError}); err != nil {
		fail("enabling radio: " + err.Error())
	}
	if err := espradio.Start(); err != nil {
		fail("starting radio: " + err.Error())
	}

	associate()

	nd, err := espradio.StartNetDev()
	if err != nil {
		fail("starting netdev: " + err.Error())
	}

	stack, err := espradio.NewStack(nd, espradio.StackConfig{
		Hostname:     hostname,
		MaxUDPPorts:  2,
		MaxTCPPorts:  1,
		PassivePeers: 64,
	})
	if err != nil {
		fail("building network stack: " + err.Error())
	}

	// The stack has to be pumped before anything uses it, DHCP included.
	go pumpStack(stack)

	// DHCP is the step that fails on a weak link: it is a four-packet
	// exchange, and losing one packet costs the whole attempt.
	var dhcp *xnet.DHCPResults
	for attempt := 1; ; attempt++ {
		dhcp, err = stack.SetupWithDHCP(espradio.DHCPConfig{})
		if err == nil {
			break
		}
		wait := backoff(attempt)
		println("DHCP attempt", attempt, "failed:", err.Error(), "- retrying in", int(wait.Seconds()), "s")
		time.Sleep(wait)
	}

	addr, ok := netip.AddrFromSlice(dhcp.AssignedAddr4[:])
	if !ok {
		fail("DHCP returned an address that is not IPv4")
	}
	println("connected, address", addr.String())
	if err := boardmdns.Register(stack.LnetoStack(), addr, listenPort); err != nil {
		fail("starting mDNS: " + err.Error())
	}
	println("mDNS: http://" + boardmdns.LocalName)
	return stack, addr
}

// associate joins the AP, retrying indefinitely on anything transient.
func associate() {
	for attempt := 1; ; attempt++ {
		err := espradio.Connect(espradio.STAConfig{SSID: ssid, Password: password})
		if err == nil {
			println("associated with", ssid)
			return
		}

		// Only a real credential rejection is fatal. Matching loosely on
		// "auth" would catch WIFI_REASON_AUTH_EXPIRE ("auth expired"), which
		// is a transient WPA2 timing failure the next attempt clears - so the
		// test is against the reasons that actually mean the passphrase is
		// wrong.
		msg := err.Error()
		if strings.Contains(msg, "authentication failed") || strings.Contains(msg, "802.1X") {
			fail("WiFi rejected the passphrase: " + msg)
		}

		wait := backoff(attempt)
		println("association attempt", attempt, "failed:", msg, "- retrying in", int(wait.Seconds()), "s")
		time.Sleep(wait)
	}
}

// itoa is strconv.Itoa without the import: println takes ints, but the
// message they go into is a string.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}

// backoff grows the wait to a 30s ceiling. A distant AP is worth checking
// twice a minute indefinitely; checking every two seconds forever is just
// noise on the console.
func backoff(attempt int) time.Duration {
	wait := time.Duration(attempt) * 2 * time.Second
	if wait > 30*time.Second {
		wait = 30 * time.Second
	}
	return wait
}

// baseline is the heap reading taken once everything is set up, so later
// readings can be reported as a delta rather than an absolute nobody can
// interpret.
var baselineInuse uint64

// reportMemory prints the heap, and the change since setup.
//
// The numbers matter more than they would on a desktop: the adapter's
// buffers, httphi's router memory and the TCP pool are all fixed at startup,
// so a figure that climbs while serving means something is allocating per
// request that should not be. The delta is what makes that visible - an
// absolute "inuse 164000" says nothing on its own.
//
// Instrumenting rather than reasoning, because on this board reasoning has a
// poor record: the actual cause of the out-of-memory was an idempotency cache
// that never shrank, after several plausible theories about JSON and GC
// pressure turned out to be wrong.
func reportMemory(when string) {
	h := readHeap()
	delta := int64(h.Inuse) - int64(baselineInuse)

	// blk is the fragmentation proxy - see heapSnapshot. Watch it against
	// free: free bytes holding steady while blk falls toward 100 means the
	// heap is shattering into single-block objects even though the totals
	// look fine, and that is the state a large allocation fails in.
	println("heap", when+":",
		"free", int(h.Free),
		"inuse", int(h.Inuse),
		"delta", int(delta),
		"obj", int(h.Objects),
		"blk/100", int(h.BlocksPerObject),
		"mallocs", int(h.Mallocs),
		"frees", int(h.Frees),
		"gc", int(h.GCs))
}

// healthBuf backs the health line. Reused across calls rather than allocated
// per request, because the reading is wanted most when memory is short - and
// a diagnostic that needs a dozen allocations disappears exactly then.
//
// That is not hypothetical: the earlier version concatenated five itoa
// results, and the header went missing from the last handful of responses
// before a crash, which is the stretch it existed to describe.
//
// Guarded by healthMu because the router serves several connections at once
// and they would otherwise overwrite each other mid-format.
var (
	healthBuf [160]byte
	healthMu  sync.Mutex
)

// healthLine is the heap, as one line for the logs endpoint and the response
// header. Formats into a fixed buffer and allocates once for the returned
// string.
//
// Every number is zero-padded to a fixed width. That is not cosmetic: the
// line goes into a response header staged into a fixed buffer, so a
// variable-width line made the header grow with the heap figures - and it
// overflowed the staging buffer exactly when the numbers got interesting,
// which is how the reading disappeared from the responses that needed it. A
// constant-width line makes the header cost a constant.
func healthLine() string {
	h := readHeap()

	healthMu.Lock()
	defer healthMu.Unlock()

	b := healthBuf[:0]
	b = append(b, "free "...)
	b = appendPadded(b, int64(h.Free), 6)
	b = append(b, " inuse "...)
	b = appendPadded(b, int64(h.Inuse), 6)
	b = append(b, " obj "...)
	b = appendPadded(b, int64(h.Objects), 5)
	// blk is average blocks per object, x100. Near 100 means the heap is full
	// of single-block objects, which is the shape that fragments a non-moving
	// collector; a larger number means fewer, bigger allocations.
	b = append(b, " blk "...)
	b = appendPadded(b, int64(h.BlocksPerObject), 4)
	b = append(b, " gc "...)
	b = appendPadded(b, int64(h.GCs), 5)
	return string(b)
}

// appendPadded writes n right-aligned in width digits, zero-padded, so the
// formatted length never changes.
func appendPadded(b []byte, n int64, width int) []byte {
	neg := n < 0
	if neg {
		n = -n
		width--
		b = append(b, '-')
	}
	var tmp [20]byte
	i := len(tmp)
	if n == 0 {
		i--
		tmp[i] = '0'
	}
	for n > 0 {
		i--
		tmp[i] = byte('0' + n%10)
		n /= 10
	}
	for pad := width - (len(tmp) - i); pad > 0; pad-- {
		b = append(b, '0')
	}
	return append(b, tmp[i:]...)
}

// heapSnapshot is everything the board can honestly say about its memory.
//
// # What each number means, and what it does not
//
// TinyGo's collector is non-moving and block-based: the heap is an array of
// fixed-size blocks, an object occupies one or more consecutive blocks, and
// nothing is ever relocated. ReadMemStats reports live blocks and live
// objects, which is enough to derive the things that matter here even though
// the runtime exports no fragmentation figure of its own.
//
//   - Free is HeapIdle: bytes not currently held by any object. It is the
//     number that looks reassuring right up until the board dies, because
//     free bytes scattered in small pieces cannot satisfy a large request.
//
//   - Objects is the count of live allocations. Watch it against Free: many
//     small objects holding a little memory is the shape that fragments,
//     while few large ones is not.
//
//   - BlocksPerObject is live blocks divided by live objects - the average
//     allocation size in blocks. A heap full of single-block objects
//     scattered through the address space is the worst case for a non-moving
//     collector, and this is the closest thing to a fragmentation reading
//     that can be computed from what the runtime exports.
//
//   - GCs and the collection rate say whether the collector is keeping up.
//     A rate climbing while Free falls means it is running constantly and
//     still losing.
//
// What none of these give is the largest contiguous run, which is what
// actually decides whether the next allocation succeeds. The collector keeps
// exactly that list (freeRanges in runtime/gc_blocks.go, sorted by length)
// and does not export it. Probing for it by allocating is not an option
// either: TinyGo's allocator calls runtimeFatal on failure, which aborts
// rather than panicking, so a probe would not measure the cliff - it would be
// the fall.
//
// So fragmentation is inferred, never measured, and this struct is built to
// make the inference legible rather than to pretend otherwise.
type heapSnapshot struct {
	Inuse   uint64
	Free    uint64
	Total   uint64
	Objects uint64
	Mallocs uint64
	Frees   uint64
	GCs     uint32

	// BlocksPerObject is scaled by 100 so it can be reported as an integer.
	// 100 means one block per object; 250 means two and a half.
	BlocksPerObject uint32
}

// blockSize is TinyGo's allocation granularity on this target.
//
// Not exported by the runtime, so it is inferred once at boot rather than
// hardcoded: allocate a one-byte object and see how much HeapInuse moves.
// Getting it wrong only skews BlocksPerObject, which is a relative reading
// anyway, but inferring it means the number stays right if the runtime
// changes.
var blockSize uint64 = 32

func calibrateBlockSize() {
	var before, after runtime.MemStats
	runtime.GC()
	runtime.ReadMemStats(&before)
	keep := make([]byte, 1)
	runtime.ReadMemStats(&after)
	if after.HeapInuse > before.HeapInuse {
		blockSize = after.HeapInuse - before.HeapInuse
	}
	blockSizeSink = keep
}

var blockSizeSink []byte

// readHeap takes a snapshot.
func readHeap() heapSnapshot {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)

	h := heapSnapshot{
		Inuse:   m.HeapInuse,
		Free:    m.HeapIdle,
		Total:   m.HeapSys,
		Objects: m.HeapObjects,
		Mallocs: m.Mallocs,
		Frees:   m.Frees,
		GCs:     uint32(m.NumGC),
	}
	if h.Objects > 0 && blockSize > 0 {
		liveBlocks := h.Inuse / blockSize
		h.BlocksPerObject = uint32(liveBlocks * 100 / h.Objects)
	}
	return h
}

// headroom is free bytes, kept as a name because several callers only want
// the one number.
func headroom() uint64 {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return m.HeapIdle
}

// fragmentationHint counts allocation failures the program survived.
//
// Fragmentation cannot be measured here, but it can be *inferred*: if the
// board reports plenty of free bytes and still cannot serve, the free bytes
// are not contiguous. The board cannot report that itself once it has
// aborted, so what is recorded instead is the last state before each failure,
// kept where it survives into the next response.
//
// Incremented by the adapter when a response cannot be built. See
// recordAllocPressure.
var (
	allocPressure uint32
	worstHeadroom uint64 = ^uint64(0)
	pressureMu    sync.Mutex
)

// heapHistory is a ring of recent snapshots, one per served connection.
//
// This is the answer to "what happened just before it went down". The board
// cannot report its own death - an out-of-memory aborts, so nothing runs
// afterwards - but it can keep the last N readings where a *later* request
// can read them. A client polling /diag as it loads sees the trend, and the
// reading from just before a stall is still there when the board comes back
// to answering.
//
// Sized so that at one sample per connection it covers a page load and the
// burst behind it several times over, for a fixed 32 x 24 bytes.
const heapHistoryLen = 32

var (
	heapHist   [heapHistoryLen]heapSnapshot
	heapHistAt int
	heapHistN  int
	heapHistMu sync.Mutex
)

// recordHeap appends a snapshot to the ring.
func recordHeap(h heapSnapshot) {
	heapHistMu.Lock()
	heapHist[heapHistAt] = h
	heapHistAt = (heapHistAt + 1) % heapHistoryLen
	if heapHistN < heapHistoryLen {
		heapHistN++
	}
	heapHistMu.Unlock()
}

// heapTrend reports the oldest and newest samples in the ring, and how many
// there are.
//
// Two points rather than the whole series: the question a reader has is "is
// it falling, and how fast", and the endpoints answer that in a line. The
// full ring is printed to the console at intervals for anyone who wants it.
func heapTrend() (oldest, newest heapSnapshot, n int) {
	heapHistMu.Lock()
	defer heapHistMu.Unlock()
	if heapHistN == 0 {
		return heapSnapshot{}, heapSnapshot{}, 0
	}
	newestAt := (heapHistAt - 1 + heapHistoryLen) % heapHistoryLen
	oldestAt := (heapHistAt - heapHistN + heapHistoryLen) % heapHistoryLen
	return heapHist[oldestAt], heapHist[newestAt], heapHistN
}

// recordAllocPressure notes that the board came close to failing, with the
// headroom at that moment. Called from the response path when a write fails,
// which is the last point at which anything can still be recorded.
func recordAllocPressure() {
	h := headroom()
	pressureMu.Lock()
	allocPressure++
	if h < worstHeadroom {
		worstHeadroom = h
	}
	pressureMu.Unlock()
}

// readAllocPressure returns the failure count and the worst headroom seen.
func readAllocPressure() (uint32, uint64) {
	pressureMu.Lock()
	n, w := allocPressure, worstHeadroom
	pressureMu.Unlock()
	if w == ^uint64(0) {
		w = 0
	}
	return n, w
}

// appendInt writes a signed decimal into b without allocating.
func appendInt(b []byte, n int64) []byte {
	if n == 0 {
		return append(b, '0')
	}
	if n < 0 {
		b = append(b, '-')
		n = -n
	}
	var tmp [20]byte
	i := len(tmp)
	for n > 0 {
		i--
		tmp[i] = byte('0' + n%10)
		n /= 10
	}
	return append(b, tmp[i:]...)
}

// reportRadio prints espradio's own counters.
//
// Worth having beside the heap numbers: a board that stops responding could
// be out of memory, or could have lost the radio, and these separate the two.
// Drops and full queues in particular are the difference between "the app
// leaked" and "the link fell over", which have nothing to do with each other.
func reportRadio() {
	println("auth verification self-check: allocations", authVerifyAllocs, "bytes", authVerifyBytes)
	var st espradio.Stats
	espradio.ReadStats(&st)
	println("radio:",
		"rx cb", int(st.RxCallbacks),
		"drops", int(st.RxDrops),
		"oversize", int(st.RxOversize),
		"queuefull", int(st.QueueSendFull),
		"isr drops", int(st.ISRRingDrops))
}

// pumpStack drives the network stack. Nothing arrives or leaves without it.
func pumpStack(stack *espradio.Stack) {
	for {
		sent, received, _ := stack.RecvAndSend()
		if sent == 0 && received == 0 {
			time.Sleep(pollTime)
		}
	}
}

// fail reports a fatal condition forever. There is no operating system to exit
// to and no log to write, so the serial console is the only place a problem
// can be seen - and it has to keep repeating, because whoever plugs in the
// cable will have missed the first print.
func fail(msg string) {
	for {
		println("FATAL:", msg)
		time.Sleep(5 * time.Second)
	}
}

// Exercise the real crypto path before Wi-Fi/router pools consume the heap.
// These counters distinguish board allocations from desktop escape analysis.
var authVerifyAllocs, authVerifyBytes uint64

func authBootCheck() {
	var before, hashed, verified runtime.MemStats
	runtime.ReadMemStats(&before)
	verifier, err := auth.HashPassword("boot-self-check")
	if err != nil {
		fail("auth self-check entropy failure")
	}
	runtime.ReadMemStats(&hashed)
	ok, err := auth.VerifyPassword(verifier, "boot-self-check")
	runtime.ReadMemStats(&verified)
	if err != nil || !ok {
		fail("auth self-check verification failed")
	}
	authVerifyAllocs = verified.Mallocs - hashed.Mallocs
	authVerifyBytes = verified.TotalAlloc - hashed.TotalAlloc
	println("auth self-check: hash allocations", hashed.Mallocs-before.Mallocs, "verify allocations", verified.Mallocs-hashed.Mallocs, "verify bytes", verified.TotalAlloc-hashed.TotalAlloc)
	runtime.GC()
}
