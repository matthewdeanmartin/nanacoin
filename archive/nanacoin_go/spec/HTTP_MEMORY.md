# HTTP request memory work (2026-09-18)

Near-router testing reproduced unresponsiveness with one virtual browser issuing
five overlapping reads, including a fresh-boot browse-only run. Free heap reached
effectively zero in the preceding run. Four concurrent writes previously produced
a serial-confirmed OOM. See ../nanacoin_load/FINDINGS.md for the workloads/evidence.

## Implemented

- Each of the four adapter buffer sets now owns its http.Request, URL, body reader,
  response writer, request/response header maps, and header-value slots. Cleanup
  clears references before returning the set, including rejected requests and
  failed responses. The adapter is also compiled on the host so its actual code
  can be exercised by race tests, rather than testing only an approximation.
- API header writes reuse available value slots. Ordinary origin-form URLs use a
  caller-owned URL; escaped/unusual forms retain the standard parser's behavior.
  Request strings are still copied. No unsafe aliases to recycled storage are
  exposed to domain state, event logs, or handlers that retain strings.
- TinyGo's allocation report showed the cleanup closure and captured variables
  escaping. A method defer eliminates those allocation sites in the new report.
- The API consumes the body already buffered by the adapter. Previously
  decodeInto allocated another 1,400 bytes per request in TinyGo, despite comments
  calling it a stack buffer. The desktop fallback is a separate noinline function,
  so its allocation is not hoisted into the board's buffered branch. Four writes
  previously had up to 5,600 bytes of redundant body buffers alone.
- The response writer and status recorder implement StringWriter, avoiding the
  string-to-byte-slice fallback during streaming JSON output.
- Truncated bodies are rejected before reaching the handler rather than silently
  returning a shorter body than the declared Content-Length.

## Validation and limits

Full `go test -race ./internal/...` and `go vet ./internal/...` passed. Regression
tests cover stale credentials/headers, retained string ownership, independent
concurrent workers, failed-response reset, truncated bodies, buffered body reader
position, and header replacement/addition semantics. Differential URL fuzzing
found `/!` needs RawPath preservation; after correction, 951,087 fuzz executions
passed. That counterexample is retained as a regression seed.

The host's adapter-only benchmark (GET with Authorization, including httphi
parsing and a small streamed response) measures 26 B/op and 4 allocs/op. It does
not measure the full API or TinyGo, and is not a zero-allocation claim. A TinyGo
ESP32-S3 ELF and flashable image build successfully. Inspected ELF frames include
adapter serve 224 bytes, finishRequest 64, WriteHeader 144, and decodeInto 48.
These are individual frames, not whole call-chain bounds.

The router's configured worker mode allocates exchanges and goroutines up front;
Exchange.Release clears the connection reference. The dynamic-mode allocation
sites in its source are not the configured serving path. No network-stack rewrite
or worker/TCP/history capacity reduction was made.

Remaining allocations include copied request strings, unusual URL parsing, API
encoders/closures, request parser scratch, health strings, and event-log strings.
The log ring bounds entry count but retains the strings placed into those entries.
Moving object storage to startup also spends fixed RAM, so the new boot headroom
must be measured. This is a concrete reduction of avoidable request memory, not a
claim that all allocation or fragmentation is solved.

## Attached-board results, 15:29 EDT

Flashed the 937,152-byte image through COM8; esptool verified the written data.
Four workers, eight TCP slots, and 512 ledger entries remain configured. The board
is attached upstairs again, so latency is not a controlled near-router comparison.

The one-user/five-read browse workload passed 245 requests over 60 seconds with
zero failures (previous fresh-boot firmware failed after 65 successes). Minimum
sampled heap was 976 bytes, p95 about 700 ms. A subsequent E2E run failed on its
first transfer after a 4,544-byte free-heap sample. Its serial capture ended before
the failure text, so that particular failure was not confirmed from serial.

Reset the same image for an isolated write experiment, without reflashing.
COM8's reset left HTTP and serial silent; a native COM9 reset restored execution.
Fresh preparation passed, then E2E passed transfer debit and reversal restoration
before a purchase produced `fatal error: out of memory` / `abort called` on serial.
The four-overlapping-write check was never reached. The last successful response
sampled 7,504 free bytes; this is not the instantaneous heap at failure.

Serial baseline deltas imply 15,056 bytes free after setup, versus 20,928 before
HTTP reuse: about 5.9 KB spent on fixed request/response storage. This tradeoff
improves browse survival but leaves insufficient headroom for some writes. It is
not a complete fix, and mixed-workload durability has not passed. Next work must
measure write/purchase peak allocations and reduce fixed overhead where possible.
Reports and serial paths are recorded in ../nanacoin_load/FINDINGS.md.
