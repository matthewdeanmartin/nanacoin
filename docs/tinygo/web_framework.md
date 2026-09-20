# The web framework

For the parallel implementation, see [Rust HTTP and concurrency](../rust/architecture.md).
Rust uses ESP-IDF's HTTP(S) transport, not this Go transport replacement.

NanaCoin's framework is a small set of in-repository layers, built around
ordinary Go HTTP handlers. The board replaces the expensive transport pieces
while preserving shared application behavior.

```text
Angular / another HTTP client
          |
    WiFi: espradio
          |
    TCP: lneto connection pool
          |
    HTTP: httphi fixed workers
          |
    internal/boardhttp adapter
          |
    internal/api handlers and middleware
          |
    internal/core service
          |
    ledger, users, marketplace, journal
```

On the desktop, Go's `net/http` server replaces the radio/TCP/httphi/adapter
portion. Both paths reach the same `internal/api` and `internal/core` packages.
Using `http.Request` as an interface type does not imply that the board is
running the standard library's HTTP server.

## The transport owns bounded resources

The board configures four request workers and eight TCP slots. Each TCP slot
has a 2 KiB transmit buffer and 1 KiB receive buffer: 24 KiB across the pool,
before worker state, headers, stacks or radio memory are counted.

Request and response header buffers are each 1 KiB. The board adapter caps
request bodies at 1,024 bytes. Shared typed decoding has a separate 1,400-byte
ceiling; the smaller board limit still wins. An older general decoder's larger
limit is not permission to send a larger body to the board.

Responses are sent in 1 KiB pieces. The current transport closes the connection
after an exchange; streaming can therefore use connection-close framing
instead of buffering the entire body to compute its length.

A response buffer is not a response-size limit for a streamed list. It is a
working-space limit. By contrast, a single encoded record must fit its bounded
record storage. There are four record buffers, each with 2,560 bytes for its
encoded data and associated fixed working state.

## Routing exists at two layers

`internal/api/routes.go` registers path-only patterns with the shared mux and
handlers check methods. This avoids depending on desktop Go routing features
that the TinyGo implementation has not supported equivalently, including
Go 1.22 method patterns and `PathValue`.

`internal/boardhttp/routes.go` separately lists the method/path shapes accepted
by httphi. Its syntax belongs to that router and includes wildcard shapes.
This is an admission list, not a second implementation of business rules.
An endpoint can work on the desktop and return 404 on the board if this list
was not updated.

OPTIONS paths are derived for preflight handling. Even a GET with an
`Authorization` header can trigger a browser preflight. A missing OPTIONS
route can look like a CORS policy failure when it is actually a routing bug.

## One request through the application

Consider a transfer:

1. A TCP slot accepts the connection and a worker parses the HTTP exchange.
2. The adapter exposes reusable request/response objects to the shared handler.
3. Middleware applies CORS and diagnostic behavior. Authentication resolves
   the bearer token where required.
4. A typed parser reads the bounded body and validates its representation.
5. The service checks permissions, account state, amount and available funds.
6. [Commit processing](storage.md#commit-processing) records the intended change
   through the journal contract and applies it under the service lock.
7. The response is encoded into bounded working storage and sent; borrowed
   resources are released even on errors.

Parsing validity and domain validity are distinct. A syntactically correct
JSON request can still ask a non-Nana user to issue money, or attempt to spend
more than an account owns.

## Typed JSON is a deliberate trade

The hot paths use explicit readers and writers instead of reflection-based
`encoding/json`. They support the concrete request and response shapes the
application needs. This reduces temporary allocation and makes maximum sizes
reviewable, at the cost of maintaining serialization code alongside the types.

When adding a field, consider escaping, Unicode, malformed input, duplicate
fields, integer limits and output size. A title containing characters that
need JSON escaping can occupy more wire bytes than its decoded value.
Use host-side reference parsing and fuzzing to test these boundaries.

## Lists stream without holding the service lock over WiFi

The service locks while it selects and encodes one record into borrowed
storage, then unlocks before writing to the network. It checks sequence or
generation information as iteration continues, so a reused ring slot does
not silently become a different record under the response.

This is safe record access, not a promise of a single database snapshot for
the whole list. Concurrent writes can change what remains available during
iteration. The board caps pages at 30 records; desktop configuration can
allow more. Clients should tolerate history leaving the retention window.

## Adding an endpoint

1. Put business behavior and authorization in the service; test its invariants.
2. Add the request type and bounded parser, including malformed/oversized cases.
3. Add the response view/writer and verify the largest escaped record fits.
4. Register the shared API route and the board admission pattern.
5. Check OPTIONS coverage, authentication, error mapping and resource release.
6. Run host tests, compile with TinyGo, then exercise the route on hardware.

Keep hardware access in the board entry point or adapter. Do not let a handler
grow an unbounded slice, hold a service lock across a socket write, or retain
a string backed by a buffer another worker can reuse.

Useful source entry points are the
[API package](https://github.com/matthewdeanmartin/nanacoin/tree/main/nanacoin_go/internal/api)
and [board adapter](https://github.com/matthewdeanmartin/nanacoin/tree/main/nanacoin_go/internal/boardhttp).
