# Builds, runtime and verification

## Generated C still needs a runtime

GnuCOBOL translates `bank.cob` to C, then a C compiler produces machine code.
The generated program contains both calculations and calls into `libcob`,
GnuCOBOL's runtime. Generating C does not remove that dependency. The runtime
supplies decimal operations, program machinery and initialization; GMP
supports its numeric implementation.

```text
bank.cob --cobc--> generated C --target C compiler--> bank object
bridge.c ---------------------target C compiler--> bridge object

Windows: bank + bridge + runtime dependencies -> ncposting.dll
ESP-IDF: bank + bridge + POSIX shim -> libncbank.a
         Rust links libncbank.a + libcob.a + libgmp.a + libltdl.a
```

The bridge is banking-independent glue. It exports a stable, fixed-width C
function and initializes the runtime. The generated `NCBANK` code contains
the COBOL logic; Rust does not duplicate those rules inside the bridge.

On Windows, the module is built with MSYS2 UCRT64 GnuCOBOL/GCC and loaded by
the Rust/MSVC executable. The shared C ABI makes that possible; a Rust ABI
across compiler toolchains would not be the contract. Executable, module and
dependency DLLs must all match the architecture. The current tooling uses
64-bit Windows and GnuCOBOL 3.2.

## Build and test on Windows

Run these commands from the repository root. The default compiler prefix is
`C:/msys64/ucrt64`; `build.py --prefix` can select another compatible install.

```powershell
# Build both engines, run HTTP contracts and compare scenarios.
# --clock includes deterministic time contracts using an opt-in launcher clock.
python conformance/check.py --cobol --clock
```

This requires no board. The launcher manages isolated server processes and
development data. Its controlled clock is a testing feature, not an added
public HTTP endpoint.

For a manual COBOL build:

```powershell
python nanacoin_rs/cobol/build.py
cargo build --locked --manifest-path nanacoin_rs/Cargo.toml --bin nanacoin --features cobol-core --target-dir .local/conformance-cobol/rust
$env:NANACOIN_COBOL_DLL = (Resolve-Path .local/conformance-cobol/ncposting.dll).Path
.local/conformance-cobol/rust/debug/nanacoin.exe --bank-engine
```

`build.py` runs the inventory audit, invokes `cobc` with `bank.cob` and
`bridge.c`, copies toolchain DLL dependencies alongside `ncposting.dll`, and
writes a source/module hash manifest. The module name is historical: it now
contains the broader bank, not just posting arithmetic. Rust loads runtime
dependencies from the module directory using Windows DLL search controls.

For the regular Rust build, omit the feature:

```powershell
cargo build --locked --manifest-path nanacoin_rs/Cargo.toml --bin nanacoin --target-dir .local/conformance-rust
.local/conformance-rust/debug/nanacoin.exe --bank-engine
```

`--bank-engine` reports the compiled selection without opening bank storage.
The regular build does not require GnuCOBOL or its DLL. Engine selection is a
build choice, not a switch that changes rules partway through a running bank.

## Static linking for a board

Windows binaries cannot run on an ESP32. `build_target_runtime.py` cross-builds
GMP and GnuCOBOL for the selected ESP-IDF C toolchain.
`build_target_bank.py` emits C with `cobc -C`, compiles it with that same target
compiler, and packages the bank with the runtime archives. The board uses
the same bridge ABI, resolved by static linking instead of DLL lookup.

`static_posix.c` satisfies runtime references to `getlogin`, `signal` and
`_fork`. These unsupported services return failure with `ENOSYS`; they are
not a full POSIX implementation. The static `libltdl` adapter keeps required
link symbols while refusing dynamic module loading. The bank's entry point
is already linked, and its storage I/O remains in Rust.

For example, with the P4 runtime archives already built in the default
`.local/cobol-firmware-source` location:

```powershell
python nanacoin_rs/cobol/build_target_bank.py --target esp32p4 --compiler C:/Espressif/tools/riscv32-esp-elf/esp-14.2.0_20251107/riscv32-esp-elf/bin/riscv32-esp-elf-gcc.exe --gnucobol-source C:/Users/matth/Downloads/gnucobol-3.2_win --output-dir .local/cobol-target-p4
python nanacoin_rs/cobol/build_firmware.py --board p4 --engine cobol --target-dir C:/nc-cob-p4 --static-module .local/cobol-target-p4
```

Adjust compiler and source paths for the installed toolchain. The
[runbook](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/cobol/README.md)
contains the runtime-build prerequisites and commands. Use short firmware
output paths for the Windows ESP-IDF build.

The firmware builder verifies the static module's target and source hashes
and passes its directory through `NANACOIN_COBOL_STATIC_DIR`. Cargo's
`build.rs` requires an absolute directory, the matching Rust target/ABI/frame
marker, and all four archives. Firmware runtime warmup happens before journal
recovery; the P4 COBOL server uses a larger stack than the regular Rust server.

The commands above build an image. Flashing and board recovery are separate
steps documented in the [Rust deployment notes](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/DEPLOY.md).
To build regular P4 Rust firmware, use `build_firmware.py --board p4 --engine rust`
with a separate target directory.

## What has been verified

The October 7, 2026 named-field/local-storage revision passed 110 HTTP
contracts per engine, 18 matching differential scenarios and 1,128 ABI
boundary vectors in each of two passes. Its optional-build Rust tests passed
225 tests. Those results establish desktop behavior and layout checks, not
every hardware workload.

An earlier COBOL image was deployed to the P4 and served HTTPS, the embedded
client and API probes. The later named-field revision was built offline for
P4; its bank kernel also passed 2,643 calls in an emulator probe with a stable
warmed heap. That probe is smaller than the complete server. Maximum-capacity
financial workloads and power-cut recovery on the real P4 remain separate
hardware gates. Do not infer those results from an ABI test or successful boot.

## Changing the implementation

For a new rule, follow its Rust wrapper to the operation and named COBOL
record, update the paragraph and adapters together, then add or adjust public
conformance assertions. A layout change also needs ABI vectors and coordinated
module metadata; a stale module must fail validation instead of misreading
memory. Update the command inventory when the public command surface grows.

Run the Windows conformance matrix first. Rebuild target archives after
changing the COBOL source or bridge; a previous static archive does not pick
up source changes merely because Cargo is rerun. Firmware verification then
checks the target runtime, stack and allocation behavior that desktop tests
cannot exercise.
