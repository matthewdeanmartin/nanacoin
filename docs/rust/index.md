# Rust and NanaCoin

`nanacoin_rs` is the Rust implementation of the household currency and
marketplace. It shares the Angular client in `nanacoin_go/angular` with TinyGo;
`nanacoin_web` can still host that client separately. Rust firmware now embeds
the production client at `https://nanacoin.local/`, alongside `/api/v1`.
Do not run the legacy UI board with the same hostname simultaneously. The Rust API board
uses ESP-IDF, internal SRAM, external PSRAM, and a persistent NVS ledger.

This guide describes implemented code, not proposed storage migrations.
The examples below are excerpts from the linked source, not standalone programs.

| Concern | TinyGo board | Rust board |
|---|---|---|
| Platform | TinyGo runtime, espradio and Go networking | Rust `std` over ESP-IDF/FreeRTOS, lwIP and mbedTLS |
| Working memory | Internal SRAM; no PSRAM in this configuration | SRAM plus PSRAM-enabled allocator |
| Live domain state | Bounded RAM stores | Bounded RAM stores |
| Durable household | No: discarding board journal | Yes: NVS journal and checkpoints |
| Execution | One active core in current target | Core 0 probes/network tasks, core 1 HTTPS/domain |
| Diagnostics | On-request heap sample plus existing crash/trend data | Periodic snapshot, hardware queries and NVS accounting |
| Browser | Same Angular application | Same Angular application |

The extra working RAM and different networking/runtime stacks make this an
unequal language benchmark. Flash is persistent storage, not extra writable
heap. Both implementations keep the live accounting state in RAM.

Read in order:

1. [Build and code map](workflow.md)
2. [Memory and ownership](memory.md)
3. [Domain, HTTP and concurrency](architecture.md)
4. [NVS keys, recovery and retirement](storage.md)
5. [Diagnostics and verification](diagnostics.md)

For the corresponding Go explanations, see [TinyGo and NanaCoin](../tinygo/index.md).
The Go implementation is frozen except for the diagnostic compatibility change;
the Rust implementation is the active development path.
