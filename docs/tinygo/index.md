# TinyGo and NanaCoin

This implementation is frozen except for diagnostic API compatibility work.
For the active implementation, see [Rust and NanaCoin](../rust/index.md).
The Go board uses internal RAM and a volatile journal; the Rust board also
uses PSRAM and persistent NVS. These are not equivalent memory configurations.

NanaCoin is a household currency and marketplace running on an ESP32-S3.
It has accounts, passwords, a ledger, listings and a browser application.
The interesting constraint is that the server lives on a microcontroller:
the same device must keep WiFi alive, receive requests and remember balances
in a small amount of working memory.

This guide assumes you can write application code. It explains the embedded
parts as they appear, using NanaCoin as a worked example rather than asking
you to learn electronics first. No sensors or soldering are involved.

## What runs where

```text
Laptop or phone                       ESP32-S3
Angular application  -- HTTP/WiFi -->  TinyGo firmware
pages, forms, charts                  HTTP server, authentication,
                                      household rules, RAM ledger
```

The browser does the presentation work. The board is the authority on users,
permissions and money. For development, an ordinary Go executable can replace
the board and run the same application packages on a laptop.

This is an experimental application. The current board forgets its household
when power is removed or the firmware restarts. Desktop journal files can
persist state; board flash persistence is future work. Do not put a household's
only record of real obligations on it.

## Contents

1. [How it works](how_it_works.md) — firmware, runtimes and the different kinds of memory.
2. [Setup](setup.md) — the board, toolchain, local patches and USB ports.
3. [The workflow](workflow.md) — desktop development, building, flashing and tests.
4. [Memory](memory.md) — allocation, ownership, fixed capacity and stack hazards.
5. [The web framework](web_framework.md) — the path from a TCP connection to a domain operation.
6. [NanaCoin's domain](nanacoin.md) — household money, authorization and the marketplace.
7. [Storage and retries](storage.md) — rings, commit processing, idempotency and future flash batches.
8. [Concurrency and cores](concurrency.md) — workers, locks, streaming and the second CPU.
9. [Diagnostics and load tests](diagnostics.md) — how to learn something useful when it fails.

## Where the code lives

| Directory in `nanacoin_go/` | Responsibility |
|---|---|
| `cmd/nanacoin` | Desktop executable and file journal |
| `cmd/nanacoin-esp32` | Board startup, WiFi, connection pool and runtime diagnostics |
| `internal/boardhttp` | Bridge between the embedded HTTP transport and Go handlers |
| `internal/api` | Routes, request parsing, response rendering and HTTP policy |
| `internal/core` | Authorization and coordinated domain operations |
| `internal/ledger` | Packed transactions, balances, text storage and history retention |
| `internal/auth` | Password checking, PKCE, sessions and login throttling |
| `internal/storage` | Journal contract and backend implementations |
| `internal/eventlog` | Bounded diagnostic history |
| `angular` | Main browser application |
| `patches` | Dependency fixes needed by the board build |

The separate [nanacoin_load project](https://github.com/matthewdeanmartin/nanacoin/tree/main/nanacoin_load)
contains Python/Locust tests and HTML reports. Python is test tooling, not
part of the firmware.

The [project README](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_go/README.md)
is the quick reference. Its linked memory and experiment notes contain
implementation history and measurements for particular builds. This guide
describes the architecture; an old throughput number is not a device guarantee.
