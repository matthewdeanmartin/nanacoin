# PSRAM epic — frozen pending upstream support

Status: proposal / deferred, 2026-09-20. TinyGo NanaCoin is in deep freeze,
apart from the explicitly requested diagnostic API compatibility work.
Do not maintain a compiler/runtime fork, repair upstream PSRAM patches, or
flash the demonstration board as part of this epic.

## Why pause

Our installed TinyGo 0.42.0 ESP32-S3 target uses internal SRAM, not the
board's 8 MiB octal PSRAM. Rust's ESP-IDF allocator uses PSRAM. Comparing
their stability is therefore not a controlled language comparison. TinyGo's
recorded load tests approached SRAM exhaustion; this is a major disadvantage.
Flash persistence is a separate issue: the Go board discards journal payloads
and keeps live state in RAM. Adding persistence alone would not free that RAM.

The installed target has no PSRAM startup support. Upstream
[PR #5554](https://github.com/tinygo-org/tinygo/pull/5554) is open as of this
review and reports octal PSRAM testing on an N16R8 module. Its example places
fixed globals in a `.psram` section. That is not automatically an expanded
garbage-collected Go heap. No release date is assumed.

## Reopening gate

- A released, supported compiler/runtime supports this board's octal PSRAM;
  record the exact version and upstream documentation. Do not rely on this
  document's dated PR status.
- Determine whether support means explicit static placement, a GC heap, or
  both, and how pointer-containing objects are traced.
- Review remaining board limitations, including multicore scheduling, against
  the released target rather than assuming all ESP32-S3 features are supported.
- Obtain separate permission for hardware tests after the owner's demo.

## Eventual work, not implemented

1. Pin the supported toolchain; retain a reproducible SRAM-only baseline.
2. Verify PSRAM detection, capacity and read/write integrity on the actual
   board, including cold boots and repeated resets.
3. Put large bounded domain backing arrays and text buffers in PSRAM using
   supported APIs. Preserve locks, capacities and overflow behavior. Keep
   hardware-required internal allocations internal; do not relocate stacks or
   radio buffers indiscriminately.
4. Verify GC roots, pointer lifetimes, stack frames and allocation behavior.
   Static placement must not hide live heap pointers from the collector.
5. Report internal and external memory separately; repeat identical load tests
   with the same users, history, concurrency and persistence policy.

Acceptance: measurable internal headroom, stable long-running load and GC
cycles, correct balances, bounded retained history, and reproducible builds
without local runtime patches. PSRAM is volatile: persistent recovery remains
a separate storage project. Do not add these proposed capabilities to the
implemented-feature documentation under `docs/`.
