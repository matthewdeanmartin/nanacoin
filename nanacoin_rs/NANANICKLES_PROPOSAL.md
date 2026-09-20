# Nana-nickles — live Rust proposal, not implemented firmware

The static Angular demo contains a deliberately tab-local prototype. This file
does not claim that real Rust or TinyGo accounts support bearer vouchers yet.
The comic is the inspiration, not a complete cryptographic/protocol specification.

## Money model

Any active member can package **existing** coins into a voucher. Atomically debit
their spendable account and credit a dedicated bearer reserve. Circulation does
not change. Nana may instead create new money: debit system issuance, credit the
reserve, increasing circulation. These must be distinct operations/confirmations.

On redemption, debit reserve and credit the authenticated holder's account in
one durable event. The secret grants the right to those coins, not access to the
issuer's account. Printing/displaying a voucher is only another representation
of the same claim. No offline reprinting or signature can create extra value.

Invariant: bearer-reserve balance equals the sum of all outstanding voucher face
values. Normal transfer/issue/reversal APIs must not independently alter that
reserve. Issuing vouchers is not spending the same coins twice.

## Proposed security boundary

- Separate token format from login tokens; include version, economy generation,
  public serial and 32 bytes from the OS/ESP-IDF CSPRNG. Never Math.random.
- Keep a SHA-256 digest of the high-entropy secret, denomination and lifecycle
  state, not the plaintext token. A password work factor is unnecessary for a
  uniformly random 256-bit secret. Compare digests in constant time.
- Production issuance/redemption requires trusted HTTPS, **and household Secure
  mode**, so a login stolen from a parallel HTTP session cannot spend the account.
  Re-authenticate for sensitive issuance if appropriate. Public static demo has
  no security boundary: visitors control its JavaScript and storage.
- Never put secrets in URL/query/hash, analytics, ledger descriptions, errors,
  clipboard automatically, localStorage, or exported diagnostics. QR codes should
  encode the voucher, not a GET redemption URL. Use a no-store response and a
  dedicated screen with deliberate show/print/copy actions and no third-party JS.
- Decode and validate bounded input before lookup. Rate-limit by account and
  connection without creating an unbounded address map. Generic invalid/spent
  errors; do not expose hashes in the public ledger.
- Hold the existing service mutex across lookup, funds validation, durable
  journal append and state change. Persist consumption and postings in the same
  event. After a lost response, the same authenticated recipient/request receipt
  can recover the result without redeeming again. No blind mint retries.
- A lost mint response is especially important: an unacknowledged reserve must
  not leave money trapped. Prefer a client-generated secret with the server
  committing its digest and idempotent public receipt, or explicitly design a
  bounded encrypted retrieval scheme. Do not persist plaintext just to solve it.
- Checkpoints must preserve every outstanding voucher and retry receipt; never
  evict live liability to make space. Use a preallocated bounded table, explicit
  capacity errors, fixed-size journal records and the existing durability rules.
- Economy reset rotates the generation and invalidates old vouchers explicitly.
  Ordinary history compaction must not resurrect spent vouchers. Unknown old
  vouchers fail closed even after their display history is retired.

## The unavoidable bearer tradeoff

Anyone with a copy can race to redeem first. A printed QR is copyable; a digital
signature proves issuance, not uniqueness or possession. Safe hand-off requires
online redemption or atomic redemption/reissue to a fresh secret controlled by
the recipient. A paper note that changes hands repeatedly without contacting
Nana cannot provide assured double-spend prevention.

Keep denominations small. A scratch-off/sealed secret may deter casual copying
but is not cryptographic proof. Screenshots, printer spools and cloud print queues
can leak it. The issuer may have retained a copy even after handing over paper.

Before live rollout decide lost-token, expiration, cancellation and recovery
rules. Unilateral issuer cancellation would make a bearer note revocable and
let its issuer cheat after hand-off. The demo intentionally offers no cancellation,
expiration or secret recovery; reload destroys its entire fictional economy.

## Required tests before a real release

Concurrent redemption; insufficient funds; ordinary-member fresh issuance denied;
disabled accounts; exhausted capacity; failed/ambiguous flash writes at every
boundary; power loss/replay; checkpoint and reset; double-spend after history
retirement; idempotent lost responses; forged/oversized tokens; full secret-log
redaction; HTTPS policy; reserve invariant; printed/mobile accessibility.

The static prototype tests reserve accounting, Nana-only fresh issuance,
single-use redemption, rejected independent reversals, and the 128-outstanding
voucher cap. It is not evidence of embedded durability or resistance to a visitor
modifying their own browser.

Security background: [OWASP token lifecycle guidance](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html)
and [session handling/XSS limitations](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).
