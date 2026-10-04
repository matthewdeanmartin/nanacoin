# Contributing to NanaCoin

The active server is **Rust** (`nanacoin_rs`); the browser client is **Angular**
(`nanacoin_ui`). The original Go/TinyGo implementation is in
[`archive/nanacoin_go`](archive/nanacoin_go) and is not part of current testing
or deployment. Project proposals and the roadmap live in [`spec`](spec);
server contracts remain in [`nanacoin_rs/spec`](nanacoin_rs/spec).

Read [AGENTS.md](AGENTS.md) before changing storage or accounting. This project
is pre-release with disposable development fixtures: use the current schema
rather than adding old-data migration scaffolding. Crash recovery, durable
writes, replay, idempotency and double-entry invariants must still hold.

## Development setup

Install Node.js 22 (the CI version), npm, and a stable Rust toolchain supporting
Rust 1.88 or newer. Install GNU Make for the shortcuts below; Rust's Makefile
also requires Bash (Git Bash works on Windows). Desktop tests do not require
the Xtensa toolchain, an ESP32 board, Wi-Fi credentials or flashing anything.

From the repository root:

```sh
make setup
make coverage-setup
```

`setup` runs `npm ci` using the Angular lockfile. `coverage-setup` installs
`cargo-llvm-cov` 0.9.1 and the active Rust toolchain's `llvm-tools-preview`
component. Repeat the component installation after switching Rust toolchains.
Vitest's V8 coverage provider is included in the npm development dependencies
and pinned to the same version as Vitest.

Without Make, these setup commands also work in PowerShell:

```sh
cd nanacoin_ui
npm ci
cd ../nanacoin_rs
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
cd ..
```

For local development, run `make -C nanacoin_rs run` in one terminal and
`npm start` from `nanacoin_ui` in another. Open
`http://localhost:4200/?api=` to use the development proxy. For a server-free
demo, run `npx ng serve --configuration demo` from `nanacoin_ui`.

## Run tests

All commands below start at the repository root and run once, without watch:

| Command | What runs |
| --- | --- |
| `make test` | Angular live-build tests, then Rust desktop unit and integration tests |
| `make test-ui` | Angular live-build Vitest suite in jsdom |
| `make test-ui-demo` | Same suite compiled with demo replacements, including demo-only tests |
| `make test-rust` | Rust default desktop tests |
| `make test-rust-s2` | Rust desktop tests using the smaller S2 bank capacity profile |
| `make -C nanacoin_rs check` | Formatting, Clippy, both capacity profiles and real HTTP/restart smoke tests |

Some Angular tests intentionally run only in demo mode and are skipped in the
live suite. Run both modes when changing demo behavior. jsdom tests do not
replace real browser checks.

Commands without Make:

```sh
cd nanacoin_ui
npm run test:ci
npm run test:demo
cd ../nanacoin_rs
cargo test --locked
cargo test --locked --features board-s2
```

For browser integration checks, see
[`nanacoin_ui/SHOWCASE_PARITY.md`](nanacoin_ui/SHOWCASE_PARITY.md) and
[the Pages workflow](.github/workflows/nanacoin-pages.yml).
The showcase builds and serves the demo, uses Playwright, and checks journeys
that unit tests cannot cover. Build the live UI with `npm run build`, or the
demo with `npx ng build --configuration demo --base-href /nanacoin/`.

## Measure coverage and open the reports

```sh
make coverage           # Angular live + Rust default, sequentially
make coverage-ui-demo   # Optional: demo replacements
make coverage-rust-s2   # Optional: S2 capacity profile
```

You can also run only `make coverage-ui` or `make coverage-rust`.
Each prints a coverage summary and creates a browsable HTML report:

| Suite | Open this file in a browser | Machine-readable files |
| --- | --- | --- |
| Angular live | `nanacoin_ui/coverage/live/index.html` | `lcov.info`, `coverage-summary.json` in the same directory |
| Angular demo | `nanacoin_ui/coverage/demo/index.html` | `lcov.info`, `coverage-summary.json` in the same directory |
| Rust default | `nanacoin_rs/coverage/html/index.html` | `coverage/lcov.info`, `coverage/summary.json` |
| Rust S2 | `nanacoin_rs/coverage-s2/html/index.html` | `coverage-s2/lcov.info`, `coverage-s2/summary.json` |

On Windows, for example, run
`Start-Process nanacoin_ui/coverage/live/index.html` or
`Start-Process nanacoin_rs/coverage/html/index.html` from the root.
On Linux use `xdg-open`; on macOS use `open`.

Without Make, use `npm run coverage` or `npm run coverage:demo` from
`nanacoin_ui`. Rust equivalents from `nanacoin_rs` (PowerShell or Bash):

```sh
cargo llvm-cov --locked --all-targets --ignore-filename-regex '([/\\]tests[/\\]|[/\\]build\.rs$)' --html --output-dir coverage
cargo llvm-cov report --ignore-filename-regex '([/\\]tests[/\\]|[/\\]build\.rs$)' --lcov --output-path coverage/lcov.info
cargo llvm-cov report --ignore-filename-regex '([/\\]tests[/\\]|[/\\]build\.rs$)' --json --summary-only --output-path coverage/summary.json
cargo llvm-cov report --ignore-filename-regex '([/\\]tests[/\\]|[/\\]build\.rs$)' --summary-only
```

For S2, add `--features board-s2` to the initial test command and use `coverage-s2` as the
output directory. Run Rust coverage profiles sequentially: they share the
instrumented Cargo target directory. Each execution starts a fresh measurement;
it does not merge live/demo or Rust capacity profiles. Reports are ignored by Git.

Angular measures application TypeScript under `src/app`, excluding test files;
framework dependencies and bootstrap files outside that directory are outside
the report. Rust measures compiled application code, excluding integration
test harness files and `build.rs`. Inline Rust unit tests remain in their source
files. Host coverage cannot measure ESP32-only code removed by conditional
compilation, physical peripherals, or the bundled-web feature unless explicitly
enabled. A high percentage is not proof of correct accounting or security.
There is initially no percentage gate; use the per-file report to find meaningful
gaps before adopting a threshold.

### Initial measured baseline

Measured on Windows on October 3, 2026 with the commands above. These are
snapshots, not required minimums; rerun coverage after changing code or tests.

| Suite | Lines | Functions | Branches |
| --- | --- | --- | --- |
| Angular live | 56.65% | 47.62% | 51.96% |
| Angular demo | 56.81% | 47.47% | 51.69% |
| Rust default desktop | 83.71% | 82.50% | Not collected |
| Rust desktop with S2 capacity profile | 83.71% | 82.50% | Not collected |

Both Angular reports contain all 112 application TypeScript files. The live
suite passed 353 tests with three demo-only tests skipped; the demo suite
passed 355 with the live board-diagnostics test skipped. Both Rust profiles
passed their unit and integration suites. Rust's configured stable-toolchain
instrumentation reports lines, functions and regions, without branch metrics.

## Submitting a change

Keep changes focused and describe the behavior, tests run, and any limitations.
Add tests for changed accounting, permissions, recovery, and other behavior
that can fail materially. Update specs and fixtures with the implementation.
Keep secrets, household journals and generated reports out of commits. Do not
flash, erase, provision or deploy hardware as part of ordinary test commands.

Report security issues privately using [SECURITY.md](SECURITY.md).
