# Build and code map

## Two adapters, one library

[Cargo.toml](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/Cargo.toml)
defines a library named `nanacoin`, a desktop binary named `nanacoin`, and an
ESP32 binary named `nanacoin-esp32`. Desktop is the default feature. The board
build disables it and enables `esp32`.

`src/domain.rs` implements state, commands and invariants. `src/journal.rs`
coordinates validation, durability and replay. `src/api.rs` turns HTTP-shaped
inputs into service calls and JSON. `src/bin/desktop.rs` and
`src/bin/esp32.rs` supply sockets and storage. Rust's `mod` declarations organize
source modules; `use` brings names into scope, rather than executing imports.

The board pins ESP-IDF v5.5.3 and esp-idf-svc 0.52.1. It is not a bare-metal,
`no_std` Rust application: `std::sync::Mutex`, threads and heap allocation use
the ESP-IDF platform underneath.

## Local verification, without touching a board

From `nanacoin_rs`, in Git Bash:

```bash
make build
make test
make check
make run
```

`make check` runs formatting checks, Clippy, Rust tests and a local HTTP/restart
smoke test. `make run` starts the desktop HTTP API. The desktop adapter persists
to files; it does not emulate NVS's physical flash layout. Use disposable test
data for load generators, which create real economic activity.

From `nanacoin_go/angular`, `npm run build` builds the shared client and
`npm test -- --watch=false` runs its tests. Its connection discovery probes
configured/current candidates, `nanacoin.local`, legacy `nanacoin-rs.local`, `nanacoin-api.local`, and
the configured known IP candidates, trying HTTPS and HTTP from an HTTP page.
An HTTPS-hosted page only probes HTTPS: certificate failure never triggers a
plaintext downgrade. HTTPS certificate validation still applies.

## Firmware compilation is separate from deployment

```bash
make certs
make firmware
```

These targets generate local development certificate files if absent and
compile firmware; they do not flash or reset a board. The board build needs
the Espressif Rust toolchain and SDK. The Windows setup in
[build-esp32.sh](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/scripts/build-esp32.sh)
uses the installed SDK under `C:/Espressif` and a short output path `C:/ncr`.
Its actual compiler invocation is:

```bash
cargo +esp build --locked --release --no-default-features --features esp32 \
  --bin nanacoin-esp32 --target xtensa-esp32s3-espidf -Z build-std=std,panic_abort "$@"
```

Credentials are read by the build from environment variables or supported
gitignored configuration files. Wi-Fi credentials and the TLS private key are
embedded in the firmware: treat binaries and build caches as sensitive.
Never publish them as sample artifacts. Certificate trust, CORS and mDNS are
separate requirements; a successful compile proves none of those work on a LAN.

The firmware embeds Angular and serves HTTP and HTTPS at `nanacoin.local` in
Easy mode. Nana can require HTTPS household-wide; HTTP then serves only trust
instructions and the public CA download, not the app or API.
`make run-bundle` builds and serves the same assets on the desktop. The
asset packager replaces the generated index's API default with same-origin
`/api/v1` without changing the standalone client's source. Both identity and
gzip representations are embedded with a generated lookup table. In
[web.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/web.rs),
the asset data is borrowed for the program lifetime:

```rust
pub raw: &'static [u8],
pub gzip: &'static [u8],
```

This does not copy the website into the heap per request. Static routes bypass
the service mutex and send bounded chunks from flash. Unknown API routes never
receive the Angular index, and missing JS files return 404. Hashed assets use
immutable caching; index revalidates. Static routes support GET only (HEAD is
explicitly rejected), without byte ranges. There is no mounted filesystem.

`make firmware` now also runs the Angular production build, packages assets,
creates an application `.bin` and checks the unchanged 4 MiB image limit.
The build uses `mkcert` and OpenSSL to create a dedicated local CA, but does not
install trust on this computer. `/trust` guides device setup and `/ca` downloads
only `certs/home-ca.der`. The server key is embedded; the CA private key remains
in ignored `.local/ca` and is never bundled. Compare the CA fingerprint through
an independent trusted channel before installing it. See
[connection security and USB recovery](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/CONNECTION_SECURITY.md).

When deployment is explicitly intended, `make deploy PORT=COM9` rebuilds,
checks the attached board's partition table, and writes only the application
at `0x10000`. It refuses other layouts; no automatic migration/erase occurs.
The ledger and bootloader are not written. `bash scripts/deploy.sh COM9 --dry-run`
does the build and prints the plan without opening a port. This is not an
initial installation tool. See the
[crate README](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/README.md)
for prerequisites and `make web-check` for offline integration checks.
