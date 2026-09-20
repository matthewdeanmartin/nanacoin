# Historical hardware experiments

These notes were moved from the README when the current TinyGo guide was added.
They preserve the reasoning and measurements of earlier implementations.
**They are not current limits, architecture guarantees or operating instructions.**
In particular, old per-record growth estimates, 64-entry retry-cache figures,
allocation-free transport claims and references to board flash storage are
superseded. The board currently discards journal payloads and keeps its household
in RAM; retry receipts are bounded to 16 entries and 4 KiB.

For current behavior, read the [TinyGo guide](../docs/tinygo/index.md) and the
[project README](README.md). Later investigations are in AUTH_MEMORY.md,
HTTP_MEMORY.md, RAM_HISTORY.md, CONCURRENCY.md and WRITE_MEMORY.md.

## Packed records, because RAM is the budget

The ledger holds **PackedTransaction**, 48 bytes with no per-record
allocation, rather than the public `Transaction` type. Measured on the board:

| | bytes per transaction | transactions in 17 kB |
|---|---|---|
| public `Transaction` | 552 | 31 (~3 weeks) |
| packed | **55** | **316 (~32 weeks)** |

A tenfold reduction, from three changes:

- **Interned strings.** Account IDs, user IDs, kinds and memos repeat across
  every record. Interning turns five 16-byte string headers into three 16-bit
  references, and stores each distinct value once. A household has a dozen
  accounts and five kinds, so the table is tiny and constant.
- **Inline postings.** A `[2]` array rather than a slice, saving the 24-byte
  header and the separate allocation. Every transaction this system creates has
  exactly two postings; a third is now *refused* rather than stored, which is a
  real narrowing recorded in `ErrTooManyPostings`.
- **Derived IDs.** A transaction's ID is `txn-<sequence>` rather than nine
  random bytes, so the ID-to-record map becomes a slice index and the
  reversal index becomes a parallel slice of sequence numbers. Two
  string-keyed maps gone, each of whose per-entry overhead exceeded the record
  it pointed at.

**Derived IDs are guessable, and that is acceptable here.** A transaction ID is
not a capability in NanaCoin: reading one requires being a party to it or being
Nana, checked server-side on every request. The spec's requirement that
authorization codes and tokens be random is untouched - those are secrets,
these are names. The upside is that "txn-41 reverses txn-38" is legible in a
way two random strings were not.

**The wire format did not change.** The API still sends and receives strings;
packing is an internal storage decision and `Unpack` rebuilds the public form
on read. Every API test passed unchanged through this work, which is how that
claim is checked.

### The whole budget

Measured, with the benchmark harness subtracted - see below for why that
matters:

**Per request** (transient, reclaimed by the collector):

| | allocated | fits in 119 kB free |
|---|---|---|
| `GET /status` | 251 B | ~480 |
| `GET /me` | 256 B | ~470 |
| `GET /listings` | 651 B | ~186 |
| `POST /transfers` | 1,539 B | ~78 |

**Per record** (retained, grows with use):

| | allocated |
|---|---|
| packed transaction | 55 B |
| listing | 345 B |
| user, with its account | 509 B |

A **user costs nine times a transaction**, and a listing six times. Attention
went to transactions because they accumulate fastest, but they were already
the cheapest record - the packing described above shrank the one thing that
needed it least per unit.

A year of household use at the spec's estimate - 520 transactions, four
members, forty listings - is about **43 kB, or 37% of the free heap**, with
listings costing nearly half as much as all the transactions combined.

### A measurement error worth recording

Earlier versions of this section claimed a read cost 6,900 bytes and that
about fifteen requests fit in the free heap. Both were wrong, and the mistake
was in the instrument: the benchmarks built each request with
`httptest.NewRequest` and `httptest.NewRecorder`, which together allocate
about 5.5 kB. The harness was 96% of the measurement.

The board does not use `httptest` - `internal/boardhttp` builds a request from
fixed buffers and streams the response - so the figure never applied to it.
`BenchmarkServerCost` reuses one request and one writer, and reports the
handler chain instead.

Two conclusions that survived the correction: the collector still has to run
between connections (that was measured on the hardware, not in a benchmark),
and the packed records still cost 55 bytes. What did not survive was the
belief that per-request allocation was the binding constraint. It is not; the
retained records are.

### Measuring it

The trap, recorded because it wasted real effort: a seeded year measured 466
bytes per transaction and the ledger was blamed, when almost all of it was the
**in-memory journal** - which on the board is the flash file and costs no heap
at all. `TestWhereTheMemoryGoes` separates the layers:

```text
ledger only:              53 bytes/txn
service, flash journal:   55 bytes/txn   <- the board's real cost
service, memory journal: 316 bytes/txn
in-memory journal:       281 bytes/txn   <- flash on the board, not RAM
```

Measure with the backend the target actually uses, or the budget is charged
for memory that does not live there.


## Notes from making it work on hardware

Three things cost real time, recorded so they cost nothing next time.

**TinyGo's `net/http` does not implement Go 1.22 routing patterns.** Every
`mux.HandleFunc("GET /api/v1/status", ...)` silently fails to match and the
whole API returns 404 with no error anywhere. `internal/api/routes.go` uses
path-only patterns and checks the method inside each handler, and extracts
path variables by hand instead of using `r.PathValue`. Not as pretty; works on
both targets.

**The first TCP connection after boot takes 20–40 seconds** to establish, then
everything is fast. Worth knowing before concluding the server is broken — it
cost several rounds of misdiagnosis here.

**The WiFi link decides how long boot takes.** The access point this board
reaches is several floors away, and both the WPA2 handshake and DHCP fail
intermittently at that range. Both are retried inside the patched espradio,
the association indefinitely — so a board that cannot see its AP waits for it
rather than needing a power cycle. Expect anything from 5 seconds to several
minutes of `association attempt N failed` on the console, and treat that as
normal rather than broken.

**`espradio.NetConnect` can succeed at most once, and a failed call leaves an
object that panics.** `espradio.Enable` is a deliberate one-shot; a second
`NetConnect` returns `ErrAlreadyEnabled` having built no network stack, so the
next `Addr()` is a nil dereference. This is why the retries live in a patch
rather than in a loop in our code. Details in `patches/README.md`, including
the upstream issues worth filing.

### Streaming, because buffering did not fit

The board writes responses **straight to the connection** in 1 kB pieces.
Nothing holds a whole response. This is the change that made the ledger and
history endpoints work at all.

The measurements that forced it, per request:

| endpoint | response | allocated |
|---|---|---|
| status | 197 B | 6.5 kB |
| me | 149 B | 7.0 kB |
| history (50) | 4.5 kB | **18.1 kB** |
| ledger (50) | 4.7 kB | 17.8 kB |

Allocating four times the response size is `encoding/json` building
intermediate buffers, and against ~120 kB of spare heap that is what produced
`fatal error: out of memory`. Streaming the render instead:

```text
marshal into one buffer:   4,123 B/op   2 allocs/op
encode straight to writer:     0 B/op   0 allocs/op
```

The cost is that `Content-Length` cannot be known when the header goes out, so
the response is framed by connection close instead (valid HTTP/1.1, RFC 9112
§6.3). That costs nothing here: httphi already sends `Connection: close`,
serving one exchange per connection regardless.

The chunk is 1 kB rather than as small as possible. A smaller one turns a 3 kB
history into a dozen small TCP writes, which on a marginal WiFi link is slower
and more fragile than a few whole-packet writes; a larger one starts to be the
buffer this removed. `TestResponseChunkIsSensiblySized` pins both ends.

### The heap reading that finally explained it

The board reports its heap on **every response**, as `X-Nanacoin-Health`, and
in the `/logs` body:

```text
X-Nanacoin-Health: heap inuse 210992, delta 46992, idle 120160, objects 374, gc 0
```

A header rather than only the log endpoint, because by the time the board is
out of memory it cannot serve `/logs` either - so the headers of the last
request that *did* succeed are the final reading anyone gets. It is listed in
`Access-Control-Expose-Headers`, or the browser would hide it from the page.

That produced the measurement every earlier theory had been missing:

```text
status         inuse=210992  delta= 46992   <- 47 kB on the FIRST request
provision      inuse=223808  delta= 59808
create bob     inuse=278592  delta=114592   <- peak, 114 kB over baseline
POST listings  inuse=259424  delta= 95424
GET me         inuse=260816  delta= 96816
GET history    DEAD
```

**It is not a leak.** Memory does come back - 114 kB down to 95 kB - so
nothing is being retained forever. The problem is that the *baseline* cost of
serving a request is around 47 kB and the working set plateaus near 110 kB
against roughly 120 kB of spare heap. At that point any request can be the one
that tips it over, which is why the failure looked random and moved between
endpoints as the code changed.

The fix was an explicit `runtime.GC()` between connections. With it, the whole
sequence completes and the heap reaches a steady state rather than a ceiling:

```text
request  1   inuse=264416  delta=100544  idle=19744
request 10   inuse=264912  delta=101040  idle=19248
request 40   inuse=266752  delta=102880  idle=17408
request 50   inuse=266752  delta=102880  idle=17408
request 60   inuse=266752  delta=102880  idle=17408
```

Flat from request 40 onward - 60 requests, no failures. The board still runs
close to the edge, with about 17 kB spare at steady state, so there is no room
for complacency. But it is a stable 17 kB rather than a number that shrinks
until something fails.

**Two things this episode should have taught sooner.** The heap header was the
user's suggestion, and it produced in one run what several rounds of
theorising had not. And the reasoning that dismissed GC frequency - "18 kB
allocated against 120 kB spare, therefore there is room" - was exactly the
kind of plausible argument that measurement exists to check.

### The leak that was actually killing it

Separately, and the real culprit: **the idempotency cache never shrank.** Every
money-moving request stored a full marshalled response under its key, nothing
ever removed one, and the journal replayed them all at boot — so it grew across
reboots too. That is what a purchase attempt walked into.

Now bounded at 64 entries with FIFO eviction. A key only has to outlive the
retries of its own operation, so keeping every key forever bought nothing and
cost the heap.

Two things this ruled out, worth recording because both were plausible:

- **GC frequency, after all.** An earlier version of this note said the
  opposite, on the reasoning that 18 kB of allocation against 120 kB spare
  leaves room. The measurement disagreed: without an explicit collection the
  heap climbs to within a few kB of exhaustion and stays there, because
  TinyGo's collector is content to let it grow while memory remains. On a
  desktop that is efficient; with 120 kB spare it means the next request meets
  a nearly full heap. `runtime.GC()` between connections - when no request is
  in flight, so the pause costs nobody any latency - is what made the
  sequence complete.
- **Not large data structures.** Every domain slice is small and bounded. The
  allocation was all in JSON rendering and in the one map that never shrank.

### Why the board does not use net/http

The board ran on `net/http` first, because it let the same handler serve both
targets. Measured on the hardware: from a cold boot it answered **20 requests**
and then stopped accepting TCP connections. Reproduced across resets.

The failure was specific, which is what made it diagnosable:

- no panic on the serial console, so the program had not crashed
- ARP still resolved, so WiFi and the IP stack were alive
- TCP connections to port 80 timed out — only the listener was gone

That is the condition upstream espradio warns about: `net/http` allocates about
10 kB per connection, and TinyGo's conservative GC handles the resulting
fragmentation badly. The warning says a board "will likely crash after a
while"; in practice it stops listening instead, which is quieter and harder to
attribute.

So the board now serves `httphi`, espradio's allocation-free HTTP server. It
allocates its request buffers, response buffers and worker goroutines once at
`Configure` time and nothing per request.

**The API was not rewritten.** `internal/boardhttp` adapts one
`httphi.Exchange` into the `http.ResponseWriter` and `*http.Request` pair the
existing handlers already take, so `internal/api` stays the single definition
of the API on both targets. The one duplication left is a list of route
*patterns* in `boardhttp.Register` — paths, not logic, and a missing one shows
up as a 404 on the board while working on the desktop.

Measured on the board after the change:

| | requests before the listener stopped |
|---|---|
| `net/http` | **20**, then dead until a power cycle |
| `httphi` | **599/600** keep-alive, then **200/200** on fresh connections |

Under a sustained burst it can still stop accepting for a few seconds while
the connection pool drains, but it comes back by itself — which is the
difference that matters. The `net/http` build never did.

Dropping `net/http` also made the firmware smaller:

| | flash | RAM |
|---|---|---|
| `net/http` | 1,137,056 | 154 KB |
| `httphi` | 927,503 | 145 KB |

Three things to know if you touch this code:

- **Every path needs an OPTIONS route, GET-only ones included.** A GET
  carrying an `Authorization` header is not a CORS "simple request", so the
  browser preflights it; a path with no OPTIONS route answers 404, a 404
  carries no CORS headers, and the browser reports a missing
  `Access-Control-Allow-Origin`. That reads as a CORS misconfiguration and is
  actually a routing bug. It broke `/me` and `/status` after login. The route
  lists live in `internal/boardhttp/routes.go` - no build constraint, so
  `go test` checks them - and `TestEveryPathHasAPreflight` fails if one is
  missing.

- **The TCP pool is larger than the worker count** (`poolConns` vs
  `maxConns`). A connection holds its pool slot until the peer closes it and
  the closing timeout expires, so a client that does not reuse connections
  exhausts a tightly-sized pool and the board stops accepting — the same
  symptom as the `net/http` failure, from a completely different cause. This
  bit once already.

  Both of these numbers are measured, and both directions fail. `poolConns` at
  24 made the board stop accepting outright: each slot carries
  `TxBufSize`+`RxBufSize`, and 24 of those does not fit beside the router and
  the WiFi blob — and because the pool is heap-allocated the build size does
  not change, so it looks fine until it runs. `closingTimeout` at 250ms was
  shorter than a closing handshake needs and broke the listener immediately;
  at 2s a burst outran reclamation. 8 slots and 1s are what held.
- **`pool.CheckTimeouts()` runs on every accept-loop pass**, not only when the
  loop is idle. Under a steady stream of requests the idle branch is never
  reached, and reaping only there would let the pool fill up.

The board also prints a served-request count every ten requests, because "still
working" and "silently stopped accepting" are otherwise indistinguishable from
the outside.

## 2026-09-19 — the TCP pool was starving the heap

`poolConns` was 8, holding `8 x (TxBufSize 2048 + RxBufSize 1024)` = **24 KB**
of buffers on a board with about 10 KB free after setup. The pool was more
than twice the entire remaining heap.

The earlier note claimed 8 was "measured": that it served hundreds of requests
before a burst outran reclamation, and recovered by itself. That was true of
*sequential* traffic and false of the traffic the board actually gets. One
browsing user issues five requests at once, which is what a page load is.

Measured with Locust (`nanacoin_load`), browse scenario:

| | poolConns=8 | poolConns=5 |
|---|---|---|
| 1 user, 45s | 60 requests, **33% failed** | 230 requests, **0 failed** |
| p50 / p99 | 350 ms / 10,000 ms | 210 ms / **440 ms** |
| min free heap | **672 bytes** | 8,224 bytes |
| after the run | dead; needed a physical reset | healthy |
| ramp to 8 users | not attempted | 1,215 requests, 4.4% failed, survived |

At 8 slots a single user drove the heap to 672 bytes and the board stopped
accepting connections. It did **not** recover on its own: twenty minutes later
it was still unresponsive, with COM9 still enumerated and the serial console
silent - alive, but unable to serve. A reset over the CH343 bridge brought it
back.

At 5 slots every one of the seven load scenarios passes with no failures, and
the ledger balances after 218 transactions.

### Why the failures looked like a crash rather than backpressure

The firmware has a proper refusal path - 503 with `Retry-After: 1` - but it
never fired, because it lives *after* the connection is accepted. The failures
were all `ConnectTimeout`: the SYN got no reply at all.

`lneto`'s listener does send an RST when the pool is empty, which is a clean
"connection refused". But its `RSTQueue` is `buf [4]rstEntry` and documented to
"silently drop if the queue is full". So the board refuses the first few excess
connections and goes silent for the rest - and a client cannot tell that from a
dead board. Worth enlarging: 16 entries would cost about 170 bytes against the
24 KB the pool was already holding.

### If this is raised again

Do not raise `poolConns` without re-running `browse` at one user and checking
`min_free`. Sequential request counts say nothing about the concurrent burst a
single page load produces, and that is the case that kills it.
