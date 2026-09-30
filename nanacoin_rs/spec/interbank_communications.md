# Interbank communications: the two banks and how to reach them

Status: handoff notes, September 28, 2026. Nothing here is an interbank
protocol yet. This is the ground truth the next developer needs before
designing interbank transfers and foreign trade: what each bank is, how to
reach it, what it can and cannot do today, and which constraints any design
must respect. Why there are two banks is in [SECOND_BANK.md](SECOND_BANK.md);
how to build, deploy and probe them is in [../DEPLOY.md](../DEPLOY.md).
The proposed protocol built on these facts is
[INTERBANK_PROTOCOL.md](INTERBANK_PROTOCOL.md).

## The two banks

Each bank is a complete, independent NanaCoin: its own board, household,
members, ledger, journal and **currency**. They run the same Rust application
and the same Angular client. Neither is a replica or client of the other.

| | S3 bank (`s3`) | S2 bank (`s2`) |
|---|---|---|
| Role | the first, established household bank | the second bank, built to test interbank work |
| Hostname | `nanacoin.local` | `nanacoin-s2.local` |
| IP (DHCP, not reserved) | 192.168.1.158 | 192.168.1.157 |
| Wi-Fi | 2.4 GHz, channel 11, same SSID | same network; −61 dBm in the basement |
| Board | ESP32-S3-N16R8: 2 cores, 8 MiB PSRAM, 16 MiB flash | ESP32-S2 Mini: 1 core, 2 MiB PSRAM, 4 MiB flash |
| MAC | `ac:a7:04:2c:2c:04` | `80:65:99:f0:1c:9c` |
| Household | The Martin House, 5 users, 4 transactions (Sept 28) | **not created yet** |
| Currency | the household's configured name | to be chosen at setup; must differ from the S3's |
| Build | default profile | `--features board-s2` (`src/board.rs`) |
| Physical location | beside the build PC | basement, USB-powered |

The single source of truth for board identity (chip, MAC, hostname,
partitions, certificate names) is `scripts/boards.py`. Replacing a board means
editing its `mac` there in a reviewed change.

DHCP addresses can move. Prefer the mDNS names; both boards advertise
`_https._tcp` (port 443, TXT `path=/api/v1/status`) and `_http._tcp`
(port 80, TXT `path=/trust`). Consider DHCP reservations on the router before
depending on IPs in tests.

## Identity: how to know which bank you are talking to

A bank's identity today is **its hostname and certificate**, plus a
self-report. None of these is a cryptographic *bank* identity suitable for
signing money messages (see "What must be designed" below).

| Signal | Where | Notes |
|---|---|---|
| TLS certificate | 443 | RSA-2048 leaf per bank, signed by the one household CA (`certs/home-ca.crt`, RSA-3072, 100 years). The S3 leaf names only `nanacoin.local`, the S2 leaf only `nanacoin-s2.local` (plus `localhost`, 127.0.0.1). `scripts/test-certs.sh` refuses a leaf naming the other bank. |
| Board self-report | `GET /api/v1/diag/static` (public, HTTP or HTTPS) | `board` (`s3`/`s2`), `hostname`, `platform`, chip, cores, flash, partition table. |
| Firmware marker | image bytes | `NANACOIN-BOARD:<id>:<hostname>;`, checked by the deploy tools only. |
| Household config | `GET /api/v1/configuration` (public) | `household_name`, `currency`, `decimals`, `minor_units_per_coin`, `money_epoch`, lending terms. **Display names, not identities**: two households could pick the same name. |

Verified on September 28, 2026: a strict TLS client expecting
`nanacoin-s2.local` refuses the S3, and one expecting `nanacoin.local`
refuses the S2, before any HTTP request.

## Connection details

| Port | Protocol | Purpose |
|---|---|---|
| 443 | HTTPS (TLS 1.2, ECDHE-RSA-AES256-GCM) | Angular app and JSON API under `/api/v1` |
| 80 | HTTP | `/trust` onboarding and CA download; the full app/API only while the household allows HTTP ("Easy mode"). Once HTTPS-only is set, plain HTTP serves onboarding only. |
| 8080 | HTTP | exists **only** after a failed startup: plain-text error report |

Trust: install `certs/home-ca.der` (also served at `/ca` and explained at
`/trust`) once per device or client; it covers both banks.

Browser origins (CORS): each bank allows only its own origins plus the
Angular dev server (`http://localhost:4200`, `http://127.0.0.1:4200`) by
default; `NANACOIN_ORIGINS` at build time can extend this. **A page served
by one bank cannot call the other bank's API** from the browser today. Keep it
that way for money: interbank messages should be server-to-server, not a
browser relaying credentials between banks.

Authentication: members log in with username and password per bank
(`/api/v1/auth/token`, bearer tokens). Tokens and sessions are per bank and
must never be forwarded to the other bank. Mutations take an
`Idempotency-Key` header and a per-member `request_id`; the retry receipts are
bounded (4,096 on the S3, 512 on the S2) and evictable, so they are **not** a
durable interbank replay guard.

## Public, read-only surfaces useful for interbank work

All of these work without signing in:

| Endpoint | Gives |
|---|---|
| `/api/v1/status` | `household`, `provisioned`, `users`, `sequence`, `transactions`, `journal_generation`, `ledger_balanced` |
| `/api/v1/configuration` | currency name, decimals, minor units, money epoch, limits |
| `/api/v1/transactions?limit=N` | the public notebook (ledger is public by design), `circulation` |
| `/api/v1/quotes` | the bank's current foreign-exchange quote book |
| `/api/v1/diag`, `/api/v1/diag/static`, `/api/v1/diag/events` | health, memory, identity, incident history |
| `/api/v1/diag/database` | capacities, journal usage, retention |

Money-moving endpoints (`/api/v1/transfers`, `/api/v1/commands`,
`/api/v1/quotes/...`, admin routes) require a member session.

## What exists for foreign exchange today

`src/forex.rs` is a bounded **in-bank** quote book: members post BID/ASK
quotes pricing the local coin in US-dollar cents (`cents_per_coin`), and one
durable command settles both legs atomically inside that one ledger
(`usd_cents` balances, `usd_issuance_balance`). It does **not** know about
another bank's coin. It is a useful model for price discovery and for
atomic two-currency settlement *within* a ledger, and a natural place to
represent "S2 coin" as a foreign currency later, but it is not interbank
settlement.

Owner decision (September 28, 2026): **each bank issues its own currency;
money between banks is exchanged, not moved.** Holding the other bank's coin
is holding a foreign currency and never counts toward local issuance.

## What the boards can and cannot do today

- **No outbound HTTP.** Neither firmware contains an HTTP(S) *client*;
  both only serve. Server-to-server interbank messages need an outbound TLS
  client (ESP-IDF `esp_http_client`/`esp-tls`, via `esp-idf-svc`), with the
  household CA pinned and the peer's hostname verified.
- **The S2 is small.** Internal RAM is the binding constraint, not PSRAM:
  after Wi-Fi and startup the largest internal block was 31 KiB, and the
  server needed restructuring to fit (HTTP runs on the main task; TLS,
  diagnostics and LED stacks live in PSRAM). After the strict probe: internal
  free about 42 KiB (minimum 39 KiB), PSRAM free 1.33 MiB. An outbound TLS
  client needs its own session buffers (16 KiB in + 4 KiB out on the S2) and
  a stack; measure before adding a task. Any task that writes flash (journal,
  NVS) must keep an internal-RAM stack.
- **The S2 is single-core and serves at most 3 TLS + 2 HTTP clients.** A
  long outbound call made on the main task would pause its web server.
  Prefer short, bounded, nonblocking exchanges and small messages.
- **Bounded everything.** Every store is a fixed-capacity array
  (`src/board.rs`). An interbank inbox, outbox or hold table must be bounded
  too, with explicit behaviour when full. The S2 keeps 300 recent
  transactions in RAM and a 512-record journal before checkpointing.
- **Clocks come from SNTP.** Timed features refuse writes until wall time is
  valid. The two boards' clocks are independent; never order interbank events
  by comparing their timestamps.
- **Durability is per board.** A successful journal append is durable; an
  ambiguous storage error latches a bank read-only until restart. Local
  atomicity does not make two boards atomic.
- **Availability differs.** The S2 has flaky first Wi-Fi association
  (reason 201 then success, ~23 s to ready) and no USB console. Assume either
  bank can be offline for minutes and restart at any time.

## What must be designed before money crosses

Carried over from the Federation section of `../../roadmap.md`, made concrete
for these two boards:

1. **Bank identity keys.** A per-bank signing key (generated on the board,
   private key never leaves it, public key published and pinned by the other
   bank), independent of the TLS leaf. Household and currency *names* are not
   identities.
2. **Currency representation.** How the S3 ledger records "S2 coin" and vice
   versa: a foreign-currency balance per member, the bank's own nostro
   position, and whether/how it is exchangeable for local coin (the quote book
   is the obvious start).
3. **Canonical signed messages** with an explicit schema independent of the
   internal postcard journal format, and a version field.
4. **A settlement protocol**: reserve/hold on the sender, acceptance by the
   receiver, settle or refund, expiry, and reconciliation. A timeout is not
   proof the peer did not act.
5. **Durable replay protection** separate from today's evictable retry
   receipts, so an old signed message can never become a new payment after
   compaction or a checkpoint.
6. **Bounded inbox/outbox and outstanding holds**, surviving restarts, with
   operator visibility (Board Health / Database views).
7. **Reconciliation view**: each bank's claimed position against the other,
   visible to both households.
8. **Authorization**: which member or role may initiate cross-bank payments
   or post cross-currency quotes, and limits.

## Testing without the hardware

The desktop server runs either profile:

```bash
cd nanacoin_rs
NANACOIN_PORT=8080 NANACOIN_JOURNAL=bank-s3.journal cargo run --locked
NANACOIN_PORT=8081 NANACOIN_JOURNAL=bank-s2.journal cargo run --locked --features board-s2
```

Two local instances with separate journals behave as two banks (desktop is
plain HTTP; allow the other instance's origin with `NANACOIN_ORIGINS` only if
a browser test needs it). `make test-s2` runs the Rust suite with the S2
capacity profile; any interbank code must pass both `make test` and
`make test-s2`, and fit the S2 image (currently 2.14 MB of a 2.49 MB
application partition).

## Operational notes for whoever works on this next

- Always name the board: `bash scripts/deploy.sh s3|s2 PORT`,
  `probe-board.py --board s3|s2 --address ...`. The tools refuse the wrong
  bank by chip, MAC, partition layout, image marker and live identity.
- S2 deployments need hands: unplug, hold BOOT, plug in, release (LED dark),
  deploy, then tap RST if the LED stays dark. Its blue LED and
  `http://<ip>:8080/` (startup failures) are its diagnostics.
- Both banks' data is disposable development data (see the repository
  `AGENTS.md`). Even so, resetting the S3's household is not part of interbank
  work unless the owner asks; experiment on the S2 or on desktop instances.
