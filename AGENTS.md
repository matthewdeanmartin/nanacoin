# NanaCoin agent instructions

## Development data and schema policy

NanaCoin is pre-release and has **zero users and zero production data on any
board anywhere** (owner confirmed September 21, 2026). Existing journals,
checkpoints, browser storage, demo data and fixtures are disposable development
data. Do not assume they need migration or backward compatibility.

- Prefer the clean current schema and implementation. Breaking API/storage
  changes and resetting/reseeding affected development data are acceptable when
  needed for the requested work.
- Do not spend time designing migrations, retaining legacy decoders, supporting
  multiple schema versions, or testing old-data compatibility unless explicitly
  requested. Update affected fixtures and documentation to the current model.
- Do not ask for approval merely to preserve or migrate disposable data. Avoid
  adding compatibility scaffolding just because an older spec mentions it.
- Keep current-schema crash recovery, durable writes, replay, checkpoints,
  idempotency and accounting invariants correct. Disposable old data does not
  make duplicate payments or corrupt new data acceptable.
- This policy does not authorize unrelated deletion, credential removal, or
  flashing/erasing hardware. Scope any reset to the requested development work.

When the owner announces the first real user, freeze the schema and adopt
explicit migrations and compatibility planning. Update this policy then; do
not prematurely apply production migration requirements now.

## Current implementation

The active server is `nanacoin_rs`; the shared Angular client is `nanacoin_ui`.
Consult their source and specs, but treat this root development-data policy as
authoritative over older migration/compatibility guidance.
