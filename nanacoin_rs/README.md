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

Storage uses the current NCR2/NCS2/NCA2 binary schema. Earlier development data must be reset and re-entered; there is no data migration or old-format decoder. See [the storage contract](spec/STORAGE_V2.md).

`NANACOIN_PORT` and `NANACOIN_JOURNAL` override the port and default `nanacoin.journal` path. An exclusive file lock prevents two writers. Desktop transport is loopback HTTP. `NANACOIN_ORIGINS` sets a comma-separated exact CORS allowlist. Defaults include localhost and 127.0.0.1 on port 4200 and HTTP/HTTPS `nanacoin.local` for the separate client board. Add the actual client origin if different.

## Authentication and client APIs

Authentication follows the existing server: salted PBKDF2-HMAC-SHA256 passwords, its 1,000-round work factor and minimum four-byte PIN, S256 PKCE with one-use 60-second codes, and eight-hour RAM-only sessions. Restart clears sessions. Logout, password changes, role changes and disabling a member revoke the relevant sessions. Five failed password attempts lock the username for five minutes. Roles and last-active-Nana protection are enforced in the domain. There are no permanent account access tokens.

Implemented client flows include provisioning/login/logout, status, members and role/status/password updates, balances and recent transactions, issue/retire/transfer/reverse, listings with descriptions and edits, purchase/cancel, and household configuration. Money endpoints use durable `Idempotency-Key` receipts. The typed command endpoint also remains available; credential commands cannot be submitted through it.

Negotiated offers support proposing, accepting, declining, withdrawing and undoing acceptance. Only the listing owner accepts, at the offered price; SELL charges the offerer and BUY charges the listing owner. Proposals need no funds, while acceptance does. Offers are private to the two parties and Nana. Undo is available to either party or Nana until the configured deadline (48 hours by default), creates a reversal even if the payee spent the money, and reopens the listing. Refund, status and reopening commit in one event. Acceptance details survive recent-history eviction, so an unexpired deal can still be undone.

The September 19 parity pass adds USD wallets, a bounded forex quote book, external-currency listing metadata, account/listing reads, balance/ledger privacy, and member/listing timestamps. The recent cache retains up to 3,000 transactions, backed by a finite committed archive; cursor pages cap at 100 rows and eight archive pages scanned. Raw `/state` omits history. Diagnostic/log streaming and Go journal import remain absent. The Rust server supports loans, scheduled interest and lotto. See [PARITY.md](PARITY.md) for remaining differences. Direct purchases still pay immediately. Nana's manual reversal requires a cached or retained fulfillment original, permits correction overdrafts, and does not reopen listings; reversing an offer's payment prevents a second refund through unaccept.

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
(cd ../nanacoin_ui && npm ci)       # one-time dependencies, if needed
make run-bundle                   # local UI + API on http://127.0.0.1:8080
make web-check                    # Rust, HTTP assets and deployment safety tests
make firmware                     # Angular + gzip manifest + ELF + checked .bin
# Only when a board is attached and deployment is intended:
make deploy PORT=COM9
make probe-board ADDRESS=192.168.1.158 # strict post-flash TLS/API/site check
```

`make deploy` builds both halves of the single-board application: the current
shared client from `../nanacoin_ui` and the Rust API firmware that
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
TinyGo migration tool. Deployment resets the board and ends sessions; the ledger bytes remain. This binary-schema change requires a separate, scoped
reset of disposable development ledger data; flashing alone does not migrate it.
`bash scripts/deploy.sh COM9 --dry-run` builds and prints the plan without
opening a serial port. Scripts use esptool 4.x through
`NANACOIN_ESPTOOL_PYTHON`, or the installed Windows ESP-IDF Python environment.

Assets live under ignored `.embuild/web`, with identity and gzip versions.
The packaged index selects same-origin `/api/v1`; standalone Angular source
defaults remain unchanged. An explicitly remembered API choice still wins:
use the connection screen or `?api=` to select this site again. Serving uses
borrowed flash slices and bounded nonblocking writes, not runtime compression, a filesystem,
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
| Members / listing slots / recent transactions | 16 / 48 / 3,000 |
| Forex quotes | 16; recycle closed/expired quotes, never live quotes |
| Offer slots / offer message and undo reason | 32 / 140 UTF-8 bytes |
| Names / titles / descriptions and memos | 40 / 80 / 96 UTF-8 bytes |
| Request body / reused response buffer | 1 KiB / 512 KiB |
| Sessions / pending login codes | 64 / 16 |
| Journal | NCR2 frames up to 1,024 bytes; NVS writes used bytes only |
| Automatic rotation | Before next event at 2,048 journal records or 1,024 pending transactions/audits |
| Checkpoint rows / archive slots | NCS2 rows up to 4 KiB; 1,024 NCA2 slots up to 4 KiB, 768 retained and 256 staging |
| Audit cache / corrections / currency epochs | 1,024 / 4,096 / 32, preallocated |
| Durable HTTP retry index | 4,096 receipts allocated once; bounded ring, preserved in checkpoints |
| Established HTTPS + HTTP sockets | 8 + 4; idle sessions expire after 60 seconds |
| TLS handshake / HTTP task stacks | 24 KiB on core 0 / 32 KiB on core 1 |
| Pending handshakes / completed handoffs | 2 / 2; handshakes expire after 4 seconds |
| Request headers / body | 4 KiB (32 headers) / 1 KiB; partial requests expire after 5 seconds |
| Pending response memory | Dispatch pauses at 2 MiB; at most one additional 512 KiB reply plus headers |
| Startup stack | 64 KiB |
| Movement amount | 1 through 1,000,000,000 whole coins |

Allocation tests observe zero allocations after startup across 2,999 financial writes with state reads, and across 1,000 forex trades with retries and quote reads. The response buffer is allocated once for bounded API views and JSON escaping; `/state` omits history, and history reads use cursor pages. The history backing storage also allocates once and overwrites oldest records without shifting the full history. TLS, HTTP headers, Wi-Fi and NVS still allocate. PSRAM and PSRAM-backed NVS cache are enabled, with 64 KiB reserved for internal-RAM allocations. Heap metrics include largest free block. Runtime OOM resilience requires hardware measurement.

Offer slots recycle the oldest closed or settled deal, never an open or still-reversible deal. Reversible acceptances also pin their listing slot. Fixed domain state is boxed once at startup so moving the service does not copy the whole state through the firmware stack. Views serialize directly from bounded iterators, without constructing response vectors. A regression test covers a full table, 1,000 offer reads and 1,000 acceptance retries without API/domain allocations after setup.

Wi-Fi/lwIP, diagnostics and a bounded asynchronous TLS handshake task use core 0. One nonblocking connection loop serves established HTTP/HTTPS clients on core 1, using one reusable 512 KiB serialization buffer. The service mutex covers the API call including JSON encoding; socket I/O and handshakes happen outside it. Pending API responses own only their encoded bytes, with a global memory budget; large static bodies remain borrowed flash slices. Each client gets one bounded I/O turn, so partial requests and slow readers yield to others. Financial writes remain serialized and durable. Persistent HTTP/1.1, TCP_NODELAY, TLS session tickets, disabled Wi-Fi modem sleep and 240 MHz operation avoid repeated setup and packet delays. New handshakes do not occupy the serving task.

`GET /api/v1/diag` is unauthenticated and reads a small, coherent snapshot published by the core 0 sampler. Its separate mutex covers only copying the snapshot, never the ledger, probes or socket I/O. Memory, temperature, network, clock, tasks/stack, server counters and NVS capacity feed Angular's Nana-only **Machine health** page. `/api/v1/diag/static` adds hardware/build/reset information and the partition map. Both serialize using the serving task's reusable buffer without taking the ledger lock. The shared diagnostics object is capped at 288 bytes, with a 4 KiB sampler stack and a startup temperature-driver handle. History stays in the browser. API responses expose `Server-Timing` for application, lock and total handler time; these exclude handshake, network and response transmission. See [API.md](API.md#diagnostics) for field semantics and differences from MicroPython.

mbedTLS content buffers are 16 KiB in/out and allocated from PSRAM, preserving internal RAM for networking and stacks. HTTP request bodies remain capped at 1 KiB regardless of TLS record size.

Validated events append durably before state changes. Failed or ambiguous writes latch the service unavailable until replay. Complete corrupt records fail startup; only an incomplete final desktop frame is truncated. NVS initialization errors do not authorize automatic erasure. The firmware's 8 MiB NVS partition and overall layout differ from Go.

File and NVS adapters stage immutable archive pages and a replacement checkpoint before publishing the new head. Only the committed archive interval is visible; unpublished pages cannot skip event replay. Checkpoints preserve balances, credentials, obligations, correction annotations, exact per-epoch counters and retry protection. The committed transaction floor trims cached originals, and pending commands are revalidated after automatic rotation. Nana can checkpoint early or reset from Household. Reset publishes empty state with a new incarnation, revokes sessions and returns to provisioning. See [retention](spec/RETENTION.md) and [storage v2](spec/STORAGE_V2.md).

Historical transactions keep original units and currency epochs. Transaction responses also expose exact current-unit projections, or null when conversion would round or overflow. Exact lifetime totals remain available after archive pruning. Partial refunds, gift requests and digital-art ownership/sales are implemented as APIs; see [commerce](spec/COMMERCE_API.md). No new commerce screens are included. Money arithmetic remains integer-only; lifetime counters are transmitted as decimal strings.

USD issuance is Nana-only. Quotes support BID/ASK, integer cents per coin, all-or-nothing takes, expiry and owner/Nana cancellation. A take validates both wallets and records both currency legs in one durable event. Balances and quote status replay together; there is no half-trade state. Ordinary disabled accounts cannot send or receive. Currency listings retain descriptive metadata but do not themselves move USD wallets.

See [API.md](API.md), [PARITY.md](PARITY.md) and [VALIDATION.md](VALIDATION.md).

## Lotto

The Angular **Lotto** page uses the Rust-only `/api/v1/lottos` endpoints. Nana
creates a draw with a title, ticket price, sales closing time, kind (`SIMPLE`,
`DELAYED`, `SAVINGS`) and fixed 30-day `rate_bps` (0–10,000; SIMPLE requires 0).
Members buy a positive integer `count` at `POST /api/v1/lottos/{id}/tickets`.
POSTs require the usual durable idempotency key. Nana cannot enter her own draw.
Each ticket has equal odds, including multiple tickets held by the same member.

- **Simple:** draw at sales close; the winner receives the full ticket pool.
- **Delayed:** draw 30 days after sales close; the winner receives the pool and
  one fixed month's simple interest.
- **Savings:** draw 30 days after sales close; every buyer receives their full
  principal back, and one winner receives the whole pool's interest.

One month means exactly 30 days. Interest starts at sales close, rounds down
once to minor units, and does not grow further if the server is offline.
Purchases are final. Ticket funds live in a separate escrow account, outside
Nana's spendable balance. Interest uses the creating Nana's available coins
first, then issues only the shortfall. Disabled accounts still receive owed
settlements. Changes to the house's role do not change its existing obligations.
No house fee is deducted. Zero-ticket draws finish without a winner or payment.

The existing bounded background scheduler journals the random winning ticket
before paying. OS randomness uses rejection sampling to avoid modulo bias.
Each principal/interest leg is a separate durable occurrence, with a persisted
settlement cursor; replay and checkpoints resume without redrawing or double
payment. Ledger entries carry `lotto-` references and cannot be reversed outside
the settlement model. Interest is classified as INTEREST, including issuance;
principal is OTHER and does not count as production. A bad clock or accounting
limit pauses settlement. No timer per draw or logged-in user is required.

There are 16 retained draws; only completed draws may be recycled. Pool plus
interest is capped at the existing per-transfer limit. Currency reforms rescale
lotto terms and balances exactly or reject the reform if rounding is required.
The browser-only demo displays an explicit notice; lotto runs on the Rust server.
The Go app is unchanged. Development journals/checkpoints from before this schema
may be reset; no old-schema compatibility is provided.

## Bank mail

A transfer of zero NC with a nonblank memo is a message. It uses the ordinary
transaction journal, idempotency receipts, checkpoints, and retained history;
no separate messaging database or financial postings are introduced. Sender and
recipient are the ordered zero-value transaction legs. The wire kind is MESSAGE.
Messages cannot be reversed and do not change balances, supply, or loan credit
eligibility. Negative transfers and blank messages are rejected.

Messages appear in the participants' account transaction histories, and are
excluded from the public notebook and public transaction reads. The raw state response does not embed transaction history. Messages share the bounded history
window with payments. Mail combines retained transactions, offers and loan
requests; its New markers are local to the browser and account. Mastodon is an
optional copy sent after the NanaCoin record succeeds, never a prerequisite for
local delivery. A failed copy does not undo or repeat the bank record.

Offer records preserve their listing direction and title so retained proposals
remain understandable after the listing is recycled. The UI groups incoming
buy/sell offers and outgoing buy/sell offers and names both parties explicitly.


Physical fulfillment is tracked separately from payment: accepted offers and
purchases create TODOs, providers claim work done or goods/cash delivered, and
recipients can dispute or withdraw a dispute. My Account shows outstanding work,
waiting deliveries and recent fulfillment activity; Messages and the public
ledger show the same status. Nana can review disputes and reverse the payment.
See [the fulfillment API](API.md#physical-fulfillment-current-development-schema)
for authorization, retention and durability details.
