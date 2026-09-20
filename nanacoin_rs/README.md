# NanaCoin Rust

A household scrip API and bundled Angular site targeting ESP32-S3 N16R8.
Open **https://nanacoin.local/** for the single-board UI, with API at `/api/v1`.
Firmware embeds the shared Angular production build as read-only flash assets.
Separate client hosting and the API-only desktop build remain supported. Do not
run the legacy `nanacoin.local` UI board at the same time. The Rust domain uses
typed commands, identity newtypes, checked integer amounts and explicit errors.

## Run locally

From Git Bash:

```bash
cd nanacoin_rs
make run
```

The API listens on `http://127.0.0.1:8080`. Use the Angular client at `http://localhost:4200`; `/` on the API returns JSON 404. A fresh journal is unprovisioned: use the client's household setup and choose a username and PIN/password.

If you ran the earlier Rust prototype, stop that Rust server first, then:

```bash
make migrate-login
make run
```

The interactive migration reads the old credential from ignored `.local/admin-token` (or prompts for it), asks for a username and PIN/password, and appends a migration event. Balances, member identity and history are preserved. It does not delete the journal. Repeat for other legacy accounts with their credentials. This only migrates the earlier Rust prototype, not Go journals.

`NANACOIN_PORT` and `NANACOIN_JOURNAL` override the port and default `nanacoin.journal` path. An exclusive file lock prevents two writers. Desktop transport is loopback HTTP. `NANACOIN_ORIGINS` sets a comma-separated exact CORS allowlist. Defaults include localhost and 127.0.0.1 on port 4200 and HTTP/HTTPS `nanacoin.local` for the separate client board. Add the actual client origin if different.

## Authentication and compatibility

Authentication follows the existing server: salted PBKDF2-HMAC-SHA256 passwords, its 1,000-round work factor and minimum four-byte PIN, S256 PKCE with one-use 60-second codes, and eight-hour RAM-only sessions. Restart clears sessions. Logout, password changes, role changes and disabling a member revoke the relevant sessions. Five failed password attempts lock the username for five minutes. Roles and last-active-Nana protection are enforced in the domain. There are no permanent account access tokens.

Implemented client flows include provisioning/login/logout, status, members and role/status/password updates, balances and recent transactions, issue/retire/transfer/reverse, listings with descriptions and edits, purchase/cancel, and household configuration. Money endpoints use durable `Idempotency-Key` receipts. The typed command endpoint also remains available; credential commands cannot be submitted through it.

Negotiated offers support proposing, accepting, declining, withdrawing and undoing acceptance. Only the listing owner accepts, at the offered price; SELL charges the offerer and BUY charges the listing owner. Proposals need no funds, while acceptance does. Offers are private to the two parties and Nana. Undo is available to either party or Nana until the configured deadline (48 hours by default), creates a reversal even if the payee spent the money, and reopens the listing. Refund, status and reopening commit in one event. Acceptance details survive recent-history eviction, so an unexpired deal can still be undone.

The September 19 parity pass adds USD wallets, a bounded forex quote book, external-currency listing metadata, account/listing reads, balance/ledger privacy, and member/listing timestamps. Recent transactions retain 365 records in a preallocated ring; HTTP ledger pages cap at 100. Old timestamp-less Rust events still report zero. Diagnostic/log streaming and Go journal import remain absent. Loans and interest are not implemented by either current server. See [PARITY.md](PARITY.md) for remaining differences. Direct purchases still pay immediately. Nana's manual reversal requires a retained transaction, permits correction overdrafts, and does not reopen listings; reversing an offer's payment prevents a second refund through unaccept.

Settlement deadlines use server wall time, never browser time or uptime. Firmware starts SNTP after Wi-Fi; optionally set `NANACOIN_NTP_SERVER` at build time for a reachable LAN time server. Until the clock is valid, timed offer mutations return 503 and offers are not advertised as reversible. A clock behind the last durable event also blocks timed mutations. Deadlines and the configuration survive replay; changing the window applies only to future acceptances.

For a code-linked introduction to Rust ownership, memory, HTTP, diagnostics and
the exact NVS key layout, start with [the Rust guide](../docs/rust/index.md).
[LITTLEFS_EPIC.md](LITTLEFS_EPIC.md) is a proposal only; storage remains NVS.

## Build and check

```bash
make check       # format, Clippy, Rust tests and real local HTTP smoke
make help
make certs       # development TLS files; preserves existing keys
make certs-check # offline RSA/key/hostname/usage validation
make firmware   # compile only; requires Wi-Fi settings
```

Firmware targets ESP-IDF v5.5.3, esp-idf-svc 0.52.1 and managed mDNS 1.8.2. The application is Rust over ESP-IDF's C platform libraries. The script uses the installed Espressif Rust toolchain and, on this Windows machine, the SDK under `C:/Espressif`. Firmware output defaults to short path `C:/ncr` to avoid SDK path-length failures; override `CARGO_TARGET_DIR` if needed. Host output normally uses `target`.

```bash
export NANACOIN_WIFI_SSID='Your 2.4GHz Wi-Fi'
export NANACOIN_WIFI_PASSWORD='Your Wi-Fi password'
make firmware
```

Wi-Fi credentials come from `NANACOIN_WIFI_SSID` / `NANACOIN_WIFI_PASSWORD` when
those are exported. Otherwise `build.rs` reads the first gitignored `.env` or
`config.py` that defines `WIFI_SSID` / `WIFI_PASSWORD` (this crate first, then
`nanacoin_web`, then the MicroPython projects), so `make firmware` works with no
environment setup. The build prints which file it used, never the value, and the
environment always wins. Wi-Fi credentials and ignored `certs/nanacoin-ca-signed.crt` /
`certs/nanacoin-ca-signed.key` are embedded during compilation. There is no embedded
household password or admin token. Treat binaries and build caches as secrets.
`make firmware` does not flash, reset, probe or open serial connections.

The board serves `http://nanacoin.local/` and `https://nanacoin.local/` in Easy
mode. mDNS needs multicast reachability. `/trust` guides certificate installation;
`/ca` serves only the public household CA. Nana can require HTTPS for everyone
after preparing all devices. Secure mode leaves HTTP available only for trust
setup and CA download. The build needs OpenSSL, preserves its dedicated
CA, and never installs trust automatically. See [connection security](CONNECTION_SECURITY.md)
for limitations, key handling and explicit USB recovery without economy reset.
Both canonical origins are accepted even when other CORS origins are customized.

Certificate generation uses 100-year RSA certificates for compatibility with
the ESP-IDF server and Firefox/NSS clients. Every firmware build runs
`make certs-check` implicitly and refuses short-lived, EC or mismatched key
material. If the ignored CA files are missing or a
deliberate replacement is required, run `make rotate-certs`; it archives the old
CA and keys under `.local/cert-backups`, generates and validates the replacement,
and requires every client to trust the new CA from `/trust` after deployment.

## Single-board build and deployment

For the copy-paste upgrade runbook, including port discovery, stop conditions,
and required live-board checks, see [DEPLOY.md](DEPLOY.md).

```bash
(cd ../nanacoin_go/angular && npm ci) # one-time dependencies, if needed
make run-bundle                   # local UI + API on http://127.0.0.1:8080
make web-check                    # Rust, HTTP assets and deployment safety tests
make firmware                     # Angular + gzip manifest + ELF + checked .bin
# Only when a board is attached and deployment is intended:
make deploy PORT=COM9
make probe-board ADDRESS=192.168.1.158 # strict post-flash TLS/API/site check
```

`make deploy` builds both halves of the single-board application: the current
shared client from `../nanacoin_go/angular` and the Rust API firmware that
embeds it. Do not run `nanacoin_web/deploy.ps1` as a second step unless you are
deliberately restoring the legacy two-board arrangement.

After flashing, `make probe-board ADDRESS=<board-ip>` verifies the live server
against the generated CA without a certificate bypass, checks hostname and TLS,
reads the API status, confirms the ledger invariant and Angular shell, and
compares the board's downloadable `/ca` byte-for-byte with the build input.

Deployment rebuilds first, reads the board's partition table, and refuses any
layout other than the current Rust layout. It writes **only** the application
at `0x10000`, not the ledger, configuration, bootloader or partition table.
There is no full-chip erase. This is an upgrade script, not a first-install or
TinyGo migration tool. Deployment resets the board and ends sessions; durable
household state remains. Back up valuable state before updates.
`bash scripts/deploy.sh COM9 --dry-run` builds and prints the plan without
opening a serial port. Scripts use esptool 4.x through
`NANACOIN_ESPTOOL_PYTHON`, or the installed Windows ESP-IDF Python environment.

Assets live under ignored `.embuild/web`, with identity and gzip versions.
The packaged index selects same-origin `/api/v1`; standalone Angular source
defaults remain unchanged. An explicitly remembered API choice still wins:
use the connection screen or `?api=` to select this site again. Serving uses
borrowed flash slices and 2 KiB writes, not runtime compression, a filesystem,
or the ledger response buffer. Hashed JS/CSS use immutable caching; index
revalidates and ETags permit 304 responses. Only known Angular routes fall back
to index; missing assets remain 404. Static routes support GET; other methods,
including HEAD, return 405. Range requests are not implemented.

UI updates reflash the application, not the ledger. An offline size gate rejects
images exceeding the unchanged 4 MiB application partition.

## Memory and persistence

Rust does not automatically prevent fragmentation. Application collections have explicit limits and return capacity errors:

| Resource | Limit |
|---|---|
| Members / listing slots / recent transactions | 16 / 48 / 365 |
| Forex quotes | 16; recycle closed/expired quotes, never live quotes |
| Offer slots / offer message and undo reason | 32 / 140 UTF-8 bytes |
| Names / titles / descriptions and memos | 40 / 80 / 96 UTF-8 bytes |
| Request body / reused response buffer | 1 KiB / 512 KiB |
| Sessions / pending login codes | 64 / 16 |
| Journal | 1,024-byte records; automatically checkpoint/retire at 2,048 changes; reads legacy logs up to 4,096 |
| Durable HTTP retry index | 4,096 receipts allocated once; bounded ring, preserved in checkpoints |
| HTTPS + HTTP sockets / handler stacks | 4 + 2 / 24 KiB each; one reusable response buffer per worker |
| Startup stack | 64 KiB |
| Movement amount | 1 through 1,000,000,000 whole coins |

Allocation tests observe zero allocations after startup across 2,999 financial writes with state reads, and across 1,000 forex trades with retries and quote reads. The larger response buffer is allocated once for the 365-record state view and worst-case JSON escaping; it does not grow per request. The history backing storage also allocates once and overwrites oldest records without shifting the full history. TLS, HTTP headers, Wi-Fi and NVS still allocate. PSRAM and PSRAM-backed NVS cache are enabled, with 64 KiB reserved for internal-RAM allocations. Heap metrics include largest free block. Runtime OOM resilience requires hardware measurement.

Offer slots recycle the oldest closed or settled deal, never an open or still-reversible deal. Reversible acceptances also pin their listing slot. Fixed domain state is boxed once at startup so moving the service does not copy the whole state through the firmware stack. Views serialize directly from bounded iterators, without constructing response vectors. A regression test covers a full table, 1,000 offer reads and 1,000 acceptance retries without API/domain allocations after setup.

Wi-Fi/lwIP and the diagnostics sampler use core 0; HTTPS and domain work use core 1. Financial writes have one owner. The service mutex now covers only the domain call: each httpd worker owns a response buffer, so serializing a reply and writing it to the socket happens outside the lock. Four TLS sockets are served concurrently; requests beyond that queue rather than being refused, so slow clients still delay others.

`GET /api/v1/diag` is unauthenticated and reads a small, coherent snapshot published by the core 0 sampler. Its separate mutex covers only copying the snapshot, never the ledger, probes or socket I/O. Memory, temperature, network, clock, tasks/stack, server counters and NVS capacity feed Angular's Nana-only **Machine health** page. `/api/v1/diag/static` adds hardware/build/reset information and the partition map. Both routes use a 4 KiB request-local response buffer and bypass the ledger response buffer. The shared diagnostics object is capped at 288 bytes, with a 4 KiB sampler stack and a startup temperature-driver handle. History stays in the browser. See [API.md](API.md#diagnostics) for field semantics and differences from MicroPython.

mbedTLS content buffers are 4 KiB in / 2 KiB out: request bodies are capped at 1 KiB, and the previous 16 KiB input buffer could not be allocated four times inside the reserved internal RAM.

Validated events append durably before state changes. Failed or ambiguous writes latch the service unavailable until replay. Complete corrupt records fail startup; only an incomplete final desktop frame is truncated. NVS initialization errors do not authorize automatic erasure. The firmware's 8 MiB NVS partition and overall layout differ from Go.

The file and NVS adapters automatically save a durable checkpoint and retire the active log before the next write after 2,048 changes. Nana can close the journal early or reset the entire economy from the Household page. Checkpoints preserve balances, credentials, outstanding deals, retry protection and recent history. Reset removes all household data, revokes sessions and returns to provisioning. Both use two generations and publish the replacement before reclaiming the old data. See [RETENTION.md](RETENTION.md) for recovery, backup files, memory bounds, client retry generations and upgrade compatibility. Archival history export is not implemented. Amounts and IDs stay within JavaScript's exact integer range; money never uses floating point.

USD issuance is Nana-only. Quotes support BID/ASK, integer cents per coin, all-or-nothing takes, expiry and owner/Nana cancellation. A take validates both wallets and records both currency legs in one durable event. Balances and quote status replay together; there is no half-trade state. Ordinary disabled accounts cannot send or receive. Currency listings retain descriptive metadata but do not themselves move USD wallets.

See [API.md](API.md), [PARITY.md](PARITY.md) and [VALIDATION.md](VALIDATION.md).
