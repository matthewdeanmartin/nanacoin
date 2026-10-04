# NanaCoin

**TinyGo firmware is frozen** pending supported upstream board/PSRAM support;
see [PSRAM_EPIC.md](PSRAM_EPIC.md). Machine-diagnostics compatibility is the
explicit exception. The shared Angular client in `../nanacoin_ui` remains active
for the Rust API.
The [Rust guide](../docs/rust/index.md) explains the active implementation.

A household currency and marketplace, administered by a trusted household
authority called Nana. Runs on a laptop and on an ESP32-S3.

NanaCoin is **not cryptocurrency**. No blockchain, no mining, no consensus, no
wallets, no distributed anything. One small server applies double-entry
transactions and answers questions about them.

This is a development application, not a production service. Do not restore
old firmware or reset a household just to leave a test board responsive before
the next build. See [development instructions](AGENTS.md).

RAM history now uses **365 preallocated transaction slots**, overwriting the
oldest when full while preserving lifetime balances and IDs. The 12 KiB shared
text buffer can evict history earlier for long memos. The event/error log is
also a ring (58 entries). [RAM history and future flash batches](RAM_HISTORY.md)
describes retention and the conservative 100,000-cycle flash design budget.
No flash persistence is enabled by this change.

The board now has four request workers and eight TCP slots. List responses
copy one record before releasing the service lock for network writes, and
overlapping idempotent retries are coordinated. See [concurrency](CONCURRENCY.md)
for RAM costs and live-list consistency. Browser behavior is unchanged.

For the complete tutorial, see [TinyGo and NanaCoin](../docs/tinygo/index.md):
setup, memory, the web framework, domain rules, storage, concurrency and testing.

## What it does

Nana issues coins, adds household members, and fixes mistakes. Everyone else
sees their balance and history, sends coins to each other, and posts things
for sale — an hour of Switch time, a LEGO set, doing the dishes, or five
dollars of actual cash.

Money is an **append-only ledger** of double-entry transactions, not a balance
field that gets overwritten. A balance is the sum of the postings touching an
account. Corrections append a mirror transaction instead of editing postings;
older history eventually leaves the RAM window. The retained audit trail shows
("Alice paid Bob 10 / Nana reversed it because Bob didn't mow the lawn")
rather than silently rewriting the original transaction.

## Status

| Piece | State |
|---|---|
| Ledger, marketplace, auth, HTTP API | Implemented, with Go unit and regression tests |
| Desktop server (`go`) | Done, file-backed journal, crash-safe |
| Angular client ([`../nanacoin_ui/`](../nanacoin_ui/)) | Implemented — **the one to use** |
| Vanilla client (`web/`) | Done, small enough to serve off the board |
| Board firmware (`tinygo`, ESP32-S3) | Runs on hardware, WiFi and all |
| Board HTTP | httphi with bounded worker buffers and reusable adapter objects; some allocations remain |
| Board uptime | Workload-dependent; use diagnostic load tests to establish current limits |
| Board persistence | **In-memory only — the board forgets on reboot** |
| Server logs | `/api/v1/logs` and a Logs page, for diagnosing the board |
| Loans, interest, idle-money tax | Not started, see Roadmap |

The board build is a working demonstration, not yet a household ledger. Two
things still need work: persistent board storage and endurance under realistic
load. The board transport already uses httphi rather than the `net/http` server. See
[`SHOPPING_LIST.md`](SHOPPING_LIST.md) for why neither is a purchase.

## Run it locally

Two processes: the API, and the Angular site.

```powershell
# terminal 1 - the API
go run ./cmd/nanacoin -web ""

# terminal 2 - the site
cd ../nanacoin_ui
npm install     # once
npm start
```

Then open <http://localhost:4200>. The first screen asks you to create the
household and become Nana. `npm start` proxies `/api` to port 8080, so the two
look same-origin and CORS never comes up in development.

There is also a smaller vanilla-TypeScript client, which the Go server can
serve itself for a single-process setup:

```powershell
cd web && npm install && npm run build
cd .. && go run ./cmd/nanacoin        # http://localhost:8080
```

Useful flags:

```powershell
go run ./cmd/nanacoin -addr :8080 -journal nanacoin.journal -web web

  -journal ""          in-memory, nothing is saved
  -capacity 1048576    bound the journal, to rehearse a full one locally
  -origins https://...  comma-separated allowed CORS origins
```

## Run it on the board

The current tested toolchain is [TinyGo](https://tinygo.org) 0.42.0 with Go
1.26.5, plus a patched
espradio — see [`patches/README.md`](patches/README.md) for what the patch does
and why it is unavoidable.

```powershell
.\patches\apply.ps1                  # once, clones and patches espradio
.\deploy.ps1 -Ssid "YourWiFi" -Password "YourPassword"
```

`deploy.ps1` builds, flashes over the CH343 bridge port, and tails the serial
console until the board reports its address.

Then open the Angular site and give it that address. If the site cannot reach a
NanaCoin it asks for one: type `192.168.1.158`, press Connect, and it is
remembered. `?api=192.168.1.158` in the URL does the same thing in one step.

The TinyGo API's base URL is **`http://nanacoin-api.local`** on the same LAN.
After DHCP, `internal/boardmdns` registers lneto's mDNS responder and advertises
HTTP on port 80. Enter `nanacoin-api.local` in the Angular client's Connect
screen, or use `?api=nanacoin-api.local`. The separate MicroPython static web
board keeps `nanacoin.local`, so the two names do not collide.
The API can be checked at `http://nanacoin-api.local/api/v1/status`; its root
path `/` does not serve the website. With both boards, open
`http://nanacoin.local/?api=nanacoin-api.local`.

The printed DHCP address remains a fallback on clients or networks that block
mDNS. This is IPv4 discovery; the current responder does not automatically
rename itself if a second TinyGo NanaCoin board claims the same name. It answers
queries rather than sending unsolicited startup announcements. The address is
set at boot after DHCP; any future live DHCP address-change support must also
refresh the responder. A Wi-Fi reassociation currently retains the stack address.

Verified on the ESP32-S3 on 2026-09-19: Windows name resolution and HTTP 200
from `/api/v1/status`, plus direct multicast A, PTR and SRV replies. The host
tests in `internal/boardmdns` exercise Ethernet/IP/UDP discovery, record contents,
repeated queries, different assigned addresses and mDNS's IP TTL of 255.

Two things about this board, both learned the hard way and both recorded in
`../BOARD_SKILL_ESP32_S3_N16R8.md`:

- **Flash at offset `0x0`**, not `0x1000`. The S3 bootloader lives at zero.
  Flashing at `0x1000` reports success and leaves a board that never boots.
- **Serial output appears on the native USB port, not the bridge.** Flash over
  the CH343 bridge (stable, usually COM8); read `println` output from the
  native USB port, which re-enumerates on every reboot.

## How it is put together

```text
cmd/nanacoin/          desktop server - flags, signals, file journal
cmd/nanacoin-esp32/    board firmware - WiFi bring-up, serial console

internal/
    ledger/            transactions, postings, balances, reversals
    core/              the application layer: journal-then-apply, authorization
    api/               HTTP handlers, CORS, views
    auth/              password verifiers, PKCE, sessions, rate limiting
    users/             household identities and accounts
    marketplace/       listings
    storage/           the persistence contract - one interface, three impls
        memory/        RAM, for tests
        flashlog/      append-only file, for desktop and as the flash template

../nanacoin_ui/         the household site: Angular 22, signals, lazy routes
web/                   the earlier vanilla client, small enough for the board
patches/               the espradio retry patches and a script to apply them
```

Three rules shape all of it.

**The server is authoritative.** The browser never decides whether a
transaction is valid, what a balance is, or who may do what. It asks and
renders the answer. Every authorization check is server-side.

**Journal first, then apply.** Domain mutations append through the journal
contract before applying the in-memory change. Durability depends on the backend:
the desktop file retains records; the board currently discards them. The reverse order would let a flash failure
leave a balance in RAM that no record supports, and the next reboot would
quietly undo a transfer the user was told had succeeded.

**Application logic never touches hardware.** `internal/storage` is a
four-method interface — append, replay, size, close. Nothing in `ledger`, `core` or
`api` knows whether it is writing to a file, a flash partition or a test
double. That is what lets the same economic code compile for x86-64 and Xtensa.

## Diagnosing it

The board has no screen, and a browser is a poor witness: it reports a missing
CORS header for a route that does not exist, a network error for a request that
was answered, and nothing at all for a request that never left. Every one of
those has cost real time on this project.

So the server records its own decisions and serves them back:

```text
GET /api/v1/logs
```

and the client has a **Logs** tab showing them. Each entry is a sequence
number, a level, a status or decision name, and a path:

```text
44  warn   401           POST /api/v1/auth/authorize
38  info   cors-allow    http://localhost:4200
37  info   preflight     OPTIONS /api/v1/logs
35  info   200           GET /api/v1/status
```

The two facts that make it useful:

- **A recorded outcome confirms the request reached the logging layer.** An
  absent entry may have been overwritten, failed earlier or not yet completed.
- **A refused origin is named** (`cors-refuse`), because that is the case the
  browser reports most misleadingly.

"Problems only" hides the routine 200s. "Follow" polls while you reproduce the
fault. "Copy" gives you the text for a bug report.

The log is readable **without a token, and without a working session** — the
failure most worth diagnosing is the one that stops you logging in, so the
connect screen links to it too. It carries paths, statuses and origins; no
tokens, no passwords, no balances.

It is a fixed ring of 58 entries allocated once, so a board serving for a
month uses the same memory as one that just booted. `total` keeps counting past
that, so you can tell when older events were dropped.

## Memory and storage

The current ledger uses packed records, inline postings, derived transaction
IDs, interned identities and a reusable text arena. It retains up to 365
transactions while preserving lifetime balances; long text can reduce that
window. The HTTP stack and domain stores reserve bounded working space at
startup, but the complete request path still has allocations.

See the [memory guide](../docs/tinygo/memory.md) for current capacities and
[storage and retries](../docs/tinygo/storage.md) for commit and retry semantics.
Earlier measurements are preserved in [hardware history](HARDWARE_HISTORY.md);
they describe earlier implementations, not current memory budgets.

## Testing at scale without flashing

`Service.Seed` generates a plausible household history - members, listings,
transfers, purchases and reversals - so the app can be exercised against
hundreds of transactions on a laptop:

```powershell
go test ./internal/core/ -run TestSeed -v
```

Desktop tests exercise wraparound, capacity failures and journal errors without
flashing hardware. The current board does not persist runtime writes to flash.
Future persistence must distinguish programming from sector erasing and batch
writes within the documented conservative wear budget.

## The parts worth explaining

### Crash safety

A power failure can land between any two bytes. Each journal record is framed
`header | payload | crc | commit marker`, with the marker written **after** the
rest has been flushed. Replay scans forward, validates magic, length, CRC,
commit marker and sequence, and stops at the first record that fails any of
them — discarding the torn tail.

So a half-written transaction can never become money. `flashlog`'s tests
truncate the journal at *every* byte offset of the last record and assert that
the committed prefix survives intact and the partial record vanishes, every
time.

### Idempotency

Money-moving endpoints accept an `Idempotency-Key` so a retry can return a
recorded result instead of repeating an operation. The cache is bounded to
16 entries and 4 KiB of response bodies, with FIFO eviction. Concurrent matching
retries are coordinated. Protection ends when the receipt is evicted or the
board restarts. The economic commit and receipt recording are separate steps;
this is not a durable exactly-once guarantee. See the
[retry-cache explanation](../docs/tinygo/storage.md#the-retry-cache).

Failures are deliberately not memoised: a transfer refused for insufficient
funds should be free to succeed once the user has been paid.

### The issuance account

Coins are created from a distinguished `account:system-issuance`, which is the
one account allowed an arbitrarily negative balance. Issuance is therefore an
ordinary balanced transaction rather than an exception to the rules, and
"every coin in circulation is accounted for" becomes a property of the whole
ledger — checked on every boot by `CheckInvariants`.

### Authentication

Authorization Code + PKCE with S256 only (`plain` is refused). Access tokens
are opaque random strings, not JWTs; only their hashes are stored. Sessions
live in RAM and die with the board, which is the right trade on flash: a
household logging in again after a power cut costs less than a flash write per
login.

Passwords are PBKDF2-SHA256, with the work factor chosen for the *weakest*
target — an ESP32 doing this on every login has to stay responsive. Argon2 and
scrypt want memory the board would rather spend on the ledger. Failed logins
are rate-limited before the hash runs, so a locked account costs no CPU.

An attacker with the flash in hand is outside the threat model.

## Hardware development

Read the [web framework](../docs/tinygo/web_framework.md) before changing route
registration or serialization, and [concurrency](../docs/tinygo/concurrency.md)
before changing worker counts, connection pools or locks. The current TinyGo
runtime uses CPU0; the second main CPU is not an independent WiFi worker.

The [diagnostics guide](../docs/tinygo/diagnostics.md) explains health headers,
serial evidence, Locust scenarios and how to distinguish network trouble from
application failure. Historical experiments are in [hardware history](HARDWARE_HISTORY.md).

## Roadmap

Nana is to become a bank and a tax authority:

- **Loans** — Nana lends, with a repayment schedule. A loan is two ledger
  transactions and a schedule object; the ledger model already supports it.
- **Interest** — periodic transactions rather than recalculated balances, so
  interest is auditable like everything else.
- **Idle-money tax** — an optional levy on hoarded coins, to encourage
  spending. A scheduled transaction from each account to the treasury.

All three are additive: new transaction kinds and a scheduler. None of them
requires changing the ledger, which is the point of having built it this way.

The nearer-term work, in order:

1. **A flash-backed journal**, so the board remembers across a reboot. The
   interface exists; only a backend is missing.
2. **Batched journal writes and recovery.** Serialize self-contained batches,
   retain checkpoints, rotate flash sectors and account for erases. Define the
   power-loss window and what happens when a pending batch cannot be persisted.
   One HTTP write is not necessarily one flash erase. See
   [future flash batches](../docs/tinygo/storage.md#future-flash-batches).

After that, a name for the board — see the note under "Run it on the board".

See [AUTH_MEMORY.md](AUTH_MEMORY.md) for authentication memory budgets and bug-oriented regression tests.
See [HTTP_MEMORY.md](HTTP_MEMORY.md) for worker-owned HTTP objects, buffered-body reuse, and concurrent-request memory validation.

See [WRITE_MEMORY.md](WRITE_MEMORY.md) for bounded write buffers, retry receipts, commit processing, and actual CPU-core use.
