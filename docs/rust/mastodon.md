# Mastodon integration

NanaCoin's first Mastodon integration is browser-to-Mastodon. The Rust server
stores one bounded, public-to-the-household Mastodon handle per member; it never
receives an OAuth client secret or access token. Each NanaCoin member connects
their own Mastodon account from the Send screen, and that browser stores the
result under the member's NanaCoin ID.

## Login and storage

The client follows Mawkingbird's dynamic-app PKCE flow:

1. Normalize the member-selected instance and register a NanaCoin app with
   `read:accounts write:statuses`.
2. Generate cryptographically random `state` and a PKCE verifier, retain them
   in `sessionStorage`, and redirect with the S256 challenge.
3. Consume and validate the pending attempt after the callback, exchange the
   code, then call `verify_credentials`.
4. Save the token and app registration in browser `localStorage`, scoped by
   NanaCoin member ID. Save only the canonical `@user@server` handle through
   the Rust member API.

Nana can also enter or correct a handle while creating or managing a household
member. A member may update their own handle but not anybody else's.

## Message boundary

Messages are Mastodon statuses with `visibility=direct`. The client always
prepends a recipient handle taken from the authenticated household member list;
there is no free-form recipient field and no public/unlisted option.

The Send screen supports a DM without a coin amount, or a transfer plus an
optional DM. Offer owners and forex takers can opt into a generated DM to the
other party after settlement. Message failure never rolls back or disguises a
completed financial operation.

ALL CAPS is client-only. Checking saves the current draft and uppercases it;
unchecking restores the saved draft. Generated settlement messages apply caps
only at send time.

## Deferred subscriptions

Subscribing to every offer and forex offer remains roadmap work. It needs an
explicit opt-in and a delivery design that covers deduplication, reconnect and
backoff, token revocation, and whether notifications require an open browser.
No polling or public-server load is part of the first integration.

Facebook sharing is the first own-account intent in Send. Other network intents
are deferred; none of them can act as a NanaCoin DM recipient bypass.
