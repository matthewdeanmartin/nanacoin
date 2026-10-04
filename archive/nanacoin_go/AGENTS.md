# NanaCoin development board

## Development freeze

The TinyGo firmware is in deep freeze pending supported upstream compiler/runtime
support for more of this board, especially PSRAM. See `PSRAM_EPIC.md`. Do not
maintain a compiler fork or fix upstream runtime PRs. The owner's explicitly
requested machine-diagnostics compatibility work is the exception; further Go
firmware work requires a new request. The shared Angular client in
`../nanacoin_ui` remains active for Rust and is not frozen by this firmware
policy.

This is an experimental development application, not a production service.
Do not flash an older firmware, reset the household, or do recovery deployments
merely to leave the board responsive between experiments or before replacing
the firmware again. Deploy the intended new build when deployment is part of
the task; preserve diagnostic evidence when a failure matters.

RAM history/logs must have fixed startup capacity, fill once, then overwrite
the oldest entries. The ledger retains 365 records (text pressure can evict
earlier), with lifetime balance checkpoints and monotonic transaction IDs.
The event/error log already uses a 58-entry ring. Live users, accounts and
open listings are state, not disposable logs; never overwrite them as history.

TinyGo's ESP32-S3 goroutine stack is 8 KiB. Avoid value copies of large arrays
(including two-value range loops over array fields). Store large preallocated
arrays behind pointers and iterate indices. The first ring build generated a
12,336-byte CheckInvariants frame and crashed at boot; pointer-backed arrays
reduced it to 112 bytes. Also check compiled frames when changing fixed buffers.

Keep future flash persistence batched. Use the owner's conservative 100,000
erase-cycle budget as a design constraint, not as a verified flash rating or
a count of HTTP writes. No per-request flash writes or repeated recovery
flashing just for availability. See RAM_HISTORY.md for persistence boundaries.
