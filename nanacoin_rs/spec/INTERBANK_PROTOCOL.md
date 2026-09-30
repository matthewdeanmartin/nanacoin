# Interbank protocol: forex, foreign trade and settlement between banks

Status: design proposal, September 29, 2026. Nothing here is implemented.
[interbank_communications.md](interbank_communications.md) is the ground truth
on the two boards (addresses, identity, limits), and
[SECOND_BANK.md](SECOND_BANK.md) explains why there are two. The accounts used
below are explained for readers in
[docs/accounting](../../docs/accounting/index.md).

## Owner decisions so far

| Date | Decision |
|---|---|
| Sept 28 | Each bank issues its own currency. Money between banks is **exchanged, not moved**. Foreign coin never counts toward local issuance. |
| Sept 28 | No proof-of-work, mining or consensus schemes. |
| Sept 29 | **Every member may hold foreign coin.** This limits how many banks one bank can federate with. |
| Sept 29 | **Exchange rates float.** Nobody decrees them after the first swap. |
| Sept 29 | A bank may overdraw with another bank **only if the other bank allows it** (a credit limit set by the creditor). |
| Sept 29 | Cross-border listings are wanted. That means copying listings between banks and settling both **person-to-person** and **bank-to-bank** payments. |
| Sept 29 | Banks are assumed to trust each other. When trust ends, there must be a way to **wind down**: buy back foreign coin and net the banks, or convert everything to USD and settle the difference. |
| Sept 29 | No interbank encryption of payloads. The other bank receives either plaintext or opaque bytes it can't read. |
| Sept 29 | Put a queue in the middle so one bank can't flood another. Workers on the queue box are a future extension. |
| Sept 29 | Browser push (WebSockets or similar) may live on the same box as the queue. |
| Sept 29 | Wire format is **postcard for now**, with a boundary that allows changing it later. |
| Sept 29 | The post office is a **$35 Raspberry Pi**, shared with other household uses. |
| Sept 29 | **Interbank overdrafts charge interest in the lending bank's home currency** (the currency that was lent). |
| Sept 29 | **Every member may trade internationally.** No per-member opt-in by Nana. |
| Sept 29 | Banks **start by each buying some of the other bank's currency**. |
| Sept 29 | **The bank only does atomic transactions.** Compound actions (convert at market, then buy) belong to the client, as separate bank transactions. |

## Goals and non-goals

Goals:

- Members of one bank can pay members of another bank, buy from their
  listings, and trade the two currencies at a floating price.
- A bank can be offline, restart or lose Wi-Fi at any moment without losing
  or duplicating money.
- Each household can see its trade with other banks and what the banks owe
  each other.
- The same protocol works on one LAN and between households far apart.
- It fits the S2: bounded memory, one core, small messages.

Non-goals:

- No shared ledger, blockchain or consensus. Each bank's ledger is the only
  authority over its own currency.
- No atomic commit across two boards. The design never needs one (see
  "Settlement").
- No member logging in to a foreign bank, and no forwarding of tokens,
  passwords or sessions between banks.
- No cross-bank order matching in the first version.

## Architecture: a post office in the middle

```
   household 1                                   household 2 (later)
 ┌────────────┐     ┌──────────────────────┐     ┌──────────────────────┐
 │ S3 bank    │◄───►│ post office box      │◄───►│ post office box      │
 │ (server)   │HTTPS│  - message broker    │ TLS │  (same software)     │
 └────────────┘     │  - relay             │     └──────────▲───────────┘
 ┌────────────┐     │  - browser push      │                │ HTTPS
 │ S2 bank    │◄───►│  - future workers    │                ▼
 │ (server)   │HTTPS└──────────────────────┘          ┌───────────┐
 └────────────┘                                       │ bank C    │
                                                      └───────────┘
```

Every household that federates runs one always-on **post office**: a $35
Raspberry Pi that also does other jobs for the household. It runs a message
broker (Mosquitto) and a small **relay**. Because the Pi is shared, the
post office runs as its own service user with its own data directory, and
nothing else on the Pi gets the relay credentials or the broker's bank ACLs.
Losing the Pi loses no money: banks keep their outboxes, and the broker's
queues are only a cache of messages the banks can resend. For households far apart, their
post offices connect to each other, or to a shared broker on the internet,
by bridging. The broker keeps queues for banks that are offline and enforces
flow limits.

### The banks stay servers

There are two ways a bank can reach the broker:

| | **T1: board is a client** | **T2: post office pulls and pushes (recommended first)** |
|---|---|---|
| New code on the board | Outbound TLS and MQTT client (`esp-mqtt` through `esp-idf-svc`), its own task, CA pinning | Two HTTPS endpoints on the existing server |
| RAM on the S2 | A second TLS session (about 20 KiB of buffers) plus a task stack; must be measured | One of the existing TLS client slots, while the relay is connected |
| Flash | `esp-mqtt` plus glue | Almost none |
| Board without a post office | Could reach a remote broker directly | Cannot federate |
| Far away | Board connects to the remote broker | The local post office bridges to the remote one |

The S2 has about 42 KiB of internal RAM free and serves at most 3 TLS
clients, and neither board has an HTTP client. T2 adds no outbound
networking to either board. The relay connects to each bank like any other
client:

- `GET /api/v1/interbank/outbox?peer=<bank_id>&after=<seq>` returns up to N
  signed envelopes as `application/octet-stream`, oldest first.
- `POST /api/v1/interbank/inbox` delivers envelopes in order. The response
  says which sequence numbers were applied and which are the next expected.

Both endpoints need a **relay credential**, a bearer token that allows only
these two calls. Messages are signed by the banks, so a stolen relay token
can't forge money. The credential protects against flooding and snooping.

The relay polls each bank's public `GET /api/v1/status` (it already does this
cheaply for the UI) and fetches the outbox only when `sequence` moved.

T1 stays open for later: the envelope, links and settlement rules are the
same with either transport. The code puts them behind a `Transport` trait
with a desktop in-memory implementation for tests.

### Why a queue and not direct connections

- **No flooding.** A bank takes messages from its inbox at its own pace. The
  broker holds the backlog, not the board.
- **Offline banks are normal.** The S2 takes about 23 s to join Wi-Fi and
  either board may restart. The queue waits for them.
- **One path for LAN and internet.** Moving to another household changes the
  broker's bridging, not the banks.
- **A home for extras.** Browser push and future workers run on the post
  office, not on the boards.

### Flow control

At four layers:

1. **Broker.** Per-bank queue length and message size limits (Mosquitto
   `max_queued_messages`, `message_size_limit`), and ACLs so a bank's relay
   can only write to peer inboxes and read its own.
2. **Sending window.** A bank has at most W unacknowledged messages per link.
   Further messages wait in its bounded outbox. When the outbox is full, new
   cross-border actions are refused with "link busy". Local business goes on.
3. **Receiving pace.** The receiving bank applies at most a few inbound
   messages per scheduler turn. On the S2 that is the same main-task turn
   that runs the payment tick, so the web server keeps priority.
4. **Value caps.** Each bank sets per-link inbound caps per day in its own
   coin (for example, at most 1,000 coins' worth of foreign payments a day)
   and a per-message maximum. A message over a cap is **returned**, not
   ignored, so money is never stuck.

### Browser push on the post office

The broker can also serve MQTT over WebSockets to phones. A bank's relay
publishes the bank's journal `sequence` to a retained topic, and the Angular
client subscribes and calls its normal `refresh()` when it changes. No slot
on the board is held open. This removes most of the cost described in
[WEBSOCKETS_ANALYSIS.md](WEBSOCKETS_ANALYSIS.md). The client keeps its
5-second polling as a fallback. Phones must trust the post office's
certificate (issued by the household CA), and the pushed value must stay
public data (the sequence number only).

### Future workers

Later, the post office can run workers for work that needs no money
authority:

- fanning out listing snapshots to many peers,
- aggregating exchange rates across banks for charts,
- proposing multilateral netting for banks to confirm.

Workers never hold keys, never sign money messages, and never apply
anything to a ledger. A bank only acts on messages signed by another bank.
That keeps the trust model the same whether there are zero workers or many.

## Bank identity and pairing

- At provisioning, each bank generates an **ECDSA P-256 key pair** with
  mbedTLS (already linked for TLS, so there's no new crypto code) and keeps
  the private key in NVS. The private key never leaves the board.
- `BankId` = the first 16 bytes of SHA-256 of the public key. Household and
  currency names are display labels, never identities. Resetting a household
  generates a new key and so a new bank.
- A bank publishes a **bank card**: `BankId`, public key, household name,
  currency name, decimals, money epoch, peer capacity, and a quote snapshot.
  The card is signed. Bank card v1 is served by the board at
  `GET /api/v1/interbank/card` and retained by the broker.
- **Pairing** is done by the two Nanas. Each enters the other bank's card
  (fetched through the relay), then both screens show an 8-character
  fingerprint to compare out loud, like Bluetooth pairing. Each side then
  journals `PairBank { card, credit_limit, inbound_caps }`. A link is live
  when both sides have paired.
- Replacing a key or unpairing is a journaled Nana action on each side.
  There's no automatic key rotation in v1.

## Wire format

Machine to machine, binary, small. A separate module (`interbank_wire.rs`)
owns it and has nothing to do with the journal encoding, so the journal
can change without breaking peers, and the wire codec can change later
without touching the ledger.

```text
SignedEnvelope { envelope: bytes, signature: [u8; 64] }   // P-256 r||s
Envelope {
    version: u8,          // 1
    from: BankId,
    to: BankId,
    link_seq: u64,        // 1, 2, 3 ... per direction, no gaps
    ack: u64,             // highest seq from the peer applied here
    sent_at: u64,         // sender's wall clock, display only
    body: Body,
}
Body =
    AckOnly
  | CreditTransfer { transfer: TransferId, asset: Asset, amount: i64,
                     payer: PartyRef, payee: PartyRef, purpose: Purpose,
                     memo: Memo }
  | Return { of_seq: u64, reason: ReturnReason, amount: i64, asset: Asset }
  | Statement { as_of_in: u64, as_of_out: u64, positions: [Position; 2] }
  | BankCard(Card)
  | CreditLimit { asset: Asset, limit: i64 }
  | ListingSnapshot { listings: bounded list }
  | ListingSold { listing: u64 }
  | SwapProposal { .. } | SwapAccept { .. }
  | Freeze { reason } | WindDown { .. }
```

- **Encoding:** postcard with a frozen struct layout. Postcard is not
  self-describing, so the layout is pinned by golden byte vectors in tests.
  A layout change is a new `version`. Pre-release, peers are simply upgraded
  together (see `AGENTS.md`).
- **Size:** one envelope is at most 512 bytes, a listing snapshot at most
  2 KiB.
- **Signature:** ECDSA over SHA-256 of the envelope bytes. The receiver
  verifies it against the paired key before looking at anything else.
- **Amounts** are integer minor units in the currency epoch stated in the
  `Asset`. A message in an out-of-date epoch is returned.
- **Memos are plaintext.** Anything a member encrypted end-to-end is carried
  as opaque bytes. Private local memos are not sent. The broker operator can
  read interbank traffic, so a household only uses a post office it trusts.

## Links, ordering and exactly-once

A **link** is a pair of paired banks. Each direction has its own sequence
numbers. Durable link state, part of each bank's checkpoint:

| Field | Meaning |
|---|---|
| `peer`, `peer_key` | Who, and the key to verify their messages. |
| `next_out` | The next sequence number to assign to an outbound message. |
| `acked_out` | Highest outbound sequence the peer has confirmed. |
| `applied_in` | Highest inbound sequence applied here. |
| `in_hash` | Hash of the last applied inbound envelope, to detect conflicting versions. |
| Positions | Balances of the correspondent accounts (below). |
| State | `Live`, `Frozen`, `WindingDown`, `Closed`. |

Rules:

1. **Only `applied_in + 1` is applied.** Earlier numbers are duplicates and
   are acknowledged again. Later numbers wait (the broker redelivers in
   order).
2. **Applying message n is one journal event** (`ApplyInterbank { peer, n,
   envelope digest, body }`), and the same event moves `applied_in` to n. The
   ledger change and the replay guard are durable together, so a message can
   never be applied twice. Unlike today's retry receipts, `applied_in` is
   never evicted.
3. **Every inbound number is used up**, even when its content can't be
   applied (unknown payee, over a cap, closed listing). The event then
   creates a `Return` message instead of a ledger credit. There are no
   gaps and no stuck money.
4. **Outbound messages come from journal events.** The event that debits a
   member also assigns `next_out` and stores the body. Replay rebuilds the
   outbox. The signature is created when sending, and re-signing on a retry
   is fine because the receiver deduplicates by number and compares content
   digests.
5. **The same number with different content** from a peer is an error on
   the peer's side (for example a restored old image). The link freezes.
6. **Acks** travel in every envelope. An idle link sends `AckOnly` after a
   short delay. The outbox drops messages up to `acked_out`.

## Clocks

No money decision depends on comparing two boards' clocks.

- Order comes from `link_seq`, never from `sent_at`.
- Expiry is decided only by the bank that owns the promise, with its own
  clock. For example, a listing or quote owned by bank B is checked by B
  when the purchase message arrives. If it has closed, B returns the money.
- `sent_at` is shown to operators, and each bank keeps a running estimate of
  the peer's clock offset. Board Health shows it. If the offset is more than
  5 minutes, outbound money pauses, because it means one side's SNTP is
  broken (and local features that need a valid clock are then unreliable too).
- Statement periods ("daily") are each bank's own local days. Statements are
  matched by sequence numbers, not by dates.

## Accounts

This is correspondent banking. Take banks A (A-coin) and B (B-coin).

On A's books:

| Account | Asset | Balance means |
|---|---|---|
| Member A-coin wallets | A-coin | As today. |
| Member B-coin wallets | B-coin | B-coin that A holds on each member's behalf. |
| `nostro[B]` | B-coin | Minus A's total holding at B. A's outside account for B-coin, as USD issuance is for dollars. |
| `vostro[B]` | A-coin | A-coin held here by bank B. Floor: minus the credit limit A granted B (zero by default). |
| `in_transit[B]` | per asset | Outbound amounts sent but not yet acknowledged. |

B's books mirror these. The two cross-checks:

```
A.nostro[B]   (B-coin) = − B.vostro[A]   after in-transit messages
A.vostro[B]   (A-coin) = − B.nostro[A]   after in-transit messages
```

Each bank's sum-to-zero invariant holds per asset on its own books, whatever
the other bank does. The cross-checks are what statements compare.

**Interbank credit creates money.** If B allows A's account at B to go
negative, B's members can end up holding more B-coin than B's issuance
account records. That's the same thing that happens when a bank lends. B's
money-supply report must add negative correspondent balances to
`circulation`, and show them separately as "credit to foreign banks".

**Overdraft interest** is charged by the lending bank in its own currency.
If A's account at B is negative, B accrues interest on it in B-coin, at a
rate B sets in the link terms. It uses the same arithmetic as household
loans (`loans.rs`: simple interest, exact remainder carried forward). B's
scheduler posts the interest as a journaled leg: `vostro[A]` pays B's house
account, with kind `INTEREST`. That can take the account further below
zero, but never past the credit limit; interest that doesn't fit stays
owed in the link record until A's account has room. B then sends A a
`CreditTransfer` notice with purpose `Interest`, so A's books record the
same amount against `nostro[B]`. Interest accrues on B's clock only, and A
never computes it, so the two clocks can't disagree about it.

**Capacity.** Every member wallet needs one balance per foreign currency.
Bounds: S3 at most 4 peers, S2 at most 2. With 16 members that's up to 64
extra balances on the S3. Account and asset IDs replace the `usd` flag first
(roadmap: "Asset ID and currency epoch instead of a USD boolean").

Until the roadmap's account refactor lands, correspondent accounts can use
reserved one-byte IDs beside issuance (0) and lotto escrow (255), for
example 240 to 247.

## Settlement

The key design choice: **each bank's side of a payment is final on its own,
and a failure is fixed by an opposite payment, never by rolling back.**
There are no two-phase holds and no timeouts that decide money.

### Person-to-person payment

Alice (at A) pays Bob (at B). The payment can be in either bank's coin.

**Paying in B-coin** (Alice holds B-coin):

1. At A, one event: debit Alice's B-coin wallet, credit `nostro[B]` (A now
   holds less at B), and queue `CreditTransfer #n`.
2. At B, applying #n: debit `vostro[A]` in B-coin (down to its floor), credit
   Bob in B-coin.
3. If B can't apply it (unknown payee, cap, floor reached), B sends
   `Return(of #n)`, and A's applying event reverses step 1 to Alice.

**Paying in A-coin** (Bob will hold foreign A-coin):

1. At A: debit Alice's A-coin wallet, credit `vostro[B]` in A-coin. Queue #n.
2. At B: debit `nostro[A]` (B now holds more at A), credit Bob's A-coin wallet.
3. Failures return the same way.

In both cases Alice sees "sent, in transit" until B's ack for #n arrives,
then "delivered", or "returned" with the reason.

### Bank-to-bank payments

The same `CreditTransfer`, with the bank's own house account (Nana's desk) as
payer or payee, authorized by Nana. They're used for:

- **The opening purchase** to start trading: each bank buys some of the
  other bank's currency. In one agreed deal, A pays X A-coin into B's
  `vostro` account at A, and B pays Y B-coin into A's `vostro` account at B.
  Each bank pays from its house account (Nana's desk), issuing first if it
  must. The ratio X : Y is the only exchange rate anyone ever sets. After
  that, the rate floats. It is proposed as `SwapProposal`, confirmed by
  `SwapAccept`, and each side applies its own half as one journaled event.
  Each Nana can then sell her foreign coin to members through her local
  order book.
- **Credit limit changes** (`CreditLimit`): only the creditor sets its
  limit. Lowering it below the current balance blocks new debits but doesn't
  claw anything back.
- **Netting and wind-down** (below).

### Returns and refunds

Only the bank that holds the money can send it back. The other bank can only
ask:

- A seller's bank that reverses a cross-border sale sends a `CreditTransfer`
  with purpose `Refund { of_transfer }`.
- A buyer's bank can send a `RefundRequest`. The seller's bank decides with
  its own dispute rules and replies with a refund or a refusal.

## Forex: floating rates

- Each bank has a **local order book per currency pair** (A-coin/B-coin,
  A-coin/USD, B-coin/USD). This generalizes today's quote book
  (`forex.rs`) from "coin against USD cents" to any two assets, keeping the
  one-event, two-leg atomic fill.
- Members who hold both currencies trade them with each other at their own
  bank. Nana can run a market-making ladder per pair, as she does for dollars
  today (`NANA_AS_MARKET_MAKER.md`).
- Each bank's card includes a **quote snapshot** (best bid, best ask, last
  trade per pair), so members see the rate on both sides.
- Prices at the two banks may differ. Members can profit from that by moving
  coin between banks (arbitrage), which is what pulls the rates together.
  It's a lesson, not a bug, and the value caps keep it small.
- **Not in v1:** taking a quote posted at the other bank. It would be done
  later as a `CreditTransfer` with purpose `TakeQuote { quote }`: the owning
  bank fills the quote and sends the other leg back, or returns the payment
  if the quote is gone.

## Cross-border listings and purchases

- A member marks a listing "visible abroad". The bank includes it in its
  signed `ListingSnapshot` (bounded: S3 16, S2 8 exported listings), sent to
  each peer when it changes.
- Each bank keeps a bounded **mirror** of foreign listings (S3 16 per peer,
  S2 8), marked "abroad", read-only, with the owning bank's listing ID and
  revision. Mirrors are public data, not obligations, and are safe to drop
  when full.
- **Buying:** the buyer's bank sends a `CreditTransfer` with purpose
  `Purchase { listing, revision, price }`, paid in the seller's currency.
  The bank never converts currency as part of a purchase. The
  seller's bank checks that the listing is active and the revision matches,
  then in one event marks it sold, credits the seller and opens a
  fulfilment. Otherwise it returns the payment. `ListingSold` then updates
  mirrors.
- **"Convert and buy" is a client feature.** The UI can offer one button
  that first takes the best local quote for the seller's currency and then
  sends the purchase. These are two separate atomic bank transactions. If the
  second one fails or is returned (listing sold, price changed), the member
  keeps the foreign coin they bought, and the UI says so and offers to sell
  it back. The bank doesn't try to make the pair atomic.
- **Offers and haggling across the border** come later: offers would be
  messages to the listing's bank, which accepts or declines them there.
- **Undo window and disputes** follow the seller's bank rules. Reversal is a
  refund from the seller's bank (see "Returns and refunds").

## Tracking foreign trade

Every `CreditTransfer` carries a `purpose`: `Purchase`, `Gift`, `Transfer`,
`Exchange`, `Refund`, `Swap`, `Interest`, `Settlement`. Each bank derives a **balance of
payments** from its own journal:

| Line | From |
|---|---|
| Exports | Foreign purchases of local listings (inbound `Purchase`). |
| Imports | Local members buying abroad (outbound `Purchase`). |
| Transfers | Gifts and plain transfers, in and out. |
| Financial | Swaps, exchange of currencies, credit drawn or repaid. |
| Position per peer | `nostro` and `vostro` balances, credit used against the limit. |
| Foreign coin held | Per member and in total, valued at the latest local rate. |
| Exchange rate | Last trade per pair over time. |

These are new flow counters per peer (like `Epoch.flows`), so lifetime
totals survive history eviction. A "Foreign trade" page reads them.

## Reconciliation

- Every K messages (for example 32) or once a local day, each bank sends a
  signed `Statement`: "after applying your messages up to X and sending mine
  up to Y, your account here holds P and my account with you should hold Q."
- The receiver compares that with its own books, adjusted for messages it
  knows are still in transit. A match is recorded. A mismatch **freezes**
  the link: no new outbound money, returns still flow. Both Nanas see the
  difference on a Reconciliation page.
- Statements are kept (bounded, the last few per peer) as evidence for both
  households.

## Winding down

A link can be wound down by either bank's Nana (`WindDown`), or after it has
been frozen and the Nanas choose to close it.

1. **Freeze:** no new payments. In-transit messages are still delivered or
   returned. Mirrors and quote snapshots are dropped.
2. **Local buy-back:** each bank's Nana buys back all members' foreign coin
   at a final rate she posts, paying in local coin. Now only the banks hold
   each other's currency.
   *Or* members sell foreign coin for recorded USD, if they prefer dollars.
3. **Netting:** the banks exchange final statements and cancel what they
   owe each other at an agreed rate, as a matching pair of `Settlement`
   transfers.
4. **Outside payment:** any remainder is paid off-system, normally in real
   dollars. Each bank records it (a USD leg on each side), and it's a
   journaled, visible settlement.
5. **Close:** final statements on both sides show zero, and the link is
   `Closed`. The peer key is kept (read-only) so old records stay
   verifiable.

If a peer disappears for good (board lost, never comes back), a bank can
close unilaterally: the Nana writes off the position against the peer with
an explicit loss entry, and members' foreign coin is bought back or written
off by Nana's choice. That's a visible event, not a silent edit.

## Bounds

| Bound | S3 | S2 |
|---|---:|---:|
| Paired peers | 4 | 2 |
| Outbox (unacked messages) per link | 64 | 16 |
| Sending window W | 16 | 4 |
| Inbound messages applied per scheduler turn | 4 | 1 |
| Exported listings | 16 | 8 |
| Mirrored foreign listings per peer | 16 | 8 |
| Statements kept per peer | 8 | 4 |
| Envelope / snapshot size | 512 B / 2 KiB | 512 B / 2 KiB |

Every bound has a defined behaviour when full: the outbox refuses new work
("link busy"), mirrors drop the oldest entries, statements keep the latest.

## Failure cases

| Situation | Result |
|---|---|
| Receiver offline | Broker queues; sender's items show "in transit". |
| Sender restarts after debiting | Replay rebuilds the outbox; the message is sent again. |
| Message delivered twice | Seq ≤ `applied_in`: acknowledged, not applied. |
| Message lost | Receiver waits for the gap; sender resends everything after `acked_out`. |
| Receiver restarts mid-apply | The journal event is atomic: either applied with `applied_in`, or not at all. |
| Storage error at receiver (bank latched read-only) | Nothing applied; broker keeps the message. |
| Payee unknown, cap exceeded, credit floor reached | `Return`, money back to the payer. |
| Peer's clock wrong | Offset shown; outbound money paused beyond 5 minutes. |
| Conflicting content for one seq | Link freezes. |
| Statement mismatch | Link freezes; Reconciliation page for both Nanas. |
| Peer reset (new key) | Old link can't verify new messages; wind down or unilateral close. |
| Relay or broker compromised | Can delay, drop or read messages; can't forge them. |

## Testing

- The wire module has golden vectors and round-trip property tests.
- Two desktop banks with an in-memory transport, plus fault injection: drop,
  duplicate, reorder, delay, and crash either side between any two steps.
  The property checked after every run: each bank's books add up, and the
  two banks' positions reconcile once the queue is empty.
- Everything passes `make test` and `make test-s2`, and fits the S2 image
  (2.14 MB of a 2.49 MB partition today).
- Hardware: S3 and S2 on the LAN with the Pi post office.

## Phases

0. **Foundations:** asset IDs instead of the USD flag; reserved correspondent
   account IDs; generalize the quote book to asset pairs. Useful by itself.
1. **Identity:** bank keys, bank card endpoint, pairing screens,
   fingerprints. No money.
2. **Post office:** Mosquitto plus the relay (T2) on the Pi. Card exchange,
   `AckOnly`, clock-offset display. Still no money.
3. **Payments:** `CreditTransfer`, `Return`, statements, freeze, on desktop
   first with fault injection, then on the boards.
4. **Foreign coin:** member foreign wallets, the opening purchase, credit limits and overdraft interest, local
   order books per pair, quote snapshots.
5. **Cross-border listings:** snapshots, mirrors, purchases, refunds.
6. **Foreign trade page:** per-peer flows and balance of payments.
7. **Wind-down** tools and the unilateral close.
8. **Browser push** from the post office.
9. **Far away:** broker bridging between households, then optionally T1 on
   the S3, and workers on the post office.

## Open questions

1. For far-away households: one shared internet broker, or each post office
   bridging to each peer? This decides who operates what.
2. Should the per-link value caps also apply per member (so one member
   can't use up the household's daily cap), or only per link?
3. What overdraft interest rate should be the default when a link is
   paired, and can it change while a balance is negative?

Answered September 29: the post office is a shared $35 Pi; overdraft
interest is paid in the lending bank's currency; every member may trade
internationally; banks open with an opening purchase of each other's
currency; currency conversion before a purchase is the client's job.
