# Commerce API

Authenticated household API; UI integration is intentionally separate. Metadata and ownership live on the board; media never does. Money uses current smallest currency units.

- `GET /api/v1/commerce` returns `requests` and `artworks`.
- `POST /api/v1/commerce/commands` requires the normal `Idempotency-Key` header and accepts one externally tagged action below. Returns `{ "sequence": 123, "replayed": false }`. Creation IDs are the returned sequence.

```json
{"create_request":{"title":"Help with art supplies","description":"Thank you!","target":50000,"deadline":null}}
{"close_request":{"request":123}}
{"contribute":{"request":123,"amount":1000,"memo":"Enjoy!"}}
{"mint_art":{"title":"Sunrise","license":"Personal profile display; copyright retained","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","locator":"https://example.org/art/sunrise.png"}}
{"list_art":{"art":124,"price":25000}}
{"buy_art":{"art":124,"expected_owner":2,"expected_revision":125,"expected_price":25000}}
{"gift_art":{"art":124,"to":3}}
{"equip_art":{"art":124,"equipped":true}}
```

Member IDs in this API are numeric. Read the latest artwork owner, revision and price before buying. `list_art` with `price:null` removes a listing. Only the owner can list, gift or equip an edition. Sales debit the buyer and change ownership in one journal event; failed validation changes neither. Transfers clear listings and equipped status. Equipping clears the owner's previous equipped edition. A mint creates one distinct local edition, identified by its ID; identical media hashes may occur in multiple editions. The digest identifies expected content but the server does not fetch or certify ownership of external media. Creator, license, locator, digest and title are immutable after minting. Locators are HTTPS, at most 192 bytes; digest is 64 hexadecimal characters; title is 80 bytes; description/license/memo are 96 bytes.

Requests accept ordinary gifts immediately, without escrow, including gifts exceeding the optional target. Owners can close requests; optional Unix deadline requires a valid server clock. Self-contributions, closed/expired requests, insufficient balances and disabled recipients are rejected. `received` tracks net contributions in the current currency units; refunds reduce it, including for closed requests, without reopening a request. Contribution transactions retain their request ID and Gift classification. Art sales retain their art ID and Good classification. Generic cash refunds/reversals of art sales are rejected because ownership must be returned as well; a negotiated return/escrow protocol is future work.

Capacity is 32 requests (including closed) and 64 editions. Capacity exhaustion returns the existing capacity error; records are never silently evicted. Monetary reform scales current prices, targets and received totals with the same exact rules as balances. Journal events retain nonmonetary commerce changes in business audit history. All mutations use normal service authorization, durable append, replay and keyed retry semantics.

