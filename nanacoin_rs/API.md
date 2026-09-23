# JSON API v1

`/api/v1/*` serves JSON. Firmware also serves Angular at `https://nanacoin.local/`;
a separately hosted client remains supported. Desktop uses loopback HTTP on
port 8080 (enable `bundled-web` to serve the UI). CORS allows configured exact
origins and the board's own HTTP/HTTPS origins. API bodies are limited to 1,024 bytes.
Static GET routes dispatch before ledger locking, with gzip negotiation, ETags
and hashed-asset caching. Unknown API routes never fall back to HTML.

## Provision and login

Transport policy applies before all API routes. Easy mode accepts HTTP and HTTPS.
Secure mode refuses every HTTP API request, including otherwise public routes.

- `GET /api/v1/transport`: public `{ "https_only": false, "supported": true }`.
- `POST /api/v1/admin/transport`: Nana bearer token, TLS listener only, body
  `{ "confirmation": "REQUIRE HTTPS" }`. Returns `true`, persists Secure mode,
  and revokes all sessions/pending codes. No remote disable operation exists.
- `GET /trust`: public, standalone installation instructions on either listener.
- `GET /ca`: public DER CA certificate attachment on either listener; never a key.

Secure HTTP serves only `/`, `/trust`, `/ca`, rejecting other paths and non-GET
methods. Economy reset does not clear this policy. See
[connection security](CONNECTION_SECURITY.md) for onboarding and USB recovery.

`GET /api/v1/status` is public. On an empty journal, `POST /api/v1/provision` takes:

```json
{"household_name":"Home","username":"nana","display_name":"Nana","password":"1234"}
```

Provisioning is allowed once. Login follows the existing client's PKCE flow:

1. Generate a random verifier of 43–128 PKCE characters. Its S256 challenge is unpadded base64url of SHA-256(verifier).
2. POST `/api/v1/auth/authorize` with username, password, code_challenge, code_challenge_method set to S256, and redirect_uri. Receive a code.
3. POST `/api/v1/auth/token` with code, code_verifier and the same redirect_uri. Receive access_token, token_type Bearer, expires_in 28800 and a user object.
4. Send `Authorization: Bearer <session-token>` on authenticated requests. POST `/api/v1/auth/logout` to revoke it.

Codes expire after 60 seconds and are consumed on redemption, including unsuccessful redemption. Sessions expire after eight hours and do not survive restart. These are temporary session credentials, not permanent account tokens. Passwords are hashed on the server; HTTP clients cannot inject credential verifiers.

## Existing client adapter

IDs use user-1, account-1, listing-5 and tx-6 forms. Roles are nana/user and member statuses ACTIVE/DISABLED. All paths below have prefix `/api/v1`.

| Endpoint | Purpose |
|---|---|
| GET `/me`, `/users` | Current user and members |
| POST `/users` | Nana creates member with username, display_name, password, optional role/grant/mastodon_id |
| PATCH `/users/user-N` | Name/password/Mastodon ID; Nana can also change role/status or other members |
| GET `/transactions`, `/transactions/tx-N` | Recent ledger and individual transaction |
| GET `/accounts/account-N`, `/accounts/account-N/transactions` | Own balance/history, or any account for Nana; `account-N-usd` selects dollars |
| POST `/transfers` | Transfer with to, amount, memo |
| POST `/admin/issue`, `/admin/retire` | Nana issuance/retirement |
| POST `/transactions/tx-N/reverse` | Nana reversal with reason |
| GET/POST `/listings` | Query/create listings |
| GET `/listings/listing-N` | Read one listing |
| PATCH `/listings/listing-N` | Edit title, description, price |
| POST `/listings/listing-N/purchase`, `/listings/listing-N/cancel` | Purchase/cancel |
| GET/PATCH `/admin/config` | household_name, initial_grant, currency, offer_settles_after (seconds; zero restores 48 hours) |
| GET `/offers`, `/offers/offer-N` | Visible offers, newest first, or one visible offer |
| POST `/listings/listing-N/offers` | Propose amount and optional message (140 UTF-8 bytes); returns 201 |
| POST `/offers/offer-N/accept` | Owner accepts; requires Idempotency-Key; returns 201 |
| POST `/offers/offer-N/unaccept` | Either party or Nana undoes before deadline, with reason (140 UTF-8 bytes) and Idempotency-Key |
| POST `/offers/offer-N/decline` | Listing owner or Nana declines |
| POST `/offers/offer-N/withdraw` | Offerer or Nana withdraws |
| POST `/admin/issue-usd` | Nana issues cents to a coin account ID; fields to, cents, reason; keyed |
| GET/POST `/quotes` | Best-rate-first book; create with side BID/ASK, cents_per_coin, coins, optional expires_at |
| GET `/quotes/quote-N` | Quote, wallet parties, rate, timestamps and live status |
| POST `/quotes/quote-N/take` | Keyed, atomic coin/cash exchange; returns quote, coin_transaction, cash_transaction |
| POST `/quotes/quote-N/cancel` | Maker or Nana cancels |

See `src/client.rs` and `src/client/offers.rs` for bounded schemas and response views. Offer views include id, listing, listing_title, offerer, offerer_name, amount, message, status, created_at, updated_at, reversible, and (after acceptance) settled_tx and settles_at. Accept/unaccept return `{ "offer": {...}, "transaction": {...} }`. Statuses are OPEN, ACCEPTED, SETTLED, DECLINED, WITHDRAWN and REVERSED. Settlement is computed from the persisted deadline; GET never writes a settlement event. `/state` is Nana-only and omits private offers; use the visibility-filtered offer routes. `/transactions` is Nana-only; individual transactions require participation or Nana. `/users` hides other members' balances from ordinary users. Member and listing timestamps persist. Listings retain kind (item/service/currency), currency and minor_units, and include buyer_name when sold.

Only the owner may accept, including when Nana is another member. SELL debits the offerer; BUY debits the owner. Acceptance checks both accounts and funds and closes the listing. Decline/withdraw move no money. At the exact deadline, unaccept refuses even for Nana. Within the window it allows a correction overdraft and atomically reverses payment, marks the offer REVERSED and reopens the listing. It works even after the original payment leaves recent history. Manual Nana reversals remain separate and prevent duplicate refunds. Reusing an acceptance key after unaccept returns its original acceptance receipt while retained; it does not accept again.

Money requests supply an `Idempotency-Key` of 1–80 bytes. Retry with the same key and command after a timeout. Per-member receipts persist through restart and intervening commands; different commands with the same key conflict. A retry requiring a transaction outside the recent window may report stale_request; it never repeats the movement.

## Typed command API

Nana-only `GET /api/v1/state` returns member, state and storage_failed fields, excluding credential verifiers and internal retry fingerprints. `POST /api/v1/commands` accepts an externally tagged Rust command:

```json
{"request_id":2,"command":{"issue":{"to":1,"amount":25,"memo":"Chores"}}}
```

Variants include issue, retire, transfer, list, update_listing, cancel, buy, reverse, make_offer, accept_offer, unaccept_offer, decline_offer, withdraw_offer issue_usd, post_quote, take_quote, cancel_quote, and Nana-only configure. Offer commands use a typed OfferId represented as an integer. Timestamps are server-owned event metadata, not command fields. Credential creation/update/migration variants are rejected here; use dedicated endpoints. Obsolete add_member exists for legacy journal replay only and cannot be submitted over HTTP.

The typed endpoint uses per-member monotonic request_id values: start at last_request + 1. The same most-recent ID and command returns the original sequence receipt with replayed=true. A changed command conflicts; an older ID is stale. Failed commands do not advance the watermark. Do not assign a fresh ID to an uncertain operation. The adapter's durable Idempotency-Key index provides stronger retry history than this last-request contract.

Creation/money endpoints return 201; updates, cancellation and unaccept return 200; logout returns 204 without a body. Ledger pages default to 100 globally or 50 per account, cap at 100, and read from the 365-record ring. Listings are newest first, with validated status filters.

Quote rates are 1–10,000 cents/coin and sizes 1–100,000 coins; there are 16 slots. Proposals do not reserve funds; taking validates both currencies and active parties. Timed quote operations require the same valid clock as offers. Expired quotes display EXPIRED and can be recycled. Transaction IDs are opaque: forex cash-leg IDs use the disjoint event-sequence + 4096 range so existing Rust event/transaction IDs remain compatible. Use response order and timestamps, not numeric ID sorting.

## Journal maintenance

Nana-only `GET /api/v1/admin/storage` returns `generation`, `sequence`,
`journal_records`, `checkpoint_after` (2,048), and `checkpoint_supported`.
`POST /api/v1/admin/checkpoint` accepts `expected_generation` and
`expected_sequence`, saves current state and reclaims the old journal.
`POST /api/v1/admin/reset` takes those same fields plus
`"confirmation":"RESET ECONOMY"`; it removes the whole household and revokes
all sessions. Both reject intervening changes with 409, reject non-Nana users
with 403, and return the new storage status on success. Reset is not retried
automatically; re-read public status after a disconnected response.

Public status includes `journal_generation` and `checkpoint_supported`.
`journal_used` now measures the active log, not lifetime sequence numbers.
Ordinary API responses expose `X-Nanacoin-Generation` through CORS. New
idempotency keys must use `g<generation>:<random>`; retained old keys still
deduplicate, while unknown retired keys fail with `stale_request`. Legacy
unprefixed keys work in generation zero only unless a receipt is retained.
See [RETENTION.md](RETENTION.md) for the complete recovery and retry contract.

Event IDs skip reserved blocks after 4,096 to preserve the disjoint forex
cash-leg range. Checkpointing never resets IDs; economy reset does.

## Diagnostics

`GET /api/v1/diag` and `GET /api/v1/diag/static` need no authentication, retain
the normal origin policy, and take no ledger lock. They use a 4096-byte
request-local response buffer, bypassing the large ledger response buffer.
Serialization overflow returns an error, never truncated successful JSON.
The Angular Machine health page is in Nana's administrator navigation.

Live readings are sampled every two seconds on core 0, away from HTTPS and
ledger work on core 1. A separate mutex protects only copying the small,
fixed snapshot (the complete shared object is compile-time capped at 288
bytes). Probes, serialization and socket writes run outside that mutex.
The sampler has a 4096-byte stack and one temperature-driver handle created
at startup. There is no board-side history, per-sample allocation, or
diagnostic flash write. NVS queries use IDF's own storage lock outside the
snapshot lock, so the last published snapshot remains available if a probe
is delayed. HTTP requests can still queue behind occupied server sockets.

Original fields remain compatible with existing soak scripts:

```json
{"uptime_seconds":977,"free_heap":159683,"largest_free_block":63488,
 "minimum_free_heap":80599,"psram_free":7359116,"samples":480,
 "sampler_core":0,"http_core":1}
```

The expanded response adds `schema: 1`, `sampled_at_ms`, `internal` and
`psram` heap objects (total/free/largest/minimum bytes and allocated/free
block counts), `temperature_c`, `rssi_dbm`, `wifi_channel`, IPv4 arrays
`ip`/`gateway`/`netmask`, `unix_seconds`, task count, sampler stack minimum
free bytes, reset-to-ready milliseconds, and HTTP request/error counters.
Counters count requests reaching the handler and HTTP error responses,
not TLS handshake failures or socket disconnects. Counters wrap at u32 max.
Unavailable probes are JSON null, including an unsynchronized wall clock.
The temperature range is 10–80 C; failed/out-of-range readings are null.

Heap sizes use INTERNAL|8BIT and SPIRAM capabilities, respectively. The
largest block measures single-allocation headroom; total free space alone
cannot establish fragmentation or TLS capacity. Minimum free records the
low-water mark since boot. Heap figures are capability aggregates, not
individual physical regions, and allocator metadata is not usable capacity.

`ledger_storage` contains read-only NVS used/free/available/total entry
counts, refreshed every 30 seconds with `storage_sampled_at_ms`. These are
storage entries, not transaction counts or erase-cycle estimates. Existing
ledger persistence still commits every journal append; diagnostics does
not change that policy.

`/diag/static` queries chip model/revision/core count, CPU MHz, reset reason,
firmware/IDF versions, physical flash size and the actual partition table.
Its fixed-capacity list holds at most 16 partitions and reports truncation.
The IDF partition iterator is released, and temporary data is dropped at
request end. Static queries execute in the HTTP handler; recurring live
probes execute on core 0. Neither reads journal contents or credentials.

These routes are firmware-only. `/status` advertises `diag_enabled: true`
on ESP-IDF and false on desktop. The dashboard fetches static information
once, polls live information without overlapping requests, suspends polling
in hidden tabs, and aborts requests on navigation. Its 120-sample history
exists only in the browser and clears on a detected uptime decrease.

Compared with `hello_wifi_s3_py`, Rust has no Python module list, GC, or
mounted file tree; the dashboard shows Rust/IDF and NVS/partition information
instead. This adds observational diagnostics, not GPIO reconfiguration,
Wi-Fi scans, forced NTP synchronization, CPU benchmarks, or CPU-utilization
instrumentation. Boot reporting is total reset-to-ready time, not a phase
trace. Die temperature is not ambient temperature.

## Errors and limits

Errors contain error and message fields. Statuses include 400 invalid input/overflow, 401 authentication failure, 403 forbidden/disabled, 404 not found, 409 conflict/stale request/insufficient funds, 413 oversized body, 429 login rate limit, 503 storage/availability failure and 507 capacity exhaustion.

Offer-specific errors include self_deal (400), offer_closed, listing_closed and offer_settled (409). An invalid or unsynchronized settlement clock returns unavailable (503). Timed mutations never reset a persisted deadline on restart.

Every movement balances debit and credit, including issuance account zero. Ordinary spending cannot overdraw. Corrections can make balances negative, matching TinyGo; they append opposite postings and never rewrite history. Journal and recent-history limits are explicit in README.md; retention is bounded.


## Physical fulfillment (current development schema)

Payment settlement and physical fulfillment are independent. A purchase or accepted
listing offer creates a TODO for the coin recipient (the provider), with the coin
payer as the recipient of the work, goods, or external cash. This applies to both
BUY and SELL listings. Classified LABOR/GOOD transfers also create TODOs. Gifts,
messages, issuance, loans, lotto and internal forex wallet transfers do not.
Listing kind `service` / economic kind `LABOR` means WORK; listing kind `currency`
means CASH; other purchases mean GOODS. Offer settlement deadlines do not mark
fulfillment done.

* `GET /api/v1/fulfillments`: `{ "fulfillments": [...] }`, scoped to either
  participant, or all records for Nana.
* `POST /api/v1/transactions/{id}/fulfillment`: requires the usual idempotency
  key; body `{ "action": "COMPLETE|DISPUTE|WITHDRAW_DISPUTE", "reason": "..." }`.
  Returns the current fulfillment record, including on a keyed retry.
* Transaction views on ledger/account reads include nullable `fulfillment`.
  Acceptance responses describe the immutable payment receipt; fetch live
  fulfillment via the above reads.

Each record has `transaction`, `provider`, `recipient`, their display names,
`description`, `kind` (WORK/GOODS/CASH), `status`, and recent `updates`. Each update
has a unique event `id`, `at` timestamp, actor account/name, status and reason.

Either the provider or the recipient can COMPLETE a TODO. Only the recipient can DISPUTE a TODO or
DONE transaction, with a nonblank reason (at most 96 UTF-8 bytes, no control
characters). Only that recipient can WITHDRAW_DISPUTE, returning it to DONE even
if the dispute was raised before a completion claim. Providers cannot overwrite
a dispute. Reversal sets REVERSED and disables further fulfillment actions.
These changes never move money. Nana's existing reversal (or an authorized
provider refund) performs any financial correction separately.

The bounded server retains 128 fulfillment records, including the original
payment needed for a refund after ordinary transaction history eviction. TODO
and DISPUTED records are never recycled. DONE/REVERSED records become recyclable
only after their original payment leaves retained history; a full unrecyclable
table rejects new obligations before payment. Each record keeps its four latest
activity events; older activity is discarded at journal compaction. Checkpoints
use 4096-byte rows for the current schema. No old development-data migration is
provided. Currency reforms rescale retained refund amounts, and an economy reset
clears obligations with the rest of the economy.
