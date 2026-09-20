# NanaCoin's domain

NanaCoin models a household currency administered by a trusted person called
Nana. It is not cryptocurrency: there is no mining, blockchain, distributed
consensus or external wallet network. One server decides which operations are
valid and records their effects.

## People, accounts and authority

A user is an identity that can log in. An account is where that person's
postings accumulate. Keeping those concepts separate makes authorization and
accounting explicit rather than treating a login record as a mutable balance.

The first provisioning operation establishes the household and Nana. Nana can
add members, administer their status, issue or retire coins, and reverse
eligible transactions. Ordinary members can use the actions their role and
status permit, including transfers and marketplace participation. These checks
live on the server; hiding a button in Angular is not authorization.

Current board state is temporary. After reboot, the household must be
provisioned again. The initial provisioning endpoint therefore deserves care
even on a development LAN: the first caller establishes the administrator.

## Money is integer postings

Amounts are whole-number units stored as integers, not floating-point values.
Money-moving inputs must be positive and obey the transaction amount limit
(currently 1,000,000,000). Each transaction has exactly two postings whose sum
is zero.

| Operation for 10 coins | First posting | Second posting | Circulation |
|---|---|---|---|
| Nana issues to Alice | System issuance: -10 | Alice: +10 | Increases by 10 |
| Alice pays Bob | Alice: -10 | Bob: +10 | Unchanged |
| Nana retires Bob's coins | Bob: -10 | System issuance: +10 | Decreases by 10 |

The system issuance account is deliberately allowed a negative balance. Its
opposite is the issued money held elsewhere. Ordinary accounts cannot spend
funds they do not have.

Transactions are immutable economic facts. Current balances remain valid
when old transactions leave the RAM history window because the ledger carries
forward opening-balance checkpoints. Summing just the visible recent history
is therefore not sufficient to reconstruct a lifetime balance.

## Corrections append another transaction

A reversal adds opposite postings and records which original transaction it
reverses. It does not edit the old entry. The service prevents repeated
reversals and checks whether the operation remains valid; for example, the
recipient may already have spent the money needed to reverse a payment.

The original transaction must still be available for the operation. A bounded
history is not a permanent audit archive. Transaction IDs use monotonically
increasing sequence numbers such as `txn-41`; recycling a storage slot does
not recycle its identity.

These IDs are names, not secrets. Server-side authorization controls who may
read a transaction. Authentication tokens and authorization codes have a
different requirement: they must be unpredictable.

## Listings and purchases

A listing describes an item, service or currency offer at a NanaCoin price.
An active listing may be canceled by an authorized action or purchased by an
eligible buyer. The owner cannot buy their own listing.

A purchase coordinates two changes: the payment from buyer to seller, and the
listing becoming sold. They form one domain event under the service lock, so
two buyers cannot both successfully purchase the same active offer. Insufficient
funds must leave both the balance and the listing unchanged.

Reversing the payment does not automatically reopen the listing. That is a
separate business decision. A listing offering physical cash also does not
make the server a foreign-exchange settlement system; it describes an offer
whose real-world delivery remains outside the ledger.

Live users, accounts and active listings are not disposable history. Capacity
pressure must not silently erase them. Closed listings can be reclaimed under
the store's retention rules.

## Input limits have economic and memory meanings

The current limits include 40 bytes for names, 80 for listing titles, 500 for
descriptions and 140 for transaction memos. These are UTF-8 byte limits, not
a count of letters displayed on screen. The shared text arena and encoded
HTTP body impose additional limits; see [memory](memory.md).

Tests should check business properties: every transaction balances, ordinary
accounts remain nonnegative, failed operations leave state unchanged, and
concurrent purchases have at most one winner. Merely asserting a particular
JSON response is not enough to establish those properties.

## Authentication and sessions

The browser uses an authorization-code flow with PKCE S256. It starts with a
password and challenge, receives a short-lived code, then exchanges that code
with the matching verifier for a bearer token. Subsequent requests send the
token in the `Authorization` header. PKCE protects the code exchange; it does
not encrypt HTTP traffic.

The server stores password verifiers and hashes of session tokens rather than
raw bearer tokens. Password checking uses PBKDF2-HMAC-SHA256 with an embedded
work factor; this is a resource trade for this project, not general guidance
for an internet-facing password service. Login failure tracking is bounded,
and rate limiting avoids repeatedly performing expensive work for a locked
account.

The board configures 32 session slots, 8 authorization-code slots, and 16
failure-tracking slots. Its access-token lifetime is four hours. Sessions are
RAM-only and disappear on reboot or logout. These counts are not the HTTP
worker count: many logged-in people can share a few request workers over time.

The current development deployment uses plain HTTP on the LAN. CORS controls
browser access behavior, not caller identity or transport secrecy. Do not
treat PKCE, an allowed origin or a private IP address as a substitute for TLS.

Loans, interest and idle-money tax are roadmap ideas, not implemented economic
features. Their eventual rules should become explicit service operations and
tests rather than browser-side balance adjustments.
