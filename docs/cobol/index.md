# GnuCOBOL and NanaCoin

NanaCoin can build the Rust server with either its regular Rust bank or a
GnuCOBOL bank. The `cobol-core` Cargo feature selects the COBOL implementation;
an ordinary build remains entirely Rust. Both serve the same JSON API and
Angular client. Clients do not select or call a different protocol.

This guide explains the implementation as of October 7, 2026. Source excerpts
are parts of the actual program, not standalone examples.

Read in order:

1. [Rust, C and COBOL interop](interop.md): the call boundary and ownership.
2. [Bank decisions and durable writes](bank.md): where the business logic lives.
3. [COBOL language and coding patterns](language.md): named records, scope and arithmetic.
4. [Builds, runtime and verification](workflow.md): Windows DLLs and board archives.

## What each language does

| Concern | Implementation in a COBOL build |
|---|---|
| HTTP, HTTPS, routing and JSON | Rust and the existing platform framework |
| Credentials, hashing and sessions | Rust; COBOL receives identity and role facts |
| Banking permissions and financial decisions | COBOL |
| Posting calculations, loans, forex, gifts, art, lotto and commerce | COBOL decisions and numeric plans; Rust installs records |
| Read visibility, selection, ordering and totals | COBOL kernels, with Rust fetching records and formatting JSON |
| Clocks, randomness and scheduling | Rust supplies facts; COBOL decides eligibility and settlement |
| Live records, journal, checkpoints and storage I/O | Rust |
| Fixed-width function boundary and runtime startup | A small C bridge |

The COBOL bank is one program, `NCBANK`, with operation-specific paragraphs.
It has no HTTP listener, JSON parser or bank database of its own. Rust calls it
synchronously to decide what a request means for the bank.

## Where to read the code

Paths below are relative to the repository root.

| Source | Purpose |
|---|---|
| [`nanacoin_rs/cobol/bank.cob`](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/cobol/bank.cob) | Constants, named ABI records, decimal calculations and policy paragraphs |
| [`nanacoin_rs/src/cobol.rs`](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/cobol.rs) | Rust wrappers, runtime lock, record adapters and prepared plans |
| [`nanacoin_rs/cobol/bridge.c`](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/cobol/bridge.c) | Exported C ABI and `cob_init` |
| [`nanacoin_rs/cobol/static_posix.c`](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/cobol/static_posix.c) | Unsupported POSIX calls needed when linking the board runtime |
| [`nanacoin_rs/cobol/inventory.py`](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/cobol/inventory.py) | Coverage audit against the Rust command surface |
| [`conformance`](https://github.com/matthewdeanmartin/nanacoin/tree/main/conformance) | Public API contracts and comparisons between builds |

The [Rust guide](../rust/index.md) explains the server, ownership and storage
surrounding this bank. The [TinyGo guide](../tinygo/index.md) describes the
earlier implementation. Detailed experiment commands and historical results
remain in the [COBOL runbook](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/cobol/README.md).
