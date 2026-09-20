# NanaCoin

A household currency and marketplace that runs on a microcontroller. Nana
issues the money, the household spends it, and a small server keeps an honest
double-entry ledger of who has what.

NanaCoin is **not cryptocurrency** — no blockchain, no mining, no consensus.
The name comes from the [SMBC comic](https://www.smbc-comics.com/comic/nanacoin).

These docs are about the engineering: fitting a correct ledger, an HTTP API and
TLS into a few hundred kilobytes of RAM, and knowing it stays up.

## Boards

| Board | Chip | Role |
|-------|------|------|
| ESP32-S3-N16R8 | ESP32-S3 | 16MB flash, 8MB octal PSRAM, dual-core. Runs the ledger and the HTTPS API. |
| DiGiYes ESP32-S2 Mini V1.0.0 | ESP32-S2FN4R2 | 4MB flash, 2MB PSRAM, native USB. Serves the Angular client. |

Neither board is required to try it — the ledger runs on a laptop first, and
that is the sensible place to start.

## Where to start

[Basic Setup](basic_setup/index.md) takes an ESP32 from "just unboxed" to
"serving a web page on your WiFi" on Windows, including the failures you will
probably hit. Read this first if you have never flashed a board.

[Rust and NanaCoin](rust/index.md) is the **active implementation**: the
ledger, the JSON API, and the firmware that runs them. It covers the build,
how ownership keeps allocation bounded, the domain and concurrency model, how
records survive a power cut in NVS, and what the diagnostics endpoint reports.

[TinyGo and NanaCoin](tinygo/index.md) is where the project started. The
firmware is frozen pending upstream PSRAM support, but the write-up is the more
detailed one on memory budgets and on load-testing a board until it falls over —
and the Angular client documented there is still the one in use.

## The shape of it

There are two ways to deploy it.

**One board.** The Rust firmware embeds the built Angular site as read-only
flash assets and serves it from the same origin as the API, so a single S3 is
the entire deployment.

**Two boards.** Or keep HTML off the ledger board entirely and let a second
board — or any static host — serve the client, which talks to the API from the
browser. That separation is what makes the ledger board's memory budget easy to
reason about, and it is the only option for the TinyGo firmware: its ESP32-S3
target uses internal SRAM and cannot reach the 8MB PSRAM, so there is nowhere
to put the bundle.
