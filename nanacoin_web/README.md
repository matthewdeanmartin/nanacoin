# nanacoin_web

## Public static showcase

The existing GitHub Pages demo is built from the same client at
`../nanacoin_go/angular` using its `demo` configuration. This folder is the legacy
MicroPython static-file host, not a separate frontend implementation. See
[showcase build and capability parity](../nanacoin_go/angular/SHOWCASE_PARITY.md).
About, lemon-bar recipes and notebook styling are shared. The static demo uses
fictional financial data and real browser health; it never needs a live board.
Nana-nickles are currently a labelled browser prototype, not a Rust/TinyGo API.

The NanaCoin site, served off an ESP32-S2 Mini.

Two boards, one household currency:

```text
   ESP32-S2 Mini                    ESP32-S3-N16R8
   4MB flash, 2MB PSRAM             16MB flash, 8MB PSRAM
   MicroPython                      Rust (or TinyGo)
   http://nanacoin.local/           https://nanacoin-rs.local/api/v1
   serves the Angular bundle        the ledger, the API, the money
          \                                    /
           \                                  /
            `------->  the browser  <--------'
                    loads from one,
                    talks to the other
```

The two boards never contact each other. The browser fetches the page from the
S2 and then makes its API calls to the S3 directly.

## Why two boards

They have opposite shapes, and each job wants one of them.

Serving files needs **flash and almost no compute** - bytes are copied off the
filesystem to a socket. The S2 has 4MB of flash and one slow core, which is
exactly enough.

NanaCoin needs **RAM**. Its ledger, bounded arrays and request workers were
sized against 8MB of PSRAM, and its 365-transaction history window and event
ring are memory budgets. None of that fits in the S2's 2MB.

See [`BOARD_SKILL_ESP32_S2_MINI.md`](../BOARD_SKILL_ESP32_S2_MINI.md) for the
full comparison, and for why this board's USB port moves on every reset.

## Setup

Once per board:

```powershell
copy config_example.py config.py     # then add your WiFi details
.\flash_micropython.ps1 -Port COM4   # erases the board, installs MicroPython
```

Then, every time the site changes:

```powershell
.\deploy.ps1 -Port COM4
```

That builds the Angular client, gzips it, wipes `/www` and copies it over.
The board's port moves on every reset - it has no bridge chip - so find it
rather than assuming:

```powershell
[System.IO.Ports.SerialPort]::GetPortNames()
```

`COM3` is usually the motherboard's own serial port. The one that *appears*
when you plug the board in is the board.

## Pointing the site at the API

The site needs to know where the NanaCoin board is. Once, per device:

```text
http://nanacoin.local/?api=192.168.1.158
```

The client remembers it in `localStorage`, so it is a one-time step per
browser. Without the parameter the site asks for the address on a connect
screen, which is the same thing with more typing.

The web board is `nanacoin.local`; Rust uses `https://nanacoin-rs.local` and
TinyGo uses `http://nanacoin-api.local`. After an unreachable or non-API
startup response, Angular searches both HTTP and HTTPS at these API names,
the saved/deployment addresses, and the documented board addresses
`192.168.1.158` and `192.168.1.157`. The connect screen also has a search
button with progress and cancellation. It probes sequentially with a
three-second timeout, validates NanaCoin's public status shape, and saves
only a successful candidate. No bearer token is sent during discovery.
This is a fixed candidate list, not a subnet scan; DHCP can still require
entering a new address manually.

## HTTP site, HTTP or HTTPS API

The MicroPython web board serves HTTP. It can call either TinyGo's HTTP API
or Rust's HTTPS API, subject to certificate trust, CORS and browser local
network permission. Rust's development certificate must be trusted and
match the requested hostname/IP; a certificate for `nanacoin-rs.local`
does not automatically validate a numeric IP. Discovery cannot bypass TLS
validation.

A page served over HTTPS may not call a plain-HTTP address: browsers block it
as mixed content before the request is made, and no CORS header can permit it.
That is why the **live-board build** is not the GitHub Pages demo - a page served from
`https://you.github.io` could never reach `http://192.168.1.158`. The
`http://localhost` exemption does not extend to private LAN addresses.
The separate `demo` build uses an in-browser ledger and needs no LAN access.

Rust already includes TLS on the S3. Keep the web site's HTTP URL available
when switching back to the TinyGo firmware.

## Administrator machine health

The Angular source is in `../nanacoin_go/angular`. Sign in as Nana and open
**Machine health** (`#/diagnostics`) to see the Rust API board's internal
RAM/fragmentation, PSRAM, temperature, Wi-Fi, clock, task/stack headroom,
server counters, NVS storage and flash partition map. The displayed board
is the API S3, not the web-serving S2. Charts keep at most 120 samples in the
browser. Hidden tabs stop polling, requests do not overlap, and leaving the
page cancels pending requests. Old Rust firmware can show basic heap
counters; unsupported firmware reports its limitations.

Building with `npm run build` in `nanacoin_go/angular` only produces local
assets. Deploy scripts write to the web board; do not run them until a
board update is intended. This diagnostics change does not require or
perform a flash/deployment as part of local validation.

## How the server works

`static.py` is the whole thing. Three decisions worth knowing:

**Files are streamed, not returned.** The rest of this project's handlers build
a response body in memory and hand back a string. That is right for JSON and
wrong for a 200KB bundle on a 2MB board, so `send` writes the header and then
copies the file a chunk at a time. Peak memory is one chunk.

**Gzip is done on the PC.** `deploy.ps1` compresses each text asset and ships
whichever copy is smaller; the board never compresses anything, it just picks
the `.gz` when the browser says it accepts it. On 4MB of flash that matters
more than it usually would.

**Unknown paths fall back to `index.html`.** Angular's routes - `/market`,
`/economy` - exist only in the browser. Without the fallback, opening the app
at a route or pressing refresh returns a 404 from a server that has never
heard of it, which is the most common way a deployed SPA looks broken.

Hashed filenames (`main-ETPGPPCZ.js`) are served `immutable` for a year;
`index.html` is `no-cache`, because it names them and a stale copy asks for a
build that no longer exists.

## Tests

`static.py` is written to MicroPython's subset but is ordinary Python, so its
path handling is tested on the PC rather than by flashing and clicking:

```powershell
python -m pytest test_static.py -q
```

The cases that matter are the SPA fallback and the traversal check - the board
holds `config.py`, which holds the WiFi password, and no request path may
reach it.
