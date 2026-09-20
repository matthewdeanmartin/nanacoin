# Diagnostics and load tests

The shared Machine health page now accepts TinyGo's `/api/v1/diag` and
`/api/v1/diag/static`. TinyGo samples its managed heap on request on the single
active core, preserving its original crash/trend fields. Unsupported hardware
metrics are unavailable, and PSRAM is explicitly disabled. Static metadata
does not pretend to enumerate flash partitions. See the
[parallel Rust/TinyGo diagnostic explanation](../rust/diagnostics.md) for the
wire format, memory bounds and differing sampling models.

"The site stopped responding" is an observation, not a diagnosis. It can mean
the board ran out of memory, the listener exhausted its connections, a worker
stalled, the radio lost contact, or the browser refused the request before
sending it.

The useful test leaves enough evidence to distinguish those cases.

## Observe through more than one channel

| Evidence | What it helps establish | Important limit |
|---|---|---|
| HTTP status and latency | What a client experienced | A timeout alone does not prove a crash |
| `X-Nanacoin-Health` response header | Memory state on successful responses | Last successful reading precedes the failure |
| Status/diagnostic endpoints | Uptime, counters and current state | They also require a working server |
| `/api/v1/logs` | Recent request outcomes and CORS decisions | Only 58 retained events; not every transport failure reaches middleware |
| Native USB serial capture | Boot, GC, in-flight diagnostics, panic or OOM text | Must be recording when the event happens |
| Load-test artifacts | Workload, timing, errors and recovery attempts | Interpretation depends on fixture and network placement |

Health headers are exposed through CORS so the browser can read them. This
also preserves a last known reading when the board later cannot answer a
dedicated diagnostic request.

An absent log entry is not proof that a request never reached the device. It
may have been overwritten, failed before the logging layer, or not completed.
The log is publicly readable for troubleshooting login failures; do not put
passwords, tokens or private domain data into diagnostic messages.

## Interpret memory at comparable points

Compare readings after startup and after completed work at equivalent GC
points. A response-header reading taken during work and a post-GC serial
reading answer different questions.

- Rising retained memory across repeated identical cycles suggests something
  is being kept alive or progressively filled.
- Stable retained memory with burst failures suggests peak demand deserves attention.
- A high total-free reading does not rule out fragmentation or a stack failure.
- `journal_used` on the discarding board backend is logical byte accounting,
  not a heap-consumption graph.

The board explicitly runs collection in its serving loop. That is an
implementation choice, not a substitute for fixing unbounded retention or
proof that all dependencies allocate safely.

## Use the separate Locust project

The sibling `nanacoin_load` directory keeps Python concerns out of the Go
module. It uses Python 3.14, `uv`, Locust and a Makefile. From that directory:

```powershell
make install
make test
make prepare
make e2e
make load
make reports
make open
```

Consult its
[README](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_load/README.md)
for host configuration, Makefile overrides and current CLI options. Preparation
creates test users and balances: use a disposable household. After a board
restart, prepare again rather than using stale tokens from the previous boot.

For a short, explicit staged workload:

```powershell
uv run ncload run --scenario write --steps 1,2,4 --stage 30
```

Choose the actual board address through the project's configuration. When
attached, add its serial capture option with the native USB port, ensuring
another monitor is not already using it. A router-side test without USB can
still collect HTTP evidence, but cannot recover serial output after a crash.

## Choose a workload that answers a question

| Scenario | What it stresses |
|---|---|
| `status` | Basic request/connection lifecycle with little domain work |
| `browse` | The browser-like fan-out of several reads |
| `ledger` | History rendering, response size and streaming |
| `write` | Fresh operations, commits, receipt churn and ledger retention |
| `replay` | Repeated keys and overlapping retry coordination |
| `market` | Listing creation, purchases, shared text and closed-slot reuse |
| `auth` | Password computation, codes, sessions and their bounded stores |

Begin with an end-to-end correctness run, then a modest baseline. Increase
one variable at a time: concurrency, payload size, connection churn or history
age. A ring-wrap experiment needs enough writes to pass the retention boundary;
a retry experiment needs repeated keys; an auth experiment needs session churn.
Randomly mixing everything may find failure quickly while hiding its cause.

Use maximum valid escaped text as well as ordinary short text. Test both fresh
and aged stores. Reach capacity intentionally and verify a controlled refusal
or documented eviction instead of merely checking that the process survived.

## Preserve the failure

Keep the HTML/Locust reports, CSV measurements, JSONL events, run metadata and
serial output together. The harness records evidence during execution so a
failed run can still produce useful artifacts. Its bounded diagnostic-failure
handling prevents an unreachable board from being hammered indefinitely.

Record firmware identity, toolchain/dependency versions, worker and pool
settings, initial household state, router distance, and every manual reset or
power interruption. A fresh uptime can establish a restart; it cannot by
itself distinguish a crash from somebody unplugging the board.

After failure, collect the available tail and limited recovery observations
before rebooting. Do not reflash a previous image merely to keep this
development device responsive between tests.

## A practical diagnosis order

1. Check browser request details or the load generator's actual error. Did
   the client send the intended URL and method?
2. Check serial for OOM, panic, a fresh boot, or an unfinished request.
3. Compare uptime and last successful memory/counter readings.
4. Separate HTTP refusal from TCP connection failure and WiFi loss.
5. Reproduce with a narrower scenario at the same placement.
6. Move near the router to separate application pressure from a poor link.

Weak WiFi can indirectly increase application memory pressure by keeping
connections and requests alive longer. Conversely, an HTTP timeout is not
evidence of overheating. Do not assign an I/O, memory or thermal cause without
an observation that supports it.

For focused scripts and historical findings, see the
[board-probe tools](https://github.com/matthewdeanmartin/nanacoin/tree/main/nanacoin_go/tools/boardprobe).
Use previous experiments to form hypotheses, not as current capacity ratings.
