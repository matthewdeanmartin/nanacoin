# Fresh TinyGo / Rust parity review — 2026-09-19

Compared the current working tree, including the uncommitted TinyGo memory work,
in `../nanacoin_go/internal/{api,core,ledger,auth,marketplace}`, the board entry point,
and the Angular client contracts against this Rust implementation. Changes in
this pass are confined to `nanacoin_rs`. No board connection or existing household
journal was used during validation.

## Gaps closed

| Area | Rust behavior after this pass |
|---|---|
| USD / forex | Nana USD issuance; separate `account-N-usd` wallets; BID/ASK quotes; reads, expiry, cancellation and keyed takes; integer rates and amounts |
| Trade integrity | Both currency legs and the filled quote commit in one journal event; failed or ambiguous storage cannot publish a half-trade |
| Financial privacy | Member lists hide other members' balances; accounts/history are self-or-Nana; the full ledger and Rust-only `/state` are Nana-only; transaction reads require participation or Nana |
| Ordinary payments | Disabled senders and recipients are refused, including direct purchases and forex; correction overdrafts remain possible |
| Resource reads | GET for individual listings and accounts, including USD accounts and their retained histories |
| Listings | Item/service/currency metadata retained; buyer name exposed; creation/update timestamps replay; validated status filters and newest-first order |
| Members | Creation timestamps replay from server-owned event time |
| History | 365-entry preallocated ring; eviction preserves lifetime balances; lifetime/retained/capacity/oldest status fields; no full-array shift on eviction |
| Live capacity | 16 members, 48 listing slots, 32 offer slots, 16 quote slots; live records are never overwritten to admit new records |
| HTTP results | 201 for creation and money movement; 204 with no body for logout; ledger limit defaults 100 globally / 50 per account, capped at 100 |
| Firmware allocation | NVS record keys formatted into a fixed stack string instead of allocating per journal read/write |

Existing authentication, issue/retire/transfer/reverse, SELL/BUY purchases,
negotiated offers, settlement deadlines, role/status changes and configuration
remain covered by the earlier regression suite. Optional request memo/reason and
listing description strings now default to empty as in Go.

## Remaining differences and explicit boundaries

This is not a claim of byte-for-byte or complete feature parity.

* Go's board currently uses a discarding RAM journal; Rust durably commits each
  command to NVS. Rust now checkpoints and retires its journal at 2,048 changes,
  preserving current state, recent history and bounded retry receipts. Nana
  can close the journal early or reset the economy to provisioning. See
  [RETENTION.md](RETENTION.md) for generation-aware client keys and recovery.
  There is no daily write batching or Go journal import. A request count is not an erase-cycle count; the owner's
  conservative 100,000-cycle flash design budget still needs wear/write-
  amplification measurements before long-term use.
* Go's packed text arena can evict ledger records before its 365-record count
  limit. Rust keeps fixed-capacity inline strings and a separate preallocated
  ring. Both are bounded, but their RAM costs differ. Rust's fixed response
  scratch is now 512 KiB for complete state views with worst-case escaping;
  PSRAM is enabled. This is startup allocation, not per-request buffer growth.
* Rust retains its existing 96-byte memo/description limit (offer messages and
  undo reasons are 140); Go allows 140-byte memos and 500-byte descriptions.
  Rust bodies remain 1 KiB and journal frames 1,024 bytes. An event whose escaped
  representation cannot fit is refused before mutation. These limits are not
  silently relaxed beyond the tested journal/stack budget.
* Go's board caps ledger pages at 30; its desktop default and Rust cap at 100.
  History is a recent window, not a complete archive. Old keyed responses may
  become unavailable after history/closed-resource eviction, but money is never
  moved a second time for a retained durable key.
* Rust has serial heap diagnostics, but no Go-compatible `/logs` or `/diag`
  routes, streaming log UI, or per-response Go health header. Status advertises
  those features as disabled. TLS, Wi-Fi, HTTP and NVS allocations are outside
  the zero-allocation domain tests.
* Error strings, unsupported-method handling (some Rust routes return 404 rather
  than Go's 405), OPTIONS responses and strict input parsing still differ.
  Rust rejects malformed side values and unknown request fields rather than
  silently defaulting them. Ordinary Rust amounts remain capped at one billion.
* Existing Rust journal events retain their IDs and encoding. Forex cash-leg
  transaction IDs use event sequence + 4096; event sequences skip alternating
  4096-ID blocks to keep this range disjoint from ordinary
  transaction IDs; IDs are opaque and must not be sorted numerically. The
  ledger response already supplies chronological order. Old events lacking
  timestamps/metadata replay with zero/default values.
* The existing Rust partition layout and HTTPS host are different from Go.
  Flashing Rust is not a migration of the Go household. No automatic erase or
  recovery deployment was added.
* Loans and interest are not implemented by either current server; the old Rust
  README listed them among omissions without distinguishing roadmap features.

## Evidence

See `VALIDATION.md` for commands and firmware output. Fifty host tests, Clippy,
formatting and the real HTTP/restart smoke pass. Added tests verify privacy,
metadata replay, both forex directions, deadlines, permissions, capacity,
recycling and injected failed/ambiguous storage. Allocation tests measure zero
allocations after startup across 2,999 financial writes with state reads and
1,000 forex trades with retries and quote-book reads. These counts exceed the
reported few-hundred-transaction TinyGo failure point, but are host domain/API
measurements, not proof of board OOM immunity.

The next hardware pass should flash this intended build when the owner connects
the board, then measure internal free/largest/minimum heap, PSRAM, stack headroom,
TLS churn, writes and replay. Do not use an older recovery image simply to keep
the board responsive between experiments.
