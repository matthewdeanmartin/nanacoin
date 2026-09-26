# Push updates: WebSockets analysis

Status: analysis only, nothing implemented. Written September 26, 2026.

## Problem

A balance changed by someone else (for example Nana reversing a purchase) used
to stay stale on other screens until F5. The Angular client now polls: every
visible tab asks `GET /api/v1/status` every 5 seconds, again on focus,
visibility and route change, and reloads everything only when the journal
`sequence` moved (`Session.watch`, `checkForChanges`, `reloadOnLedgerChange` in
`nanacoin_ui/src/app/api/session.ts`). Changes appear within 5 seconds.

The question is whether a push channel (WebSockets) is worth building to make
that immediate.

## Current server shape

The board does not use ESP-IDF's `esp_http_server`, which has WebSocket
support. `src/bin/esp32/server.rs` is a hand-written nonblocking HTTP/1.1 loop:

- Core 0 performs TLS handshakes (`HANDSHAKES = 2`); core 1 multiplexes all
  established connections, one bounded I/O turn per client per 1 ms tick.
- `TLS_CLIENTS = 8`, `HTTP_CLIENTS = 4`, for the whole household. Idle sessions
  expire after 60 seconds (`IDLE`). When a pool is full, `add_client` evicts
  the longest-idle session of the same transport.
- `CONFIG_LWIP_MAX_SOCKETS=24` is sized exactly to those pools.
- Ledger writes happen in `respond()` on the core-1 loop and in the scheduler
  thread in `src/bin/esp32.rs` (`service.tick()` for loans and lottos).
- The desktop server (`src/bin/desktop.rs`, tiny_http) handles requests one
  at a time on a single thread.

Any push mechanism has to be added to this loop by hand.

## What any push channel needs

1. **A connection that never goes idle and is never evicted.** This is the
   real cost. With 8 TLS slots, four tablets with two tabs each would take
   every slot and ordinary API calls would be refused. Push connections need
   their **own small pool** (for example `STREAM_CLIENTS = 6`), accounted
   separately from request slots. When it is full, evict the oldest stream;
   that tab falls back to polling. Raise `CONFIG_LWIP_MAX_SOCKETS` and
   `CONFIG_LWIP_MAX_ACTIVE_TCP` to about 32. Memory is fine: a TLS session is
   roughly 2 x 16 KiB mbedTLS buffers in PSRAM. A stream client needs only a
   tiny input buffer, not `INPUT_LIMIT`.
2. **A change signal.** An `AtomicU64` in `Context`, stored after every API
   call in `respond()` and after every scheduler `tick()`. Stream clients
   compare it on their turn and write only when it moved.
3. **Heartbeats** every 20-30 seconds, so neither the 60-second idle timer nor
   home routers drop the connection.
4. **No authentication.** Only the journal `sequence` is pushed, which is
   already public in `GET /status`. The client reacts by calling its normal
   authenticated `refresh()`. This avoids the usual problem that browsers
   cannot set an `Authorization` header on a WebSocket or `EventSource`.
5. **Close when hidden.** A tablet left in a drawer must not hold a scarce
   slot. The client closes the stream on `visibilitychange` to hidden and
   reopens it, with an immediate status check, on visible.
6. **Desktop:** a thread per stream and the service behind an `Arc<Mutex>`,
   or one stream blocks every request.
7. **Demo backend** (in-browser) keeps polling.

## Server-Sent Events instead of WebSockets

| | SSE (`GET /api/v1/events`) | WebSocket |
|---|---|---|
| Handshake | Ordinary HTTP response that never completes | `101` upgrade; needs SHA-1 (only `sha2` is a dependency today) |
| Framing | Write `data: {"sequence":42}\n\n` | Frame encoder, plus a decoder for masked client frames |
| Reads | Only to detect close | Must handle ping, pong, close |
| Browser | `EventSource`, automatic reconnect | Hand-written reconnect and backoff |
| Fit with current loop | A "streaming" client state that only writes | A second protocol state machine |

WebSockets' extra capability is client-to-server messages. NanaCoin must not
use it: money moves only through REST calls with idempotency keys and durable
journal writes. The extra parser and failure modes buy nothing here.

## Benefit versus cost

- Today: a change shows up within 5 seconds, or at once on focus or page
  change. Each visible tab makes one small `/status` request every 5 seconds
  over an already established TLS session. Hidden tabs make none.
- With SSE: under a second, and most polling disappears. Keep polling as a
  fallback at about 30 seconds while a stream is open; drop back to 5 seconds
  when it closes or is refused.

The gain is real but modest for a household app. The risk sits on the board:
long-lived connections compete for the scarcest resource, and that behaviour
can only really be tested on hardware.

## Estimated work

- Board: streaming client state, separate stream pool and eviction, atomic
  sequence plus scheduler hook, socket limits. About 150-200 lines.
- Desktop: thread per stream and shared service. About 50 lines.
- Client: `EventSource` tied to visibility, slower polling while connected.
  About 60 lines in `session.ts`.
- Tests and docs: `API.md`, a load check in `scripts/transport-board.py`
  (several streams open plus normal traffic, and API calls must still get a
  slot), and the README capacity table.

## Recommendation

Keep 5-second polling unless the delay actually bothers the household. If push
is wanted, build SSE with a separate stream pool, and measure slot pressure on
the board before shipping.
