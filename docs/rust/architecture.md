# Domain, HTTP and concurrency

## Follow a financial write

The HTTP adapter accepts a bounded body, applies origin rules, and passes
method, path, credentials, retry key, bytes and listener TLS state to `api::handle_keyed_on`.
Authentication and business authorization happen in the API/service, not just
in Angular. A Nana-only route in the browser is not an API security boundary.

The service validates a typed command, constructs one event, encodes it into
a fixed frame, persists it, and then applies it to RAM. From
[Service::commit](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/journal.rs):

```rust
if self.journal.append(self.records, &frame).is_err() {
    self.storage_failed = true;
    return Err(Error::Storage);
}
self.state.apply(&event);
```

`Result<T, Error>` makes failure an explicit return value. Elsewhere `?`
propagates an error to the caller; it is not an exception handler. A failed or
ambiguous persistence operation latches storage unavailable until restart and
replay. The application must not proceed assuming a possibly committed event
never happened.

A transfer carries both debit and credit in one event. A forex take includes
both currencies' effects in one operation. It does not independently persist
one user's balance and then hope the second write succeeds. Integer money,
capacity checks, permissions and domain invariants live in
[domain.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/domain.rs),
[offers.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/offers.rs)
and [forex.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/forex.rs).

## Shared ownership versus exclusive mutation

The board wraps the service as follows in
[esp32.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/bin/esp32.rs):

```rust
let shared = Arc::new(Mutex::new(service));
```

`Arc` shares ownership using a reference count; it does not itself make
mutation safe. `Mutex` supplies exclusive access. The handler takes the
service guard around `api::handle_keyed_on`, including its domain response
encoding. Socket output happens after that guard leaves scope. Thus a slow
client does not hold the ledger lock while consuming bytes, but an expensive
checkpoint still blocks other domain calls.

HTTP and HTTPS/domain handling is configured for core 1. Wi-Fi/lwIP and the dedicated
diagnostic sampler use core 0. This does not mean two threads independently
modify the ledger: there is one serialized service. Nor does using both cores
eliminate contention inside platform libraries or flash operations.

## Separate diagnostics lock

The sampler gathers readings outside its own mutex, then publishes a small
copy. The handler copies that snapshot and encodes after unlocking. It never
needs the service lock for its readings. The HTTP listener separately checks
the household transport policy under the service lock before routing. HTTPS
diagnostics skip that check. This lets HTTPS diagnostics remain accessible during domain
work, subject to the network/RTOS still making progress. See
[diagnostics](diagnostics.md) for code and limits.

## Browser, authentication and retries

The shared Angular client discovers an API separately from the static page
host. Login uses username/password authorization followed by PKCE code exchange
and bearer sessions. Mutations use an `Idempotency-Key` with a generation prefix;
retries reuse the same key. Rust records command fingerprints and durable
bounded receipts to reject mismatched reuse and return retained results.

The server exposes its current generation in status/responses. After retirement,
an unknown old-generation key is rejected rather than interpreted as a new
payment. The Angular interceptor tracks generation, but does not re-key an
uncertain previous operation automatically. See
[client service](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_ui/src/app/api/nanacoin.service.ts)
and [API contract](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/API.md).

Timed offer actions require a valid server wall clock. Uptime is not an expiry
clock across reboot. Private storage serialization includes credential material
needed for recovery; public JSON views must not expose it.
