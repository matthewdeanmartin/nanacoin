# Littlefs-backed ledger — proposal only

Status: not implemented. The board currently uses ESP-IDF NVS. This epic
does not authorize changing the partition table, migrating data, formatting
storage, flashing firmware, or resetting the owner's economy.

## Motivation and baseline

NVS already supplies wear leveling and recovery for key/value storage. Our
8 MiB `ledger` partition contains namespaces `nanacoin`, `ncnext`, and
`ncmeta`; numbered event/checkpoint blobs plus a generation head implement
the accounting store. These are logical keys, not fixed physical sectors.
See `src/bin/esp32/journal.rs` and `RETENTION.md`.

A filesystem is a more natural abstraction for sequential journals and
checkpoint streams. The proposal is littlefs beneath our existing bounded
accounting protocol, not a general SQL database and not a new transaction
model. Keep small device configuration in NVS.

## Candidate layout and protocol

Candidate files: a committed manifest, two checkpoint generations, and their
journal files. Names/layout are undecided. Append a complete checksummed event,
sync it before acknowledging success, and reconstruct bounded RAM state on
boot. For rotation, write/sync a replacement checkpoint, publish its manifest
using the filesystem's supported atomic operation, then reclaim retired files.
Keep the service lock and existing stale-retry generation rules. Confirm exact
littlefs/VFS sync, rename and error behavior before choosing the protocol.

The filesystem handles obsolete physical blocks; the application still decides
which accounting history may be retired. A file that grows forever still fills
the board. Retain enough free space for the active generation, its replacement,
filesystem metadata and recovery headroom. Refuse safely before exhaustion.

## Benefits

- Sequential file layout fits journals and checkpoints; fewer individual keys.
- Common file-oriented interfaces could simplify host/board adapter reasoning.
- Bounded littlefs caches can fit the preallocation policy.
- Filesystem-managed allocation, reclamation and dynamic wear leveling.

## Costs and risks

- NVS already wear-levels; fewer erases or faster writes are hypotheses to
  measure, not promised benefits.
- Another dependency, flash driver/VFS integration, buffers and failure modes.
- Atomic file operations do not make several files one database transaction.
- Copy-on-write metadata and syncing have write amplification and latency.
- Dynamic wear leveling is not full static wear leveling across unchanged data.
- Power failure, full-volume behavior, cache-disabled operations and lock order
  require real hardware tests. No automatic format on mount failure.
- Encryption/security parity must be reviewed explicitly; logical deletion and
  reset are not secure erasure of physical remnants.
- Existing NVS data cannot simply be mounted as littlefs.

## Migration and acceptance gates

First build a disposable test adapter and failure-injection suite. Preserve all
current financial, checkpoint, idempotency and reset tests. Test interrupted
append/sync/publication/cleanup and corruption, both on host and later through
authorized hardware power cuts. Measure RAM, maximum request latency, physical
write/erase amplification and usable capacity with realistic and full datasets.

Design an explicit backed-up migration with verified balance/state equivalence,
credentials protection, rollback boundaries and sufficient spare storage. If
space is insufficient, require an explicit export/restore workflow; do not
silently destroy the existing NVS partition. Export/import is not implemented
today. Only adopt after measurable benefit and a reviewed recovery protocol.

References:

- [NVS](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32s3/api-reference/storage/nvs_flash.html)
- [littlefs guarantees and API](https://github.com/littlefs-project/littlefs)
- [littlefs design and wear leveling](https://github.com/littlefs-project/littlefs/blob/master/DESIGN.md)

Keep this proposal outside `docs/`; published implementation documentation
must continue describing NVS until a migration is actually implemented.
