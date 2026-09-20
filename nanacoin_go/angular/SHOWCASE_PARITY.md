# One client, explicit capability differences

This extends the existing `demo` Angular configuration and
`.github/workflows/nanacoin-pages.yml`, not a second website. `nanacoin_web`
serves this same Angular client from the legacy MicroPython board; Rust bundles
it into firmware. TinyGo firmware remains frozen under `nanacoin/AGENTS.md`.

| Feature | Static browser demo | Rust API | TinyGo API |
| --- | --- | --- | --- |
| About, comic link, vegan/vegetarian recipes | Public, no login | Same shared client | Same shared client |
| Cursive/spiral presentation | Public fictional ledger and authenticated history | Existing authorized ledger/history | Existing authorized ledger/history |
| Diagnostics | Real browser-reported health, no API fetch | Actual ESP32 measurements | Supported single-core diagnostic subset |
| Public full ledger | Fictional tab-local data, latest 100 | Remains Nana-only; no new public data exposure | Remains existing permissions |
| Foreign exchange | Tab-local USD balances, quotes and atomic two-leg settlement | Implemented: USD issuance, quotes and settlement | Unsupported |
| Bearer nana-nickles | Prototype: member-funded reserve or Nana fresh issuance, QR/text voucher, single redemption | Not implemented; see `nanacoin_rs/NANANICKLES_PROPOSAL.md` | Not implemented; firmware frozen |
| HTTP/HTTPS household policy | Hidden; no board policy to change | Implemented | Unsupported |

Never present a mock ESP32 reading as a browser measurement. Browser memory is
optional/non-standard; unavailable is not zero. Browser health does not estimate
CPU load, physical RAM, flash wear or electricity. Economic demo data is seeded
fiction and resets on reload. Voucher secrets only work within that same tab.

## Publishing the existing demo

From `nanacoin/angular`, Git Bash:

```bash
npm ci
npm test -- --watch=false
MSYS_NO_PATHCONV=1 npx ng build --configuration demo --base-href /microcontroller/nanacoin/
uv run --with playwright python scripts/showcase-check.py
```

The environment flag prevents Git Bash from turning `/microcontroller/...` into
a Windows filesystem path. On Linux omit the flag if desired and install the
Playwright Chromium browser. The browser check uses installed Edge on Windows.
It blocks all external/API requests, tests anonymous pages, fonts, mobile width,
browser health, and QR voucher creation/printing/redemption/replay.

The existing Pages workflow publishes on a matching push to `main` or manual
dispatch of a committed ref. Local edits are not published by running a workflow
against an older ref. Build artifacts remain ignored. Do not upload ignored
board configs, certificate keys, journal files, or an ESP32 firmware binary.

This site links to SMBC; it does not redistribute the comic. The locally bundled
Dancing Script font includes its SIL OFL license in `public/fonts/OFL.txt`.
No runtime Google Fonts request is required. Energy figures are labelled planning
assumptions, with a dated Cambridge Bitcoin estimate—not live energy telemetry.

## Navigation and keyboard conventions

One horizontal menu bar is shared by all builds; at 760 CSS pixels and below it
becomes a hamburger-activated vertical flyout. Escape restores toggle focus;
navigation, outside click and leaving the menu close it. Ordinary links/buttons
retain Tab/Enter/Space behavior. The active page has `aria-current="page"`.

The footer's **? for keyboard help** opens a native modal dialog. The analogous
Mawkingbird patterns are `?`, `g` then `h`/`e` (Market), `g` then `l` (notebook),
`j`/`k`/`0` (records), Alt+PageDown/PageUp, and `n` (Market's listing title).
Navigation sequences expire after one second. Forms, composition, repeat events,
browser modifiers, and confirmation dialogs suppress app shortcuts. Users may
disable them; the footer help button and standard controls remain usable.

Interaction reference: `C:/github/mimb/mawkingbird/ui/src/app/hotkeys.ts`,
`shortcut-help/shortcut-help.ts`, and the shell's outside-click/Escape pattern.
These are adapted conventions, not a claim that all Mastodon keys exist here.
No quick-send or financial-action shortcuts are assigned.
