// Package eventlog is a small in-memory ring of recent server events, so that
// a board with no screen can be asked what it has been doing.
//
// This exists because diagnosing the board from outside has repeatedly meant
// guessing. A browser reports a CORS failure for a missing route, a 404 for a
// path that is registered, and a timeout for a board that is serving fine -
// and none of those tell you which it was. The serial console says more, but
// only to whoever is holding the USB cable.
//
// So the server records what it decided, and serves it back. A request that
// was refused says why; a request that never arrived is absent, which is
// itself the answer.
//
// Bounded by construction: a fixed-size ring of fixed-size entries, allocated
// once. A board running for a month uses exactly as much memory as one that
// booted a minute ago, which is the same discipline the rest of NanaCoin
// follows.
package eventlog

import (
	"strconv"
	"sync"
)

// MaxDetail bounds one entry's detail string. Long enough for a path and a
// reason, short enough that Capacity entries are a predictable cost.
const MaxDetail = 96

// Level classifies an event, so a reader can find the interesting ones without
// reading all of them.
type Level uint8

const (
	// Info is a request served normally.
	Info Level = iota
	// Warn is a request the server deliberately refused - an authorization
	// failure, a validation error, a preflight from an origin not on the
	// list. Expected, but worth seeing.
	Warn
	// Error is the server failing to do something it meant to do.
	Error
)

func (l Level) String() string {
	switch l {
	case Warn:
		return "warn"
	case Error:
		return "error"
	default:
		return "info"
	}
}

// Event is one recorded moment.
type Event struct {
	Seq    uint64 `json:"seq"`
	At     int64  `json:"at"`
	Level  string `json:"level"`
	Kind   string `json:"kind"`
	Detail string `json:"detail"`
}

// Log is a fixed-size ring of events. Safe for concurrent use: the HTTP
// handlers write to it from the router's worker goroutines and the status
// endpoint reads it.
type Log struct {
	mu      sync.Mutex
	entries [Capacity]Event
	methods [Capacity]string
	next    int
	seq     uint64
	now     func() int64
	health  HealthFunc
}

// New returns a log that timestamps with the given clock. A nil clock records
// zero timestamps, which is what a board with no RTC has before NTP anyway -
// the sequence number is the ordering that always works.
func New(now func() int64) *Log {
	if now == nil {
		now = func() int64 { return 0 }
	}
	return &Log{now: now}
}

// Add records an event. Never blocks on anything but the mutex, never
// allocates, and silently truncates an over-long detail rather than refusing
// to record - a clipped reason beats no reason.
func (l *Log) Add(level Level, kind, detail string) {
	if l == nil || Capacity == 0 {
		// A nil log is a working no-op, so callers do not need to check.
		// So is a zero-capacity one, which is what a no-logs build has:
		// the ring arithmetic below divides by Capacity, and there is no
		// slot to write into anyway.
		return
	}
	if len(detail) > MaxDetail {
		detail = detail[:MaxDetail]
	}

	l.mu.Lock()
	defer l.mu.Unlock()

	l.seq++
	l.methods[l.next] = ""
	l.entries[l.next] = Event{
		Seq:    l.seq,
		At:     l.now(),
		Level:  level.String(),
		Kind:   kind,
		Detail: detail,
	}
	l.next = advance(l.next)
}

// Recent returns up to limit events, newest first. limit <= 0 means all of
// them.
func (l *Log) Recent(limit int) []Event {
	if l == nil || Capacity == 0 {
		return nil
	}
	l.mu.Lock()
	defer l.mu.Unlock()

	n := Capacity
	if l.seq < uint64(Capacity) {
		n = int(l.seq)
	}
	if limit > 0 && limit < n {
		n = limit
	}

	out := make([]Event, 0, n)
	// Walk backwards from the most recently written slot.
	for i := 0; i < n; i++ {
		idx := back(l.next, i)
		out = append(out, l.eventAt(idx))
	}
	return out
}

// Count is how many events have ever been recorded, which is more than
// Recent can return once the ring has wrapped.
func (l *Log) Count() uint64 {
	if l == nil {
		return 0
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.seq
}

// Itoa is strconv.Itoa, re-exported so callers building a detail string do not
// have to import strconv alongside this package.
func Itoa(n int) string { return strconv.Itoa(n) }

// HealthFunc reports a line of host health - heap figures on the board,
// nothing on a desktop. Registered by the host so the log can serve it
// alongside the events, since the whole point of the log is to be readable
// without a USB cable attached.
type HealthFunc func() string

// SetHealth registers the health reporter. Optional.
func (l *Log) SetHealth(fn HealthFunc) {
	if l == nil {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	l.health = fn
}

// Health returns the host's health line, or empty if none was registered.
func (l *Log) Health() string {
	if l == nil {
		return ""
	}
	l.mu.Lock()
	fn := l.health
	l.mu.Unlock()
	if fn == nil {
		return ""
	}
	// Called outside the lock: the host's reporter reads runtime stats and
	// has no business contending with event recording.
	return fn()
}

// Each walks the recent events newest first, up to limit (<=0 means all),
// without allocating the slice Recent returns.
//
// The log is what gets polled when the board is already in trouble, so the
// diagnostic endpoint must not be the thing that pushes it over. Recent
// allocates a slice of up to Capacity events - each carrying two strings -
// which is a poor shape to build at exactly that moment.
//
// The lock is held for the whole walk, so fn must not call back into the log.
func (l *Log) Each(limit int, fn func(Event) bool) {
	if l == nil || Capacity == 0 {
		return
	}
	l.mu.Lock()
	defer l.mu.Unlock()

	n := Capacity
	if l.seq < uint64(Capacity) {
		n = int(l.seq)
	}
	if limit > 0 && limit < n {
		n = limit
	}
	for i := 0; i < n; i++ {
		idx := back(l.next, i)
		if !fn(l.eventAt(idx)) {
			return
		}
	}
}

// Request retains readable fields separately. The old hot path allocated both
// a status string and "METHOD path" for every request. Joining now happens only
// when logs are inspected; public log entries keep their familiar text shape.
func (l *Log) Request(level Level, status int, method, path string) {
	if l == nil || Capacity == 0 {
		return
	}
	kind := statusKind(status)
	if len(method) >= MaxDetail {
		l.Add(level, kind, method[:MaxDetail])
		return
	}
	if n := MaxDetail - len(method) - 1; len(path) > n {
		path = path[:n]
	}
	l.mu.Lock()
	defer l.mu.Unlock()
	l.seq++
	l.entries[l.next] = Event{Seq: l.seq, At: l.now(), Level: level.String(), Kind: kind, Detail: path}
	l.methods[l.next] = method
	l.next = advance(l.next)
}

func (l *Log) eventAt(i int) Event {
	e := l.entries[i]
	if m := l.methods[i]; m != "" {
		e.Detail = m + " " + e.Detail
	}
	return e
}

func statusKind(code int) string {
	switch code {
	case 200:
		return "200"
	case 201:
		return "201"
	case 204:
		return "204"
	case 400:
		return "400"
	case 401:
		return "401"
	case 403:
		return "403"
	case 404:
		return "404"
	case 405:
		return "405"
	case 409:
		return "409"
	case 413:
		return "413"
	case 429:
		return "429"
	case 500:
		return "500"
	case 503:
		return "503"
	default:
		return strconv.Itoa(code)
	}
}

// advance and back do the ring's index arithmetic.
//
// They exist because the compiler rejects a literal `% Capacity` as a
// division by zero when Capacity is the no-logs build's 0 - even on a path
// guarded by `Capacity == 0` and never reached. Routing the modulo through a
// function makes the divisor an ordinary value rather than a constant
// expression, so the no-logs build compiles. Callers still guard, so these
// are never actually entered with cap == 0.

func advance(next int) int {
	cap := Capacity
	if cap == 0 {
		return 0
	}
	return (next + 1) % cap
}

func back(next, i int) int {
	cap := Capacity
	if cap == 0 {
		return 0
	}
	return (next - 1 - i + cap*2) % cap
}
