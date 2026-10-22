# Household email

The Rust server and Angular client support email on the **Email** page:

- Verify a private address using the current password and an emailed code.
- Recover a forgotten password using a previously verified address. The sign-in
  form links to recovery. Codes expire after 15 minutes, are single use, and
  become invalid after a password change or server restart. Resetting a password
  uses the existing durable account command and revokes sessions and API keys.
- Send a deliberate message to a household member who has enabled notifications.
  Routine payments and other ledger events never generate email.
- Email household invitations. These give directions to the bank; Nana still
  creates accounts. An invitation does not grant access.

Email routes and navigation are omitted from the web demo, and the Angular API
methods reject email operations in demo builds before making a network request.
The API key is never part of the Angular bundle.

## Configuration

`NANACOIN_BREVO_API_KEY` selects the initial Brevo provider.

- **Desktop:** email is enabled only when `NANACOIN_BREVO_API_KEY` is set in the
  environment when the server starts. Desktop builds never embed a key, so an
  ordinary `cargo run` or a copied binary cannot send real email.
- **Firmware:** the board has no runtime environment, so ESP32 builds embed the
  key. Build settings also accept `GAMEMATCHER_BREVO_API_KEY`, `BREVO_API_KEY`,
  and `BREVO_OTHER_KEY`, and search the crate and repository `.env` files,
  `../microcontroller/gamematcher/api/.env` and `.env` (relative to the
  repository), `MINIFRAMEWORK_ENV`, and the shared household `C:\github\.env`.
  Build environment values take precedence. Treat firmware images as secrets.

Optionally set `NANACOIN_MAIL_FROM` (aliases `GAMEMATCHER_MAIL_FROM` and
`BREVO_SENDER`) to a verified sender address. Otherwise Brevo's first active
sender is used, with the display name **NanaCoin**. Without a configured key the
server does not enable email. The provider checks the sender when delivering;
configuration alone does not prove that the key or sender is valid.

Transport is behind `email::Provider`; future SMTP providers implement `send`
and `name` and are supplied to `Mailer::start`. Account workflows and Angular
requests are independent of Brevo. SMTP is not implemented yet.

Brevo API reference: <https://developers.brevo.com/reference/send-transac-email>.

## Delivery and storage

Email uses a separate background thread and never performs provider network I/O
under the ledger lock. The desktop uses HTTPS with public certificate roots;
ESP32 uses the framework's public-CA HTTPS client.

Preferences and a bounded queue are private KV data: `<journal path>.email/` on
desktop and the `nc-email` NVS namespace on boards. Keep them private alongside
the journal; queued recovery emails contain their plaintext code. Verified
addresses are not added to public member profiles or the financial journal.
Email requests require human sessions; API keys cannot change recovery settings
or send email. Recovery requests return the same accepted response for unknown
and unverified accounts.

There are at most 16 queued messages, a 15-minute queue lifetime, a persisted
100-email daily allowance, and a one-minute sending cooldown per signed-in
member. Unauthenticated recovery requests are limited to one per account every
five minutes and to 20 of the daily 100, so a stranger cannot exhaust the
allowance members use or block recovery for other accounts. These are per bank,
not a shared allowance across NanaCoin boards or GameMatcher. Member emails can
be turned off without disabling future recovery requests. Removing an address
also invalidates its recovery challenges. Turning off notifications/removing
an address drops queued messages to that address; an already claimed send may
still finish.

Delivery is **at most one attempt**, not guaranteed delivery. Jobs are removed
from durable storage before calling the provider. A crash in that window can
lose a message, and ambiguous network failures are not retried automatically.
The user may request another code or resend after the cooldown. Queued is not
delivered; Nana can see the most recent provider error on the Email page.
Recovery and verification challenges themselves stay in RAM, so old queued
codes cannot recover accounts after a restart.

## Tests

`tests/email.rs` drives `Mailer` against an in-memory journal and KV store, a
recording `Provider`, and a test clock. It covers verification, recovery and
reset (single use, session revocation, invalidation by password change, expiry
and restart), identical recovery responses, API-key rejection, preferences and
removal, invitations, cooldowns, the daily and recovery allowances across a
restart, queue bound and expiry, at-most-once delivery, Nana-only error
display, and address validation. A fake `Fetch` checks the Brevo requests,
sender selection and every failure message. `src/app/pages/email.spec.ts`
covers the Angular page's requests and secret-field clearing.

## Validation status (October 19, 2026)

`make check` passes: formatting, strict clippy (default and S2), all Rust tests,
S2 tests, and the HTTP smoke test. Angular `ng build`, `test:ci` and `test:demo`
pass. A 16 KiB worker thread was measured to overflow the stack on a desktop
debug HTTPS request, so the desktop worker uses 256 KiB.

On boards the worker keeps an 8 KiB internal-RAM stack (it writes NVS) and each
send runs on a 48 KiB PSRAM-stack helper; a resident 24 KiB internal stack
starved the S3's serving connections. The S3 bank runs this build and passed
the strict probe (see DEPLOY.md, October 19, 2026).

Still unverified: real Brevo delivery from either target (no real email has been
sent), and the PSRAM helper's stack depth under a real TLS handshake. The S2
firmware no longer fits its application partition and is not deployable.
