# Live board performance, September 25, 2026

Read-only probes from the Windows development machine, after the board was
plugged back in. Both HTTPS probes verified their household CA and hostname
while connecting by IP. No firmware was installed or settings changed.
These are small public-read samples, not authenticated application coverage.

| Measurement | NanaCoin (`192.168.1.158`) | Mastomini (`192.168.1.161`) |
|---|---|---|
| Isolated fresh HTTPS | 1.75 s mean across 9 reads | About 1.0 s mean for each public endpoint |
| Four clients, fresh HTTPS | 5.01 s mean, 7.07 s maximum across 8 status reads | Instance: 5.26 s mean, 9.10 s maximum across 12 reads |
| Open HTTPS connection | Server advertises close; zero reuse observed | Isolated subsequent reads: 11–34 ms |
| Server application timing | Not instrumented | Under 9 ms on all 60 sampled requests |

NanaCoin's nine isolated reads covered diagnostics, status and one public
transaction. TLS setup averaged 1.53 s out of 1.75 s total (87%). This is time
spent establishing TLS as seen by the client, including network and server
scheduling, not a measurement of cryptographic CPU alone. The board reports
160 MHz, ESP-IDF 5.5.3, RSSI around -65 to -67 dBm, and a progressing diagnostics
sampler. These readings do not isolate Wi-Fi power saving or packet loss.

Mastomini's recent transport work is present in source, and its live timing
headers and fast reused responses are consistent with those improvements.
Four newly connecting clients still produce seconds of delay despite short
application execution. Reused four-client requests mostly recover to tens of
milliseconds, with roughly 950 ms outliers while other clients establish their
initial connections. Its benchmark uses requests without TLS session resumption;
fresh-connection figures do not predict clients that successfully resume TLS.

Both listener implementations perform synchronous TLS handshakes on the HTTPS
server task. NanaCoin additionally advertises `Connection: close` on every
response, uses esp-idf-svc's chunked small-write response path, and leaves Wi-Fi
power saving at its default. Mastomini already implements connection reuse,
TCP_NODELAY before handshake, consolidated responses, TLS tickets, disabled
modem sleep and a 240 MHz build. See its `spec/08-performance.md` for its prior
controlled comparisons; those prior measurements are distinct from this run.

The evidence supports prioritizing transport over caching domain reads:

1. Give NanaCoin actual persistent connections and request-stage timing.
2. Port the independently useful Mastomini transport changes with firmware and
   hardware verification, rather than assuming desktop speed proves improvement.
3. Investigate why real clients reconnect or fail to resume TLS, then address
   the synchronous handshake queue shared by both boards if cold bursts remain
   unacceptable. Increasing socket count alone does not add handler concurrency.
4. Measure authenticated, representative pages before optimizing serialization
   or ledger operations; public endpoints cannot establish their costs.

Local raw evidence is under `nanacoin_rs/.local/`:

- `mastomini-perf-current.json`: 60/60 successful public HTTPS requests, one and
  four clients, new and reused connection pools, three rounds per worker.
- `nanacoin-perf-current.json`: 36/36 successful reads with phase timings and
  diagnostic snapshots. Only its first nine fresh HTTPS samples are an isolated
  HTTPS baseline: a second probe overlapped the later reuse and HTTP phases.
- `nanacoin-perf-burst-isolated.json`: the replacement isolated four-client
  experiment, 8/8 successful status requests, run after both earlier probes ended.
- `nanacoin-perf-burst.json`: overlapping exploratory run; do not use this as
  the controlled four-client result.

The reusable NanaCoin phase probe is `nanacoin_rs/scripts/perf-board.py`.
Burst measurements used the existing sibling
`mastomini_rs/scripts/bench-api.py` with explicit public endpoint paths, CA,
TLS hostname and IP address. No Mastomini files were edited.

## Follow-up: matched plain HTTP versus HTTPS

The initial attribution needed a direct HTTP comparison. The follow-up used
the same client, endpoint, four rounds per worker and connection policy for
each protocol. Experiments ran sequentially without overlapping our probes.
NanaCoin used `/api/v1/status`; Mastomini used `/api/v2/instance`.
HTTPS retained CA and hostname verification. These are comparisons of the
deployed listeners, whose socket limits differ, not a TLS-only microbenchmark.

| Fresh connection for each request | HTTP mean | HTTPS mean | HTTP successes | HTTPS successes |
|---|---:|---:|---:|---:|
| NanaCoin, one client | 252 ms | 1,809 ms | 4/4 | 4/4 |
| NanaCoin, two clients | 166 ms | 2,901 ms | 8/8 | 8/8 |
| NanaCoin, four clients | 75 ms | 5,393 ms | 10/16 | 16/16 |
| Mastomini, one client | 64 ms | 975 ms | 4/4 | 4/4 |
| Mastomini, four clients | 241 ms | 3,581 ms | 16/16 | 16/16 |

Means exclude failures. NanaCoin's four-client HTTP number is therefore
**not** a successful-throughput comparison: six calls failed with connection
errors. Its HTTP listener permits two sockets versus four for HTTPS. The
two-client comparison avoids exceeding that configured HTTP capacity and had
no failures. Attempted reuse on NanaCoin still reconnects; at four clients
HTTP had another 4/16 connection errors, while all HTTPS calls succeeded.

To separate initial connection queueing from sustained requests, another probe
opened four Mastomini client sessions, completed an unmeasured request on
each, then released a barrier and measured five requests per client. All
twenty measured requests succeeded in each protocol/run:

| Established connections, four clients | HTTP mean / median / max | HTTPS mean / median / max |
|---|---:|---:|
| First run (HTTP then HTTPS) | 438 / 44 / 1,916 ms | 55 / 55 / 104 ms |
| Repeat (HTTPS then HTTP) | 46 / 45 / 68 ms | 53 / 51 / 93 ms |

Thus connection reuse can remove several seconds of response time under
contention, not just the requesting client's own one-second handshake. The
same server task handles new TLS handshakes and existing HTTP requests; a
request can wait behind other clients' handshakes. An illustrative four-client
queue of one-second handshakes delays clients by approximately 1, 2, 3 and 4
seconds before accounting for other work. This is an explanation of the
mechanism, not a precise decomposition of every observed outlier.

The HTTP-only 1.9-second outlier means **not all stalls are TLS**. It did not
recur in the reversed-order run; these measurements cannot distinguish network
loss, external traffic or scheduling as its cause. The sustained multi-second
cost is strongly associated with fresh HTTPS connections, while ordinary
established HTTPS reads are about as fast as HTTP. This does not establish
costs of authenticated timelines, writes or large responses.

Raw follow-up files under `nanacoin_rs/.local/`:

- `{nanacoin,mastomini}-matched-{http,https}.json`: one/four clients,
  fresh/reuse modes, four rounds per worker.
- `nanacoin-two-clients-{http,https}.json`: fresh connections at two clients.
- `mastomini-fully-warm.json` and `mastomini-fully-warm-reverse.json`:
  synchronized, established-session runs, with the local driver `compare-warm.py`.

## NanaCoin fix and deployment

The replacement transport keeps HTTP/1.1 connections open and moves TLS
establishment to a bounded, nonblocking task on core 0. A separate core-1 loop
serves established HTTP/HTTPS clients fairly, without blocking on partial
requests or slow socket writes. It supports eight TLS and four HTTP clients,
two pending handshakes, TLS session tickets and TCP_NODELAY. TLS buffers use
PSRAM; the CPU runs at 240 MHz and Wi-Fi modem sleep is disabled. API responses
include `Server-Timing` so application/lock time can be distinguished from
connection and network time. Domain writes retain their existing serialization
and durability semantics.

Hardware verification caught two implementation issues before completion:
sub-tick libc sleeps busy-wait on this IDF build, so the transport loops now use
explicit FreeRTOS delays; Rust pthread attributes override the IDF thread-stack
configuration, so stack sizes are set on Rust's thread builders. Startup now
retries transient Wi-Fi association/DHCP failures instead of exiting the app.

The owner identified COM9 as NanaCoin and explicitly authorized discarding its
development ledger. The existing journal refused startup with `CorruptJournal`;
only the 8 MiB ledger partition at `0x410000` was erased. No migration or legacy
decoder was added. The board is now unprovisioned and needs household setup.
Wi-Fi configuration, CA/server identity, bootloader and partition table remain
in place. Measurements below use the fresh ledger and public read endpoints;
they do not establish performance for password hashing or large financial books.

| Updated board test | Samples | Mean | Median | Maximum |
|---|---:|---:|---:|---:|
| Established HTTPS status reads | 20 | 19.7 ms | 18.2 ms | 36.0 ms |
| Established reads during a cold TLS handshake and a silent TLS client | 50 | 25.7 ms | 23.5 ms | 57.7 ms |
| Established reads during another client's partial HTTP request | 10 | 25.5 ms | 24.2 ms | 53.6 ms |
| Eight established HTTPS clients | 200 | 64.3 ms | 51.2 ms | 234.1 ms |
| Four established clients over 90 seconds, with nine periodic cold TLS connections | 360 | 87.2 ms | 44.2 ms | 1,600.5 ms |

All requests succeeded. One resumed TLS connection plus its first request took
68.8 ms. The simultaneous full cold handshake plus request took 1,020.7 ms,
while the silent handshake was closed after approximately 4.05 seconds.
Pipelined request boundaries, duplicate Content-Length rejection and the exact
71,694-byte gzip JavaScript asset also passed live checks.

A separate sequential phase probe completed all 36 reads across HTTP/HTTPS and
fresh/reused connections. Warm HTTPS reads averaged 18.8 ms (eight measured
requests excluding initial establishment); fresh HTTPS averaged about 1.01 s,
including 949 ms of TLS work/wait on average. Thus full connection establishment
still costs roughly a second, but no longer blocks the established serving loop.
These small runs are diagnostic evidence, not a guarantee of a latency percentile.
RSSI at the end of the transport checks was -60 dBm.

The longer run completed all 360 warm reads and nine cold connections without
errors or a reboot. Of the warm reads, 210 were at most 53 ms, 292 were at most
100 ms, and four exceeded one second; observed p95 was 230.6 ms. Reported handler
time stayed at or below 1.054 ms, including the slow responses. This rules out
slow API execution for those samples, but does not distinguish pre-handler
scheduling, network loss or socket transmission as the remaining outlier cause.
The result is a large reduction in typical latency, not elimination of all
stalls. The sampler advanced from 79 to 125 samples during the run; internal
free memory recovered to 96,595 bytes after clients closed, with a 38,912-byte
largest block and a 27,599-byte observed minimum. RSSI ranged from -58 to -61 dBm
in the saved boundary snapshots.

`make check`, `make web-check`, firmware compilation and deployment dry-run
passed. The final application was written to COM9 through the standard deployment
script, which verified the existing partition table and application write hash.
The strict live-board probe passed CA/hostname verification, balanced ledger,
exact bundled Angular index, public ledger/diagnostics and matching CA download.
No browser automation surface was available, so interactive UI behavior was not
verified in a browser.

Application artifact: 2,837,104 bytes, built from the working tree based on
`7c2a025`; SHA-256
`cd08c1725bf1830f518e017902e9e2dd36b266414ef718007aa4d8816c6cb9aa`.
Raw evidence is in ignored `nanacoin_rs/.local/transport-after.json`,
`perf-after.json`, `soak-after.json`, `live-stackfix.log` and `deploy-latest.log`.
The strict live-board probe passed again after the stability run.
