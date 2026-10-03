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
| POST `/users` | Nana creates member with username, display_name, password, optional role/grant/mastodon_id, and `kind` (`human`, the default, or `bot`; a bot is never Nana) |
| PATCH `/users/user-N` | Name/password/Mastodon ID/`bio` (public profile line, 96 UTF-8 bytes, no control characters; only the member themselves); Nana can also change role/status or other members |
| GET `/me/api-key` | `{active, created_at, full: {active, created_at}, read: {active, created_at}}` for the caller's keys (`active`/`created_at` describe the full key); never a key |
| POST `/me/api-key` | `{password, scope?}` makes or replaces the caller's key of that scope (`full`, the default, or `read`); the response's `api_key` (`nc_…`) is the only copy, with its `scope`. Send it as `Authorization: Bearer nc_…` |
| DELETE `/me/api-key` | Revokes the caller's full key (`?scope=read`: the read key). Requests authenticated by a key get 403 here and on any password change; a password change revokes both keys |
| GET/POST/DELETE `/users/user-N/api-key` | Nana only, `kind: bot` members only: the bot's key status, a new full key (`{}`; shown once), or revocation. Bots never sign in |
| GET `/activity?after=N&limit=M` | The economy's public activity feed; see below |
| GET `/transactions`, `/transactions/tx-N` | Recent ledger and individual transaction |
| GET `/accounts/account-N`, `/accounts/account-N/transactions` | Any active member may read any account's balance and money movements (profiles); zero-value messages appear only to their sender and recipient; `account-N-usd` selects dollars |
| POST `/transfers` | Transfer with to, amount, memo |
| POST `/admin/issue`, `/admin/retire` | Nana issuance/retirement |
| POST `/transactions/tx-N/reverse` | Nana reversal with reason |
| GET/POST `/listings` | Query/create listings. Nana alone may create `kind: "good_deed"` with `side: "BUY"`: a standing good deed. Members claim it with an offer; accepting issues the offer amount as new money (ISSUE transaction, no fulfillment TODO) and the listing stays ACTIVE. Undoing an accepted claim retires the reward, even after the deed is cancelled. Direct purchase of a good deed is 403 |
| GET `/listings/listing-N` | Read one listing |
| PATCH `/listings/listing-N` | Edit title, description, price |
| POST `/listings/listing-N/purchase`, `/listings/listing-N/cancel` | Purchase/cancel |
| GET/PATCH `/admin/config` | household_name, initial_grant, currency, offer_settles_after (seconds; zero restores 48 hours) |
| GET `/offers`, `/offers/offer-N` | Visible offers, newest first, or one visible offer |
| GET `/loans?member=user-N` | That member's funded (ACTIVE or PAID) loans as lender or borrower, for profiles; `memo` is empty unless the reader is a party or Nana |
| GET `/lottos?member=user-N` | All draws, with `my_tickets` counting that member's tickets |
| GET `/offers?member=user-N` | That member's OPEN offers, for their profile; `message` is empty unless the reader is the offerer, the listing owner or Nana |
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

See `src/client.rs` and `src/client/offers.rs` for bounded schemas and response views. Offer views include id, listing, listing_title, offerer, offerer_name, amount, message, status, created_at, updated_at, reversible, and (after acceptance) settled_tx and settles_at. Accept/unaccept return `{ "offer": {...}, "transaction": {...} }`. Statuses are OPEN, ACCEPTED, SETTLED, DECLINED, WITHDRAWN and REVERSED. Settlement is computed from the persisted deadline; GET never writes a settlement event. `/state` is Nana-only and omits private offers; use the visibility-filtered offer routes. `/transactions` is the public ledger of money movements; a zero-value message transaction is readable only by its participants. `/users` includes every member's balance: balances are part of the shared ledger. Member and listing timestamps persist. Listings retain kind (item/service/currency), currency and minor_units, and include buyer_name when sold.

Only the owner may accept, including when Nana is another member. SELL debits the offerer; BUY debits the owner. Acceptance checks both accounts and funds and closes the listing. Decline/withdraw move no money. At the exact deadline, unaccept refuses even for Nana. Within the window it allows a correction overdraft and atomically reverses payment, marks the offer REVERSED and reopens the listing. It works even after the original payment leaves recent history. Manual Nana reversals remain separate and prevent duplicate refunds. Reusing an acceptance key after unaccept returns its original acceptance receipt while retained; it does not accept again.

Money requests supply an `Idempotency-Key` of 1–80 bytes. Retry with the same key and command after a timeout. Per-member receipts persist through restart and intervening commands; different commands with the same key conflict. A retry requiring a transaction outside the recent window may report stale_request; it never repeats the movement.

## Bots: member kind, keys and the activity feed

A member is a person (`kind: human`) or a bot (`kind: bot`): a program such as
a trading or news bot with its own account. Nana adds bots with `POST /users`
and makes their keys at `/users/user-N/api-key`. Bots get the starting grant
like anyone, may trade, lend, borrow and buy lotto tickets, and can't claim
good deeds (`403 bot_good_deed`).

**Read keys.** A member may hold one full key and one read key. A read key
makes every GET its member may, and nothing else (`403 read_only_key`), so a
news bot's leaked key can't move money.

**Limits.** One member keeps at most 2 live forex quotes on the 16-slot book
(`507 member_quote_limit`) and at most 4 unanswered loan offers
(`507 member_loan_limit`), so a few bots can't crowd people out.

**`GET /api/v1/activity?after=<seq>&limit=<n>`** — any active member or key.
`limit` 1–50 (default 50). Events with `seq > after`, oldest first:

```json
{"incarnation": 3, "generation": 7, "sequence": 912, "decimals": 4, "truncated": false,
 "events": [{"seq": 905, "at": 1790500000, "kind": "listing_opened",
             "actor": "account-3", "actor_name": "Robin", "actor_bot": false,
             "subject": "listing-905", "title": "Bike tune-up", "side": "SELL",
             "amount": 50000}]}
```

`sequence` is the newest event (a new reader's cursor). `truncated` says events
after `after` were already evicted (the feed reads the in-memory audit cache:
1,024 events on the S3, 128 on the S2). A changed `incarnation` means the
economy was reset. `amount` and `coins` are NC minor units, `rate` is ¢ per
whole NC, `apr_bps` a yearly rate, times are unix seconds. Kinds:

| kind | Fields beyond seq/at/actor |
|---|---|
| `member_joined` | — |
| `listing_opened` | subject, title, side (`SELL`/`BUY`), amount (price) |
| `listing_sold` | subject, title, amount; actor is the owner, other the buyer |
| `good_deed_posted` / `good_deed_claimed` | subject, title, amount (reward); other is the claimant |
| `lotto_opened` | subject, title, amount (ticket price), closes_at, lotto_kind |
| `lotto_drawn` | subject, title, amount (pool); actor is the house, other the winner |
| `loan_requested` | subject, amount, apr_bps |
| `loan_funded` | subject, amount, apr_bps; actor is the lender, other the borrower |
| `loan_paid` | subject, amount; actor is the borrower, other the lender |
| `quote_posted` | subject, side (`BID`/`ASK`), rate, coins |
| `quote_taken` | subject, side of the quote, rate, coins; actor is the taker, other the maker |
| `gift_request_opened` | subject, title, amount (target, if any) |
| `art_minted` | subject, title |

Never in the feed: descriptions, memos, offer messages, zero-value messages,
loan notes, dispute and reversal reasons, credentials or identity changes.
Events whose object has since been recycled (an old sold listing, a drawn
lotto that has been replaced) are left out rather than shown half-empty.

Loan views also carry `apr_bps` (`rate_bps × 365 / rate_days`, rounded down).

Automated clients may use replay-stable idempotency keys rather than random
ones, as long as each distinct action gets its own: the bots use
`g<generation>:m<money epoch>:mmb:<bot>:<slot>:<action>`.

## Typed command API

Nana-only `GET /api/v1/state` returns member, state and storage_failed fields, excluding credential verifiers and internal retry fingerprints. `POST /api/v1/commands` accepts an externally tagged Rust command:

```json
{"request_id":2,"command":{"issue":{"to":1,"amount":25,"memo":"Chores"}}}
```

Variants include issue, retire, transfer, list, update_listing, cancel, buy, reverse, make_offer, accept_offer, unaccept_offer, decline_offer, withdraw_offer issue_usd, post_quote, take_quote, cancel_quote, and Nana-only configure. Offer commands use a typed OfferId represented as an integer. Timestamps are server-owned event metadata, not command fields. Credential creation/update/migration variants (including `create_bot`, `set_api_key` and `set_read_key`) are rejected here; use dedicated endpoints. Obsolete add_member exists for legacy journal replay only and cannot be submitted over HTTP.

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

`GET /api/v1/diag/events` returns bounded, sanitized RAM incident history:
48 events, 32 five-second samples, boot ID, counters and connection high-water
marks. It uses the same public diagnostics/origin policy and a separate 32 KiB
wire limit. No secrets or request contents are retained; history clears on
reboot. See [diagnostics](../docs/rust/diagnostics.md#retained-incident-history-rust-firmware).

`GET /api/v1/diag` and `GET /api/v1/diag/static` need no authentication, retain
the normal origin policy, and take no ledger lock. They reuse the serving
task's serialization buffer; diagnostic wire tests enforce a 4096-byte bound.
Serialization overflow returns an error, never truncated successful JSON.
The Angular Machine health page is in Nana's administrator navigation.

Live readings are sampled every two seconds on core 0, alongside the separate
TLS handshake task. Established HTTP/HTTPS and ledger work run on core 1.
A separate mutex protects only copying the small,
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

Offer-specific errors include self_deal (400), offer_closed, listing_closed and offer_settled (409). Bot and key errors: read_only_key and bot_good_deed (403), member_quote_limit and member_loan_limit (507). An invalid or unsynchronized settlement clock returns unavailable (503). Timed mutations never reset a persisted deadline on restart.

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

## Public System Info

These read-only GET endpoints require no login, respect the normal transport
policy, and remain available when a storage failure has latched:

* `/api/v1/configuration`: household name, currency/decimal scale, grants,
  settlement timing and economy policies. No configuration mutation endpoint is
  added; changes still require Nana's existing administrative authorization.
* `/api/v1/diag/database`: aggregate persistent/RAM collection occupancy,
  capacities, retention, memory estimates, journal/checkpoint usage and invariant
  status. Does not expose credentials, tokens or record contents.
* `/api/v1/diag/database/benchmark`: bounded existing read queries, with run counts,
  min/mean/max microseconds, response sizes and query errors. No service mutation,
  cleanup or writes. Maximum three runs per query and a 250 ms budget for starting
  queries; server timing excludes network and lock wait.
* `/api/v1/diag/events`: bounded RAM incident history, counters and health samples;
  clears at reboot. Also available on the desktop server.

See [diagnostics](../docs/rust/diagnostics.md) for measurement limits and UI details.

## Kitchen screen copies

Both routes require a normal NanaCoin session or API key and return
`{"queued":true}` after persisting the outbox. They accept an empty JSON object
and do not write financial journal events:

- `POST /api/v1/transactions/tx-N/screen`: only the sender of a retained,
  zero-value NanaCoin message can request its kitchen screen copy.
- `POST /api/v1/transactions/tx-N/screen/read`: only that message's recipient can
  dismiss its copy. Reads may precede delivery; Minicloud saves a read tombstone.

Payment transactions are rejected. Retries use the same stable screen/event ID.
Notification expiry is the original message timestamp plus 86,400 seconds;
expired messages cannot be newly copied. A full outbox returns a capacity
error; storage errors are separate from the earlier successful transfer.


### Loan applications

POST /api/v1/loans/request takes the same body as a loan offer, with borrower
set to the authenticated applicant's account. It journals REQUESTED terms and
moves no money. GET /api/v1/loans includes open applications for every signed-in
household member. Existing private loan offers remain visible only to the two
parties and Nana.

POST /api/v1/loans/{id}/offer takes lender-proposed terms for that application's
original borrower. The first valid proposal claims the application and changes
it to OFFERED. The lender cannot accept on the borrower's behalf. Existing
/accept funds the loan only after borrower consent; /close lets the applicant
withdraw a REQUESTED application. All mutations require idempotency keys and
retain ordinary restart/replay and accounting checks. Applications share the
bounded 32-entry loan book. Competing proposals after the first receive conflict;
multiple simultaneous lender bids are a future extension.
