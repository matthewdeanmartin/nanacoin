# boardprobe

For new investigations, use [evidence-first experiments](EXPERIMENTS.md).
`bottleneck.py` isolates workloads and saves request-stage, heap, serial and
recovery evidence on the host. The historical observations below describe
different builds; their causal interpretations are not all established.

Load-tests the board without hanging.

```
python probe.py watch --seconds 120      # is it up? does it flap?
python probe.py provision                # household + 3 users, saves a token
python probe.py stress --rounds 40       # write and read until it breaks
```

Pass `--ip` if the board moved. The token is cached in `.token` (gitignored).

## Why not curl in a loop

Because that is what kept hanging. A request to a board that has stopped
answering costs the full timeout, and a loop of those costs half an hour to
learn one fact — so the tool meant to *detect* a hang becomes the hang.

Here every request has a hard deadline (3s reads, 15s writes), the run has a
wall-clock budget, and `stress` gives up after a few dead rounds instead of
burning the remainder against a corpse. Samples print as they happen, so a
board that stops is visible on the next line.

Exit code is non-zero if the board finished unreachable, so it can gate a
build.

## Reading the output

`watch` prints a line per sample and flags every transition:

```
  12.1s  200  tx=47   inuse 0212768 delta 0015568 idle 0070384 ...
  >>> 14.3s -> DOWN
  14.3s  ---  timeout
```

Transitions are the interesting part. A board that flaps — answers, dies,
answers again — is failing differently from one that dies once and stays
dead, and the two have different causes. A browser getting one good response
in the middle of an outage is the flapping signature.

`stress` prints a line per round with the ledger size and the live heap:

```
r12  tx=125  ok=15  fail=0
r13  tx=130  ok=12  fail=3   read:0,read:0,xfer:0
```

`fail` counts requests that did not return their expected status; `:0` means
no answer at all, as opposed to `:500`, which means the board answered and
said no.

## Measured on hardware

First real run, 2026-09-17, ESP32-S3 N16R8 at ~-81 dBm.

### The leak the GC was hiding

Polling `/status` alone, empty ledger, with the between-connections
`runtime.GC()` removed:

```
inuse 199024 -> 231232    +2176 B per request
obj      369 -> 1055      +46 objects per request
gc      0000              the collector had NEVER run
```

At ~52 KB headroom that is about 24 requests to death, which is exactly how a
burst of reads took the board off the network: no HTTP, no ICMP, serial
silent, power LED on. A hang, not a reboot.

Restoring one `runtime.GC()` between connections:

```
per request   +2176 B  ->  +8 B      (270x)
objects       +46      ->  +0.2
gc            never    ->  every connection
headroom      countdown -> flat at 81 KB
```

The collector does not run on its own here because the allocator only collects
when it cannot satisfy a request from the free list — and with 50 KB free,
every small allocation succeeds immediately. The heap is consumed before the
collector is ever consulted.

### The ledger curve

Stress run, 45 transactions, 4 accounts, reads at `limit=100` every round:

```
r1   tx=6    idle 49040
r5   tx=18   idle 30032     steep - one-time growth
r9   tx=30   idle 28640     flattening
r14  tx=45   idle 24096     roughly flat, oscillating
```

It **plateaus**. Most of the early drop is one-time (session, listings map,
intern table), not per-transaction. The last five rounds cost ~4.5 KB for 15
transactions and the figure moves up as well as down, which is collector noise
rather than a leak.

No failures attributable to the board in that run: every `fail` was the test
sending a transfer the API correctly refused (self-deal, then insufficient
funds).

### The torture test reproduced the browser crash

`torture.py` is the one that finally broke it the way a person does. Four
concurrent members, each running a full session (login, browse, transfer,
listing, purchase, 30 deliberate errors, history, logout), every authenticated
request preceded by a real CORS preflight.

Clean board, 81 kB free:

```
r1   tx=6   reqs=150   11.2s   dead=2     idle 28736
r2   tx=?   reqs=147  289.2s   dead=102
r3   tx=?   reqs=4      6.0s   dead=106
*** board stopped answering at round 3 ***
slowest: 21.1s on abuse/not-json
```

Round 1 served 150 requests in 11 seconds. Round 2 took **289 seconds** and
lost 102. Dead by round 3.

### It is not memory - it is a first-request stall

Isolating that 21-second request, authenticated, one at a time:

```
0: malformed took 21.05s | idle 89888
1: malformed took  0.06s | idle 79920
2: malformed took  0.05s | idle 79696
...
```

**The first request after boot takes 21 seconds. Every one after it takes
60 milliseconds.** The heap is untouched throughout - 79 kB free the whole
time. Whatever the stall is, it is not the allocator.

That explains the shape of every failure seen so far:

- Sequential probes survived because one client pays the stall once, then
  proceeds at full speed.
- Four concurrent clients each hit the stall *at the same time*, each holding
  a pool slot and a worker goroutine for 21 seconds. With `Workers = 2` and
  `poolConns = 8`, the pool never drains and the board stops accepting.
- A person clicking around hits it because a browser opens several
  connections at once - so a human session looks like the concurrent case,
  not the sequential one.

The A/B is clean: same 4 users, same 5 rounds, `--no-preflight` stayed **UP**
with the heap flat at 70 kB; preflights on went **DOWN**. Preflights double
the connection count, which is what turns one stalled client into four.

### Packing the domain store: necessary, not sufficient

Users, accounts and listings were moved into fixed pointer-free arrays with an
interned-string table and a text arena (`internal/core/store.go`), the same
treatment the ledger already had.

On the host that works exactly as intended - 192 listing create/cancel cycles
moved the heap **-26 kB** (down, not up). Churn no longer accumulates.

On the board it did **not** fix the crash, and the numbers say why:

```
r1  idle 14208   <- was 28736 before packing
```

The store's 16 kB is now allocated *up front*, so the starting headroom is
lower. Packing converted a growing cost into a fixed one - which is the right
property and stops the long-run fragmentation - but it did not create room.

Tracing a single session shows where the room actually goes:

```
baseline          idle 74608
after provision   idle 54480    <- 20 kB for ONE user
after 1 login     idle 47888    <-  6.6 kB
after browse 1    idle 47472    <-  0.4 kB
after browse 2    idle 47088
after browse 3    idle 46736
```

**Reads are now nearly free** (0.4 kB per full page load, four requests each).
The cost is concentrated in `provision` and `login`: ~20 kB and ~6.6 kB, and
not reclaimed afterwards. That is where the next work belongs - PBKDF2's
working set, the journal encode path, and whatever the auth flow retains -
not in the domain objects.

### De-reflection: the fix that worked

`encoding/json` is gone from all production code - hand-written encoders
(`internal/api/jsonwriter.go`, `viewsjson.go`), hand-written parsers
(`jsonreader.go`, `requestsjson.go`), and a binary journal format
(`internal/core/wire.go`, `eventswire.go`).

Measured with `tinygo build -print-allocs`:

```
                  before   after   change
total sites:         742     709      -33
json + reflect:      138      67      -71   (-51%)
binary size:     952,960 920,000  -32,960
```

Per-operation, on the host:

```
parse a transfer:   264 B / 6 allocs  ->   48 B / 2 allocs  (6x faster)
encode a txn:       384 B / 2 allocs  ->  288 B / 1 alloc
journal a user:     389 B JSON        ->  225 B binary, 0 allocs to encode
```

**On hardware the torture test now survives.** Same suite that killed every
previous build at round 3:

```
r1   tx=6    idle 62624
r6   tx=30   idle 53168     6 rounds, up
...
r14  tx=91   idle 49728     20 rounds total, ~4500 requests, up
```

Headroom falls from 62 kB to 50 kB over 91 transactions and then holds.
`no-answer` counts are backpressure - connections refused while workers are
busy - not failures; `server-error(5xx)` stayed at 0 throughout.

The 67 remaining json/reflect sites come from TinyGo's own `net/http`, which
the board adapter still depends on. Removing them means replacing `net/http`'s
types in the handler signatures, which is a much larger change than anything
here.

### Still open

- **No persistent crash record.** Three storage attempts failed; see
  `cmd/nanacoin-esp32/blackbox.go`. When the board hangs there is still
  nothing to read afterwards.
- **The ceiling is unmeasured.** 45 transactions plateaued. Whether it holds
  at 500 or 5000 is not known, and "a year of household use" is the claim that
  has been wrong before.
- **What causes the 21-second first request?** Unknown, and it is now the main
  suspect rather than the heap. Candidates worth measuring: the TCP pool's
  `EstablishedTimeout`/`ClosingTimeout` interacting with a cold connection,
  the lneto stack's first-packet path, or DHCP/ARP resolution happening
  lazily on first use. It is *not* PBKDF2 - an unauthenticated malformed body
  stalls identically, and that path never hashes.
- **First write after boot is slow** — tens of seconds. `WRITE_TIMEOUT` is 45s
  because 15s timed out on a healthy board and made it look unreachable.
