# NanaCoin on miniframework

Status, October 4, 2026: **phases 0–2 done and tested on the desktop; both
bank images compile; no board has been flashed with them.**
OTA: [miniframework/spec/OTA.md](../../../microcontroller/miniframework/spec/OTA.md).

miniframework (`../../microcontroller/miniframework`) was extracted *from*
NanaCoin's S2 bank, then both kept growing, until they held diverged copies
of the same HTTP framing, connection loop, light policy, static-file
serving, startup sequence and deploy tooling. NanaCoin is now *the ledger
app* on the framework.

## Owner decisions (October 4)

- Framework first, with more unit tests; then the HTTP layer; then the
  board startup code, still for two kinds of board (S3 and S2).
- **No shared UI** beyond the startup-failure page. A framework that ships
  pages becomes a monolith apps configure; each app owns its Angular client.
- **No serialization change.** NanaCoin keeps its hand-written,
  allocation-free JSON; the framework's `message!` formats are optional.
- **Auth stays in the apps** (not in the framework yet).
- Deploy safety checks and live probes **are** framework parts.

## What moved, and where

| Was NanaCoin | Now |
|---|---|
| `src/http_transport.rs` | `miniframework::http` (NanaCoin's copy deleted; the framework's was a superset) |
| `src/bin/esp32/server.rs` (TLS task, mux, response assembly) | `miniframework::esp::Board::serve`, `miniframework::mux`, `Site::respond` |
| `src/bin/desktop.rs` on `tiny_http` | `miniframework::desktop::serve`: the desktop now runs the boards' connection loop |
| `src/web.rs` routing, negotiation, `/trust`, `/ca`, locked-HTTP mode | `miniframework::web::route` + `Site` (`Spa::Routes`, `Service::https_required`, `Config::trust_html`) |
| `src/cache.rs` ETag hashing and 304 | `Reply::revalidate`; `cache.rs` keeps *which* reads are public |
| `/metrics` | framework built-in with `Config::influx_tags` (`bank=s3`) |
| `run()` startup stages, Wi-Fi retry, SNTP, mDNS, `report_failure` on :8080 | `esp::init`, `esp::start`, `Board::partition`, `esp::fail` |
| Transport incidents recorded inside the server | `miniframework::events` observer → `incidents::Recorder::apply` |
| `scripts/deploy.py`, `firmware-image.py`, `asset_probe.py`, `probe-board.py` logic | `tools/boardsafe` (Python); NanaCoin's scripts bind it to `scripts/boards.py` |

The adapter is `src/server.rs` (`Bank`, `config`), shared by the desktop and
both banks.

### Framework changes this needed (each with tests)

1. **NVS is never erased.** `esp::start` used `EspDefaultNvsPartition::take()`,
   which erases a full or newer-IDF partition; now `take_with(false)`, and
   `Board::partition(name)` pre-initializes custom partitions so their
   errors surface instead of an erase.
2. **Two kinds of board.** `BoardConfig::s2()` / `s3()`; with `app_core`
   set, `serve` gives the multiplexer its own task on that core and runs
   `tick` (flash writes) on the calling task. `Limits::large_board()`
   (8 TLS, 4 HTTP, 2 handshakes, 2 MiB budget); `Limits::handshakes`.
3. **HTTPS-only mode** at runtime (`Service::https_required`): plain HTTP
   serves only `/trust`, `/ca`, `/` (as the trust page) and `/metrics`.
4. **Reply hooks**: `Reply::fill` (handlers that write into a slice, no
   per-request zeroing or copy), `Reply::revalidate`, app headers replace
   site defaults, `Request::uri`.
5. **Config**: CORS method/header/expose lists, `ca_filename`,
   `trust_html`, `spa` route list, `influx_tags`; origins capped at 256 bytes.
6. **Startup failure page** on :8080 (`esp::fail`, text from
   `status::failure_report`); app-numbered steps (`SIGNALS.step`, from 8).
7. **Wi-Fi disconnect reason and count** in `/api/v1/sys` and `/metrics`.
8. **Events** (`miniframework::events`) for an app's own incident history.
9. **Partition table** in `/api/v1/sys`, running slot marked.
10. **Stricter static negotiation** (q-values, `identity;q=0`, `*;q=0`) and
    weak ETag lists, ported from NanaCoin.
11. Two bugs found by NanaCoin's smoke tests and fixed in the framework:
    a request refused for its size now carries CORS headers (a browser could
    not read the 413), and the connection drains the unread body before
    closing (closing first reset the connection, sometimes before the client
    read the 413). Slow-request timing now includes handler time.
12. `gzip` and `coredump` are features; NanaCoin uses neither.

### Behaviour visible to clients

Covered in `DEPLOY.md` (October 4 section): startup step numbers, the
`/metrics` tag order and fields, `HEAD` 200, preflight 204,
`/api/v1/sys` and `/api/v1/log` built-ins. The Angular client needed no
change; `smoke.py` and `web-smoke.py` pass against the new server.

### Stays in NanaCoin

Ledger, domain, journal/checkpoint/archive, money and every financial
feature, auth, the API, screen delivery, minicloud, Board Health
(`/api/v1/diag*`, answered outside the ledger lock), and the RGB/Morse light
(it reads the framework's startup and health signals). The journal storage
moves only if a second app needs durable storage like it.

## Not done yet

- **Flash and verify on hardware.** App-only upgrade for each bank (no
  partition change): `make deploy BOARD=s3|s2`, then `make probe-board`.
  Watch internal-heap minimum in `/api/v1/sys` against the old firmware; the
  S3 tick moved from a 24 KiB task on core 1 to the 64 KiB main task on
  core 0.
- A static-file request now allocates a small header list; NanaCoin's
  zero-allocation test for static routes was removed with its router.
- OTA, per the OTA spec: the S3 bank only (the S2 has no room for two slots).
