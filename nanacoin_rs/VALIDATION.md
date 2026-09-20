# Validation record

Fresh parity verification: September 19, 2026, on Windows. No board was flashed, erased, reset, probed or accessed over serial. The owner will connect the board for a subsequent flash.

## Host checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` (50 tests), `cargo build --locked`, and `python scripts/smoke.py` pass. Smoke starts its own temporary server and journal; it does not stop an existing user server.

Tests cover checked ledger invariants, authorization, capacities, purchases, reversals, history eviction, persistence, ambiguous writes, file locking, partial-tail recovery and corrupt-frame refusal. Authentication cases cover password hashing, PKCE challenge/redirect/code/session expiry, rate limits, logout, role/status/password revocation, last-Nana protection, legacy migration and durable HTTP retries across restart.

The allocation tests observe zero allocations after startup across 2,999 financial writes with a full state read after every write, and 1,000 forex trades (2,000 ledger legs) with immediate keyed retries and quote-book reads. History wraps repeatedly and quotes recycle while balances remain correct. Worst-case escaped JSON tests cover response bounds and oversized journal-event refusal. This does not measure HTTP libraries, TLS, FreeRTOS, Wi-Fi, NVS or board stacks.

Fifteen offer tests derive the behavior from `../nanacoin_go/internal/core/offers_test.go` and add Rust persistence/capacity cases. They cover both payment directions, negotiated prices, permissions/privacy, insufficient funds and disabled accounts, exact/configurable deadlines, replay, clock rollback, correction overdrafts, history eviction, pinned listings, closed-slot recycling, bounded input, durable keyed retries, and injected failed/ambiguous accept and undo writes. A second allocation test fills all 32 offer slots, reads them 1,000 times and retries an acceptance 1,000 times with zero allocations in the API/domain path after initialization.

The HTTP smoke uses disposable journals and processes on an unused loopback port. It exercises JSON-only root behavior, Angular-shaped login/views, provisioning, CORS, members, money movement, listing edits, account and ledger privacy, USD issuance, forex trading, offers/accept/unaccept, 1,000 consecutive offer reads, revocation, body limits, restart and durable retries. It invokes the migration CLI against a generated legacy Rust journal and checks that balances survive while the old token stops authenticating. It never migrates the user's real journal. This is API contract testing, not an interactive Angular browser test.

Five new parity tests cover listing metadata and timestamp replay, read permissions, BID/ASK payment directions, keyed HTTP trade retries across restart, manual corrections in both currencies, exact expiry, quote capacities/recycling, and failed/ambiguous appends. Both trade legs either replay together or neither does.

## Firmware compilation

The compile-only release build and link passed for xtensa-esp32s3-espidf, ESP-IDF v5.5.3 and mDNS 1.8.2. Output:

```text
C:/ncr/xtensa-esp32s3-espidf/release/nanacoin-esp32
```

The fresh build uses the saved ignored `../nanacoin_go/wifi.local.json` Wi-Fi settings and existing ignored local development TLS files. Credential values are not recorded in this report. No administrator credential is embedded. SDK configuration enables HTTPS, octal PSRAM/malloc, PSRAM-backed NVS cache, a 64 KiB main stack and multicore FreeRTOS. Wi-Fi/lwIP uses core 0; HTTPS selects core 1.

Firmware also starts ESP-IDF SNTP for persisted offer deadlines. Clock synchronization and settlement behavior across physical power cycles still require board validation. No test used the plugged-in device.

The compiled Xtensa entry frames include 18,608 bytes for main, 10,272 for service startup, 7,680 for authorize parsing, 3,920 for the HTTP wrapper and 3,744 for the client router. The HTTP stack is increased from 16 to 24 KiB for nested call-chain headroom; main remains 64 KiB. Static frames do not establish peak runtime stack use: board high-water marks are still required.

## Deferred hardware acceptance

Compilation does not establish board uptime or OOM immunity. Future board work, after authorization, must verify boot/PSRAM, TLS/certificate trust, mDNS, authentication, NVS persistence/interrupted writes, Wi-Fi recovery, fragmentation during repeated TLS handshakes and stack high-water marks.

`scripts/soak.py` is a future two-client HTTPS read soak with bounded response reads, fresh connections and latency/failure reporting. It prompts for username and PIN/password and logs in through PKCE; NANACOIN_USERNAME / NANACOIN_PASSWORD environment values are optional. It changes no household funds. Keep duration below the eight-hour session lifetime. It has not been run against the board.

Flash endurance, memory headroom and NVS power-loss behavior remain unmeasured. Backup and compaction are required before long-term use.
