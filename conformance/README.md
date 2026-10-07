# NanaCoin JSON v1 conformance

These are executable client-facing contracts for a NanaCoin bank. They use real
HTTP requests and JSON responses only: no Rust imports, direct state access,
database inspection, or journal decoding. They can test an alternative server
without changing the assertions. Python 3.10+ and its standard library suffice.

Run from the repository root in **PowerShell on Windows**:

```powershell
python conformance/check.py
```

To build/test both selectable engines and reopen restart fixtures with the
other engine in **both directions**, including credentials, receipts, art,
reforms and settlement deadlines:

```powershell
python conformance/check.py --cobol
```

The matrix runs all 100 ordinary contracts twice. The fresh process uses its selected
build; restart fixtures reopen the same opaque data path with the other build.
The default Rust build requires no COBOL compiler or runtime. The wallet boundary contract fills NC and USD to the exact integer limit,
rejects overflow, and rechecks a large partial refund and retry after restart.
This checks
current-schema interchange between the two builds, not legacy migrations.
After building both executables, the matrix tests the independent banks in
parallel. Each contract has its own ephemeral port and data directory. Logs and
JUnit reports go to each isolated target directory as `conformance.log` and
`conformance.xml`; failed runs print their complete log.

For deterministic interest and scheduler contracts, enable the optional launcher
clock on both builds:

```powershell
python conformance/check.py --cobol --clock
```

This command also compares complete HTTP/JSON response transcripts for 18
deterministic scenarios covering accounting, accounts, offers, forex, gifts,
art, fulfillment, reform, configuration, lending and lotto. The settled lotto
scenario has a single eligible participant, so its observable winner is fixed
without changing the server's randomness source. Each scenario runs once
per engine and reopens its data with the opposite engine. Authentication
responses and startup polling are excluded; scheduler waits compare the complete
condition-satisfying observation rather than the number of polling reads. Bank incarnation identifiers are
normalized. Financial values, business identifiers, fixed-clock times and
object states must match exactly. The comparison artifact is
`.local/conformance-differential.json`. These comparisons supplement the
110 contracts rather than increase that contract count.

To repeat only the differential scenarios with already built clock-enabled
executables:

```powershell
$env:NANACOIN_COBOL_DLL=(Resolve-Path .local/conformance-cobol/ncposting.dll).Path
python conformance/differential.py --rust .local/conformance-rust/debug/nanacoin.exe --cobol .local/conformance-cobol/rust/debug/nanacoin.exe --report .local/conformance-differential.json
```

This runs 110 contracts per build. The ten additional cases check one-day simple
interest, interest-first payment/payoff, scheduled installment catch-up,
exact quote/gift expiry boundaries, member 32 lotto payouts, savings principal/
interest, delayed and empty draws, recycling, same-second identity changes that
must not create public join events, and listing order/status filters across
selection chunks, including cross-build restart. The tenth case fills all bounded
business books simultaneously, checks each overflow rejection, reforms the full
bank, crosses the advertised retained-transaction capacity with balanced payments,
and compares every book, ledger paging and retained counts after reopening with
the other engine. This also exercises current-schema checkpoint-and-tail recovery. The runner
changes an opaque launcher clock file;
all financial assertions still use HTTP/JSON. `conformance-clock` is an explicit
desktop build feature; its `--conformance-clock-file PATH` input adds no HTTP
endpoint and is absent from regular builds. Alternative implementations may
provide this launcher convention to opt into the extra contracts. Without
`--clock`, the 100 ordinary contracts run and the ten clock cases are skipped.

For an alternative implementation:

```powershell
python conformance/run.py --server C:/path/to/alternative-bank.exe
```

The executable must serve HTTP on `127.0.0.1:$NANACOIN_PORT`, store all its
development data under `$NANACOIN_JOURNAL` (sidecars are fine), and start
unprovisioned when that path does not exist. A wrapper executable may translate
these settings to another server's command-line options. `--server-arg VALUE`
can be repeated; use `--server-arg=--flag` for values beginning with a dash.
The runner sets `NANACOIN_ORIGINS=http://localhost:4200` and
`NANACOIN_MINICLOUD_URL=disabled-for-conformance` to suppress Rust screen delivery.
Alternative implementations must also keep external integrations disabled for
this profile. The household-statistics contract enables only an ephemeral
loopback mock screen receiver and checks the existing screen HTTP/JSON protocol;
it contacts no external service. Each contract gets its own process, ephemeral loopback port and
temporary directory. Restart tests reopen the same opaque data path. Processes
are terminated and fixture directories removed on success or failure. The runner
never attaches to an existing bank or accepts a board URL.

To run one contract or write JUnit results:

```powershell
python conformance/run.py --server .local/conformance-rust/debug/nanacoin.exe --test test_restart_recovers_money_and_receipts_but_not_sessions
python conformance/run.py --server .local/conformance-rust/debug/nanacoin.exe --xml .local/conformance-rust/results.xml
```

## Baseline profile (75 contracts)

| Area | Observable promises |
| --- | --- |
| Setup and identities | Provision once; discover user/account IDs from JSON; username/display-name/control/Mastodon validation and blank-display fallback; duplicate usernames, 32-member capacity and error precedence; profile persistence and self-permission boundaries; PKCE and one-use codes, including failed redemption/wrong redirect; logout; full/read/bot keys and replacement/persistence/revocation; role changes; disabled users; protect the last active Nana |
| Authorization | Bearer authentication, administrative permissions, read-only API keys, public money ledger and private messages in participants' account history |
| Accounting | Issue, retire, transfer, administrative reversal with correction overdraft; recipient partial refund; integer amount constraints; exact browser balance ceiling; rejected operations leave balances unchanged; disabled accounts block ordinary refunds but permit administrative corrections; underfunded refunds can retry, partial refunds prevent full reversal, and issuance/correction rows cannot be refunded; private-message text validation, classified quantity/item/kind/unit checks, and historical payment/refund amounts across several epochs with unchanged USD |
| Retry and persistence | Stable receipts, conflicting key reuse, intervening commands, simultaneous retries, header bounds, failed requests may retry, per-member key scope, typed-command watermarks; durable money/receipts after forced process termination; sessions expire on restart |
| Activity and audit | Sanitized public event kinds, fields, parties and chronology; private descriptions/messages/credentials excluded; loan payoff sequence disambiguates same-second repayments; activity limit/after bounds, read keys and cross-build restart; admin-only audit paging, stale incarnation/page precedence, reverse sequence order and no duplicates |
| Ledger | Balanced double-entry postings; bounded cursor pages with no gaps or duplicates for the fixture; clamped limits, stale/oversized cursors, and snapshot paging across cross-build restart and intervening writes; deterministic generated payments checked against an independent client balance model |
| Marketplace | Direct purchase/cancellation; BUY/SELL payment direction; owner-only acceptance, declined/withdrawn/unfunded offers, private offer notes, good-deed issuance/undo, no double refund, atomic undo/reopening, persistent settlement deadlines and reads never writing; open-offer capacity protection/closed-record recycling; concurrent acceptances with distinct keys pay once |
| Forex and lending | BID/ASK atomic coin/cash legs, exchange retries, rejected/cancelled trades, exact amounts, member/book capacity, price/id ordering and closed-row recycling; disabled-lender settlement pauses and failed-key recovery; loan term/lender limits, request/response identity, book capacity/terminal recycling, credit waits for zero balance, proposals move no funds, funding/repayment authorization, private notes, closure and payoff |
| Gifts and art | Contribution target is not a cap, closure and cumulative net refunds, request permissions/capacity, refunds in current currency after reform and stale epoch-key rejection; art revision checks and atomic purchase/ownership, competing buyers pay once, retained 64-edition capacity, gifting/equipping, metadata boundaries and correction rejection |
| Fulfillment | Work/goods/cash types and participant direction; TODO/completion/dispute/withdrawal transitions, permissions, stable retries, no cash movement; partial refunds preserve obligations, full refunds reverse them; capacity protects recent completed records and survives cross-build restart |
| Currency precision | Reform preview moves nothing, exact rescaling of balances/listing prices and historical projections, USD unchanged, inexact/stale proposals rejected, epochs/scales survive restart; compound reform with loans, lotto escrow, quotes, gifts, art and partially refunded payments; original refund units, exact quote cash totals, rounded lotto obligations, parameter/overflow rejection and the 32-epoch ceiling |
| Lotto | Ticket purchase/escrow and receipts, permissions/count validation, closed draw rejects late tickets, single-participant draw pays once and winner/result survives restart; bounded book/creation validation, house restriction and atomic unfunded purchases; clock cases cover member 32 payouts, all-member savings principal, delayed interest, empty pools and recycling |
| Configuration and HTTP | Household settings and initial member grants; CORS preflight/denied origins; malformed and oversized JSON |

Amounts are integer NC minor units (the fresh bank has four decimal places),
and USD wallets use cents. Tests compare semantic JSON objects, discovered IDs
and balance changes, never hard-coded journal sequence numbers or wall times.
The balance ceiling is 9,007,199,254,740,991; one movement is at most
1,000,000,000,000,000 minor units. Zero-value transfers are private messages;
they are excluded from the public `/transactions` endpoints even with a token.
Participants read them through `/accounts/account-N/transactions`.

This is a **baseline profile**, not complete certification of every NanaCoin
feature. It does not cover lotto randomness fairness, all scheduler race combinations, compound reforms of all
financial products, retention exhaustion/checkpoint failure, HTTPS policy or
board diagnostics. Those need additional interface contracts. A short offer
deadline and actual simple draw settlement use the server's real wall clock;
the draw takes about 36 seconds to finish its durable settlement steps. The full
ordinary profile normally takes about a minute per build on this machine;
the clock profile adds three complete lotto settlements. Timed cases
poll public state with bounded timeouts. The optional clock profile advances
loan and lotto time through the launcher and does not wait real days. Restart tests prove process-crash recovery,
not physical flash power-cut behavior. The API reference is
[nanacoin_rs/API.md](../nanacoin_rs/API.md); the tests pin down this profile's
precise request/response behavior and should evolve with the current schema.

## COBOL experiment

The same unmodified contracts run against a Rust server using a GnuCOBOL
posting kernel:

```powershell
python nanacoin_rs/cobol/build.py --check
```

See [the port notes](../nanacoin_rs/cobol/README.md) for its scope and ABI.
The same notes contain the phased plan for keeping both builds deployable.
