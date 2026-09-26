# The public demo

The NanaCoin client with an in-memory ledger behind it, so the site can be
shown to someone without a board, a network, or a server process anywhere.

```powershell
npm start -- --configuration demo     # locally
npx ng build --configuration demo     # what CI publishes
```

Published by `.github/workflows/nanacoin-pages.yml` to `/nanacoin/` on GitHub Pages.

## Seeded examples

Nana opens with **1,000 NC and $100 in recorded dollar reserves**. The sample
lotto interest and garden-tool loan subsequently use some of those coins;
the central-bank report reconciles the opening issuance with current cash,
loan principal and interest. Open Nana buy/sell quotes demonstrate reserve
coverage. Choose Nana at login and open **Nana as Central Bank**; select the
full history period to see the opening issuance.

The Notebook includes partial/full corrections, watercolor gift contributions,
a digital-art sale, loan principal and interest, lotto principal and interest,
and both sides of forex trades. Dollar records retain cents when NC is reformed.
Paginated history reaches the opening reserves. Original transaction postings
remain unchanged by reform; views project NC into current units.

The demo supports `/commerce`, `/commerce/commands`, and transaction `/refund`
through its local interceptor. It seeds an open gift request, purchased/equipped
art, art gifted to Nana, and an edition for sale. These features remain API-only;
their payments are visible in the Notebook. Art URLs/digests are fictional
fixture metadata; no remote art is fetched. Reloading resets the entire demo.

Run the actual static showcase (including 320–1024px Loans checks) with:

```powershell
npm run build -- --configuration demo --base-href /nanacoin/
uv run --with playwright python scripts/showcase-check.py
```

## How it works

An **HTTP interceptor**, not a mock service. Swapping `NanacoinService` for a
fake would demo the fake; this way every line of the real client runs
unchanged — the same PKCE exchange, the same idempotency keys, the same error
mapping, the same signals — and only the wire is different. If the demo works,
the client works.

```text
  the real Angular client
          |
     HttpClient
          |
    demoBackend  ← interceptor: never reaches the network
          |
     DemoLedger  ← the same rules, in TypeScript
```

| File | What it is |
|---|---|
| `ledger.ts` | the economic rules: double-entry, issuance, reversals, offers |
| `demo-backend.ts` | the interceptor, routing URLs to ledger calls |
| `seed.ts` | eight weeks of a household, spread over real days |
| `demo.ts` / `demo.public.ts` | the build flag, swapped by `fileReplacements` |

## What it copies, and what it does not

It copies the **rules**. Money is an append-only list of double-entry
transactions; a balance is a fold over postings; issuance comes from a
distinguished account allowed to go negative; corrections are mirror
transactions rather than edits. It implements the corresponding Rust service
rules for the supported demo operations. `ledger.spec.ts` tests those, including that the book
balances after every operation.

It does not implement binary persistence, flash archives or crash recovery.
History remains in the tab; some business limits (including commerce capacities)
match the server. Use Rust and board tests for storage behavior.

Two other deliberate differences:

**Login is a list of people.** No passwords: the household is made up, it
lives in one tab, and asking a visitor to type a password printed beside the
box would be a puzzle rather than a demonstration. The client still runs the
real PKCE exchange; the backend just accepts any password.

**Passwords are stored in plain text** in `DemoLedger`, which is correct here.
The real server uses PBKDF2 because it holds real credentials. Hashing four
made-up accounts would be theatre, and would imply the demo is somewhere
secrets could be kept.

## Keeping it out of the ordinary build

`IS_DEMO` is a compile-time constant, so `ng build` tree-shakes the ledger,
the seed and the interceptor away entirely — checked by grepping the output
for seed text that exists only in the demo build. The board pays nothing for
the demo existing.

The one thing that does survive is the login screen's demo wording: it lives
in a template branch that is never taken, and Angular cannot shake a string
out of a template. About a hundred bytes.

That is also why the workflow's verification greps for `Monthly household
float` rather than anything on the login screen — the login text would pass
in either build, which would make the check worthless exactly when it matters.
