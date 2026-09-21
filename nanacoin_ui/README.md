# NanaCoin — Angular client

The household-facing site. A static Angular app that talks to the NanaCoin
HTTP API, wherever that happens to be running.

This is the client meant for daily use. `../nanacoin_go/web/` holds an earlier
vanilla TypeScript client with the same design; it is kept because it is small
enough to serve off the TinyGo board itself.

## Run it

Two processes: the Rust API, and the dev server.

```powershell
# terminal 1 - the API
cd ../nanacoin_rs
make run

# terminal 2 - this site
npm install     # once
npm start
```

Then open <http://localhost:4200/?api=> to use the local Rust API through the dev
proxy. Without that parameter, the default is the TinyGo board at
`http://nanacoin-api.local`. With the API running you land on the setup
screen, which asks you to create the household and become Nana. Without it you
get "Where is NanaCoin?" instead — see below.

`npm start` proxies `/api` to `localhost:8080` (see `proxy.conf.mjs`), so the
site and the API look same-origin during development and CORS never comes up.

```powershell
npm run build     # production bundle into dist/
npm test          # unit tests, vitest
```

## Pointing it at the board

The site does not have to be served by the thing it talks to — that is the
whole point of the API server's CORS configuration.

**The default is `nanacoin-api.local`.** No address needs to be entered when
the TinyGo board is reachable by mDNS on the same LAN. A previously saved
server remains an override; use `?api=nanacoin-api.local` to replace it.

**The site asks if it cannot connect.** If NanaCoin cannot be reached, the first screen is "Where is
NanaCoin?" with an address field. Type what the board printed —
`192.168.1.158` — and press Connect. The scheme and the `/api/v1` suffix are
filled in, the address is verified against `/status` before being accepted, and
it is remembered, so this happens once rather than every visit. Once connected,
the footer shows where requests go and offers **Change server**.

That is the only route a household member needs. The rest are conveniences:

**A query parameter**, the quickest thing to paste into a chat or a terminal:

```text
http://localhost:4200/?api=192.168.1.158
```

Bare host, host with a scheme, host with a port, or a full `.../api/v1` all
work. An empty `?api=` clears the saved address and uses the page's own origin
for that visit. Removing the parameter restores the deployment default.

**A meta tag** in `src/index.html`, for a static deployment baked with a known
address:

```html
<meta name="nanacoin-api" content="http://nanacoin-api.local" />
```

**Nothing**, which uses the meta tag's default of `nanacoin-api.local`.

Precedence is query parameter, then remembered choice, then meta tag, then
same-origin if the meta tag is empty or absent.

Whichever you use, the server's allowed origins must include wherever this site
is served from, or the browser blocks the request before it is sent. The Rust
desktop server uses `NANACOIN_ORIGINS`; its defaults already include the local
Angular development origin. For example:

```powershell
$env:NANACOIN_ORIGINS = "http://localhost:4200"
cd ../nanacoin_rs
make run
```

## How it is put together

```text
src/app/
    api/
        models.ts             wire types for the server JSON API
        nanacoin.service.ts   the HTTP client, PKCE, idempotency keys
        api-base.ts           which NanaCoin to talk to, and remembering it
        session.ts            who is logged in, and the derived state
    pages/
        connect-form.ts       "Where is NanaCoin?" - the address screen
        logs.ts/.html         server logs, for diagnosing the board
        setup-form.ts         first run: become Nana
        login-form.ts         PKCE login
        market.ts/.html       listings, buying, selling
        send.ts               transfers
        history.ts            your own transactions
        nana.ts/.html         household, issuance, the full ledger
    ui/
        toasts.ts             outcome messages
        toast-list.ts
    app.ts/.html              the shell: setup / login / app
```

Angular 22, standalone components, signals throughout, and
`provideZonelessChangeDetection` — there is no zone.js in the bundle. Routes
are lazy, so Nana's admin screens (the largest part, and the part most
household members never open) are not in the initial download.

Around 123 kB transferred for the initial load.

### Things worth knowing

**Hash routing.** `withHashLocation()`, so this can be dropped on any static
host — including a plain file server with no rewrite rules — without `/market`
404ing on refresh.

**The server is authoritative.** Nothing here decides whether a transaction is
valid, what a balance is, or who may do what. `Session.refresh()` re-reads
everything after each mutation instead of patching state locally, which is
both simpler and correct when someone else in the house is clicking at the
same time. Hiding the Household tab from ordinary users is a courtesy; the
server enforces every rule regardless.

**Idempotency keys are made once per operation**, before the first attempt, and
reused on retry — that is what makes them work. A key generated per *request*
would defeat the entire mechanism. See `newIdempotencyKey` and its callers.

**Tokens live in `sessionStorage`**, not `localStorage`: on a shared household
computer, closing the tab should end the session. Every access is wrapped,
because private browsing and blocked site data both throw.

**Money is always integers.** There is no fractional NanaCoin, and the forms
reject anything that is not a whole positive number before it reaches the
server (which rejects it again).

**A gateway error is reported as unreachable.** A 502/503/504 comes from a
proxy answering on NanaCoin's behalf because it could not reach it — the dev
server does exactly this when the API server is not running. "Bad Gateway" tells
a household member nothing, so those statuses and a status of 0 are all
reported as "could not reach NanaCoin at <host>", which is both true and
actionable.

**The Logs page needs no session.** It is reachable from the tab bar and from
the connect screen, because the failure most worth diagnosing is the one that
stops you logging in. See `/api/v1/logs`.

**The API base is a signal, read per request.** It has to be changeable at
runtime, which a bootstrap-time injection token could not be — so `ApiBase` is
a plain root service and `NanacoinService` reads `current()` on every call. No
reload is needed after changing the server.
