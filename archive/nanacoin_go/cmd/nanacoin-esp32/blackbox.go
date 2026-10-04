//go:build tinygo

package main

// The black box: what the board knows about how it died last time.
//
// # The problem this solves
//
// The board lives downstairs by the router, unplugged. There is no serial
// console, and the WiFi upstairs is too weak to browse reliably - so the one
// channel that works is a response header on a request made from downstairs,
// and that channel closes at exactly the moment it becomes interesting. A
// board that has run out of memory cannot serve the page that would say so.
//
// The health header shows the heap *before* the crash, which is genuinely
// useful and is why it exists. What it cannot show is the crash itself: the
// last successful response is by definition the one before the failure, so
// the reading that matters most is always the one that was never sent.
//
// # What this does
//
// Nothing here can catch an out-of-memory. TinyGo's allocator calls
// runtimeFatal, which calls abort() - not a panic, not recoverable, no
// deferred function runs. Any design that depends on running code at the
// moment of death is wrong on this platform.
//
// So instead the board writes down what it is doing *before* each risky
// operation, and reads that back after the reboot. If the board comes up and
// the note says "was serving GET /api/v1/transactions?limit=30, heap at
// 4 kB", then that request is what killed it - and the note survived because
// it was written before the fact, not during.
//
// # Where the note lives: nowhere, for now
//
// It does not persist. Three attempts, all tested on the board:
//
//  1. A package-level Go variable. TinyGo zeroes .bss before main, so the
//     note was erased by the very reboot it existed to survive.
//  2. RTC slow memory at 0x50000000, written through a raw pointer. Reads
//     work; writes do not persist across a reset.
//  3. RTC fast memory at 0x600fe000. Same result.
//
// The giveaway on the third attempt was that rtc-fast reported a valid magic
// and a boot count on its *first* read, before anything had written there.
// Both addresses are almost certainly aliasing into the flash-mapped image
// rather than addressing real RTC SRAM, which means the "survival" observed
// in attempts 2 and 3 was reading a constant out of the binary, not reading
// back a note. TinyGo's esp32s3.ld maps no RTC section, and without one there
// is no supported way to place data there from Go.
//
// Making this work needs either a linker script change (a real .rtc_noinit
// section, which means patching the TinyGo target) or a small flash-backed
// record. Both are more invasive than a diagnostic should be, and the flash
// question is the next thing to look at anyway.
//
// So the struct below is live-only. Boots is always 1, Crashed is always
// false, and the endpoint says "clean boot" every time. That is deliberate:
// reporting a crash history that does not exist would be worse than reporting
// none, and the in-flight phase is still useful to a reader watching the
// serial console or polling /diag while the board is up.
//
// What still works without persistence:
//
//   - the health header, which shows the heap just before a failure
//   - alloc_failures and worst_headroom, which accumulate during a run
//   - the phase and route, for anyone watching live
//
// What does not: knowing what the board was doing when it died, after the
// fact. That remains the open problem.

import (
	"sync"
	"time"

	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/api"
)

// blackBoxMagic marks a note as written by this program rather than left over
// as uninitialised SRAM. Chosen to be a value unlikely to occur by accident.
const blackBoxMagic uint32 = 0x4E414E42 // "NANB"

// Phase is what the board was doing when the note was written. Small integers
// rather than strings: a string in the note would be a pointer into a heap
// that the reboot invalidated.
type Phase uint8

const (
	PhaseBoot Phase = iota
	PhaseIdle
	PhaseAccept
	PhaseServing
	PhaseWiFiRetry
)

func (p Phase) String() string {
	switch p {
	case PhaseIdle:
		return "idle"
	case PhaseAccept:
		return "accepting"
	case PhaseServing:
		return "serving"
	case PhaseWiFiRetry:
		return "wifi-retry"
	default:
		return "boot"
	}
}

// blackBox is the note. Kept deliberately small and free of pointers, since
// anything it referenced would not survive the reboot.
type blackBox struct {
	Magic uint32
	CRC   uint32

	// Boots counts power-ons plus resets, so a board that is silently
	// rebooting in a loop is distinguishable from one that is merely slow.
	Boots uint32

	// Phase and Route say what was in flight. Route is a small code rather
	// than a path string - see routeCode.
	Phase Phase
	Route uint8

	// HeapFree is the headroom recorded when the phase was entered, in
	// 64-byte units so it fits a uint16 up to 4 MB.
	HeapFree uint16

	// Served counts connections accepted since boot, so "died on the third
	// request" and "died after four hundred" are distinguishable.
	Served uint32
}

// box is the live note. Live-only: see the package comment for the three
// attempts at making it survive a reboot and why none of them worked.
var box blackBox

var boxMu sync.Mutex

// lastBoot is the note as it was found at startup, and whether it was
// trustworthy. Read by the diagnostics endpoint.
var (
	lastBoot      blackBox
	lastBootValid bool
)

// checksum is a cheap integrity check over the note's payload.
//
// Not cryptographic and not trying to be: the only adversary is uninitialised
// SRAM, and a simple mix rejects that with overwhelming probability while
// costing nothing on a board that writes this on every connection.
func (b *blackBox) checksum() uint32 {
	h := uint32(2166136261)
	mix := func(v uint32) {
		h ^= v
		h *= 16777619
	}
	mix(b.Magic)
	mix(b.Boots)
	mix(uint32(b.Phase))
	mix(uint32(b.Route))
	mix(uint32(b.HeapFree))
	mix(b.Served)
	return h
}

// recoverBlackBox reads the note left by the previous run, then re-arms it for
// this one. Must be called before anything else in main.
func recoverBlackBox() {
	// Nothing to recover: no storage survives the reboot on this toolchain.
	// The call is kept, and kept first in main, so that the day a persistent
	// location exists this is the only function that has to change.
	lastBootValid = false

	box = blackBox{
		Magic: blackBoxMagic,
		Boots: 1,
		Phase: PhaseBoot,
	}
	box.CRC = box.checksum()
}

// note records what the board is about to do.
//
// Called on the hot path, so it does no allocation and takes no measurement
// beyond a MemStats read. The mutex is uncontended in practice: the router
// serves Workers requests at a time and Workers is 2.
func note(p Phase, route uint8) {
	free := headroom() / 64
	if free > 0xFFFF {
		free = 0xFFFF
	}

	boxMu.Lock()
	box.Phase = p
	box.Route = route
	box.HeapFree = uint16(free)
	box.CRC = box.checksum()
	boxMu.Unlock()
}

// noteServed bumps the connection counter.
func noteServed(n uint32) {
	boxMu.Lock()
	box.Served = n
	box.CRC = box.checksum()
	boxMu.Unlock()
}

// routeCode maps a request path to a small integer for the note.
//
// A code rather than the path itself because the note must contain no
// pointers: a string field would point into a heap the reboot invalidated,
// and reading it back would be reading whatever now occupies that address.
//
// The mapping is coarse on purpose. Knowing it died on "a transaction list"
// is what matters; knowing which account's is not worth carrying a string
// across a reboot for.
const (
	RouteUnknown uint8 = iota
	RouteStatus
	RouteLogs
	RouteAuth
	RouteMe
	RouteUsers
	RouteAccounts
	RouteTransactions
	RouteListings
	RouteAdmin
	RouteDiag
)

func routeName(c uint8) string {
	switch c {
	case RouteStatus:
		return "status"
	case RouteLogs:
		return "logs"
	case RouteAuth:
		return "auth"
	case RouteMe:
		return "me"
	case RouteUsers:
		return "users"
	case RouteAccounts:
		return "accounts"
	case RouteTransactions:
		return "transactions"
	case RouteListings:
		return "listings"
	case RouteAdmin:
		return "admin"
	case RouteDiag:
		return "diag"
	default:
		return "unknown"
	}
}

// classifyRoute maps a path to its code without allocating.
//
// Takes bytes, so the adapter does not have to make a string copy of the
// request target just to categorise it. Comparisons against string literals
// are allocation-free in Go for []byte operands.
func classifyRoute(path []byte) uint8 {
	const prefix = "/api/v1/"
	if len(path) <= len(prefix) || string(path[:len(prefix)]) != prefix {
		return RouteUnknown
	}
	rest := path[len(prefix):]
	switch {
	case hasPrefix(rest, "status"):
		return RouteStatus
	case hasPrefix(rest, "logs"):
		return RouteLogs
	case hasPrefix(rest, "auth"):
		return RouteAuth
	case hasPrefix(rest, "me"):
		return RouteMe
	case hasPrefix(rest, "users"):
		return RouteUsers
	case hasPrefix(rest, "accounts"):
		return RouteAccounts
	case hasPrefix(rest, "transactions"):
		return RouteTransactions
	case hasPrefix(rest, "listings"):
		return RouteListings
	case hasPrefix(rest, "admin"):
		return RouteAdmin
	case hasPrefix(rest, "diag"):
		return RouteDiag
	}
	return RouteUnknown
}

func hasPrefix(s []byte, p string) bool {
	return len(s) >= len(p) && string(s[:len(p)]) == p
}

// lastBootLine describes the previous run in one line, for the console and
// the diagnostics endpoint.
//
// This is the sentence the whole mechanism exists to produce: after a crash
// upstairs, the next page load downstairs says what the board was doing when
// it went.
func lastBootLine() string {
	if !lastBootValid {
		// Always, on this toolchain. Worded so nobody reads it as "the board
		// has never crashed" - it means no record was kept, which is a
		// different and less reassuring statement.
		return "no crash record kept (nothing survives reboot on this build - see blackbox.go)"
	}
	b := make([]byte, 0, 128)
	b = append(b, "previous run: boot #"...)
	b = appendInt(b, int64(lastBoot.Boots))
	b = append(b, ", died while "...)
	b = append(b, lastBoot.Phase.String()...)
	if lastBoot.Phase == PhaseServing {
		b = append(b, " "...)
		b = append(b, routeName(lastBoot.Route)...)
	}
	b = append(b, ", after "...)
	b = appendInt(b, int64(lastBoot.Served))
	b = append(b, " connections, heap free "...)
	b = appendInt(b, int64(lastBoot.HeapFree)*64)
	return string(b)
}

// bootReport prints the previous run's fate at startup.
func bootReport() {
	println(lastBootLine())
	if lastBootValid && lastBoot.Phase == PhaseServing {
		println("  -> the route above is the prime suspect; it was in flight when the board stopped")
	}
}

// bootTime is when this run started, for the uptime figure.
var bootTime = time.Now()

// boardDiagnostics is the api.DiagnosticsFunc for this host.
//
// This is what a browser downstairs sees at GET /api/v1/diag after the board
// has come back from a crash upstairs. It is the whole reason the black box
// exists: the console is unreachable, the event log died with the reboot, and
// this is the only channel left.
func boardDiagnostics() api.Diagnostics {
	failures, worst := readAllocPressure()

	oldest, newest, n := heapTrend()

	d := api.Diagnostics{
		Machine:       machineDiagnostics(),
		LastBoot:      lastBootLine(),
		Crashed:       lastBootValid,
		AllocFailures: failures,
		WorstHeadroom: worst,
		UptimeSeconds: int64(time.Since(bootTime).Seconds()),
		Health:        healthLine(),

		FreeNow:    newest.Free,
		FreeOldest: oldest.Free,
		FreeDrop:   int64(oldest.Free) - int64(newest.Free),
		Samples:    n,
		ObjectsNow: newest.Objects,
		FragNow:    newest.BlocksPerObject,
		FragOldest: oldest.BlocksPerObject,
		GCsNow:     newest.GCs,
		GCDelta:    newest.GCs - oldest.GCs,
	}
	if lastBootValid {
		d.Boots = lastBoot.Boots
		d.Phase = lastBoot.Phase.String()
		d.Route = routeName(lastBoot.Route)
		d.ServedBeforeCrash = lastBoot.Served
		d.HeapFreeAtCrash = uint64(lastBoot.HeapFree) * 64
	}
	return d
}

// boardObserver feeds the black box from the HTTP adapter.
//
// The note is written before the handler runs, which is the only ordering
// that works here: an out-of-memory aborts, so nothing gets to run afterwards
// and a record written on the way out would never be written at all.
type boardObserver struct{}

func (boardObserver) BeginRequest(path []byte) {
	note(PhaseServing, classifyRoute(path))
}

func (boardObserver) EndRequest(ok bool) {
	if !ok {
		// The response could not be written. Usually the peer went away,
		// which is ordinary - but it is also what a near-miss on memory looks
		// like from here, so the headroom at this moment is worth keeping.
		recordAllocPressure()
	}
	// Back to idle, so a crash between requests is not blamed on the last
	// route served.
	note(PhaseIdle, RouteUnknown)
}
