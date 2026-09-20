# NanaCoin

A household currency you can actually run. Nana issues the money, everyone
spends it on chores and favours, and the whole thing lives on a microcontroller
stuck to the side of the fridge.

Name and idea from the [SMBC comic](https://www.smbc-comics.com/comic/nanacoin).

**NanaCoin is not cryptocurrency.** No blockchain, no mining, no consensus, no
wallets, no distributed anything. It is one small server that applies
double-entry transactions and answers questions about them — closer to a shared
spreadsheet with an honest referee than to Bitcoin.

## Who it is for

Families and housemates who want a play currency with real rules: pocket money
that is tracked, chores that pay, a marketplace where a sibling can sell you
their dessert. And, honestly, it is for people who want to see how far a $10
board can be pushed — the interesting engineering here is fitting a correct
ledger into a few hundred kilobytes of RAM.

Nana is the trusted authority. She issues and retires currency, reverses
mistakes, and can disable an account. Everyone else holds a balance, transfers
to each other, lists things for sale, haggles over offers, and trades coins for
(pretend) dollars at a posted exchange rate.

## How it works

Two boards, one household currency:

```text
   ESP32-S2 Mini                    ESP32-S3-N16R8
   4MB flash, 2MB PSRAM             16MB flash, 8MB PSRAM
   MicroPython                      Rust
   http://nanacoin.local/           https://nanacoin-rs.local/api/v1
   serves the Angular bundle        the ledger, the API, the money
          \                                    /
           \                                  /
            `------->  the browser  <--------'
```

The split is deliberate: the board holding the money does nothing but hold the
money. It serves JSON and no HTML. Neither board is required — you can run the
whole thing on a laptop first, and most people should.

## Projects

| Directory | What it is |
|---|---|
| [`nanacoin_rs/`](nanacoin_rs/) | **The active implementation.** The ledger and JSON API in Rust, for desktop and ESP32-S3. |
| [`nanacoin_go/`](nanacoin_go/) | The original TinyGo implementation, plus the shared Angular client in `angular/`. Firmware is frozen; the client is current. |
| [`nanacoin_web/`](nanacoin_web/) | MicroPython static host that serves the Angular bundle from an ESP32-S2. |
| [`nanacoin_load/`](nanacoin_load/) | Load lab: Python 3.14 and Locust, with HTML reports. Answers "does the board fall over?" |

## Install

You need [Rust](https://rustup.rs/) 1.88+ and [Node](https://nodejs.org/) for
the client. Nothing else for the laptop version.

```bash
git clone https://github.com/matthewdeanmartin/nanacoin.git
cd nanacoin
```

Building firmware additionally needs the Espressif toolchain — ESP-IDF v5.5.3
and the Xtensa Rust toolchain. [`docs/basic_setup/`](docs/basic_setup/) walks
through that from unboxing, including the parts that go wrong.

## Run it

Start the API on your laptop:

```bash
cd nanacoin_rs
make run                      # http://127.0.0.1:8080
```

Then the client, in another terminal:

```bash
cd nanacoin_go/angular
npm ci                        # first time only
npm start                     # http://localhost:4200
```

Open <http://localhost:4200>. A fresh install has no household: the client
walks you through creating one and choosing Nana's username and PIN. From
there, add members and start issuing money.

To put it on a board instead, see [`docs/rust/workflow.md`](docs/rust/workflow.md).
`make firmware` builds the image; flashing is a separate, deliberate step.

## Hardware known to work

Both boards below have been flashed, run, and load-tested — these are measured,
not aspirational.

| Board | Chip | Role | Notes |
|---|---|---|---|
| ESP32-S3-N16R8 | ESP32-S3 | Ledger and API | 16MB flash, 8MB octal PSRAM, dual core. Serves HTTPS on four concurrent TLS sockets. |
| DiGiYes ESP32-S2 Mini V1.0.0 | ESP32-S2FN4R2 | Static site host | 4MB flash, 2MB PSRAM, native USB only. |

The S3 firmware has run a mixed read/write load for an hour without a reboot,
holding a flat heap and a constant largest-free-block. The load lab in
[`nanacoin_load/`](nanacoin_load/) is how that gets checked, and
[its FINDINGS.md](nanacoin_load/FINDINGS.md) records what each board actually
did.

No soldering, no GPIO wiring, no sensors. If it needs a breadboard, it is not
this project.

## Documentation

Full write-up in [`docs/`](docs/) — build it with `cd docs && make serve`.

- **[Basic Setup](docs/basic_setup/index.md)** — ESP-IDF from unboxing to
  serving a page, including the failures along the way
- **[Rust and NanaCoin](docs/rust/index.md)** — the active implementation:
  build, memory and ownership, domain and concurrency, NVS storage, diagnostics
- **[TinyGo and NanaCoin](docs/tinygo/index.md)** — the original firmware, its
  web framework, memory budgets and load testing

## Credits

Built by [Matthew Dean Martin](https://github.com/matthewdeanmartin).

The name, and the idea of a grandmother running a household economy, come from
Zach Weinersmith's [SMBC comic](https://www.smbc-comics.com/comic/nanacoin).

Standing on: [TinyGo](https://tinygo.org/), [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc)
and the [esp-rs](https://github.com/esp-rs) project, [Espressif's ESP-IDF](https://github.com/espressif/esp-idf),
[MicroPython](https://micropython.org/), [Angular](https://angular.dev/) and
[Locust](https://locust.io/). The TinyGo networking stack uses
[soypat/lneto](https://github.com/soypat/lneto) and
[espradio](https://github.com/tinygo-org/espradio), with local patches in
[`nanacoin_go/patches/`](nanacoin_go/patches/).

MIT licensed — see [LICENSE](LICENSE).
