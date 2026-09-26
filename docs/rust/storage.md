# NVS keys, recovery and retention

The Rust server stores its journal, checkpoints and archive in ESP-IDF NVS.
The [partition table](../../nanacoin_rs/partitions.csv) assigns `ledger` 8 MiB at
`0x410000`; device configuration has a separate NVS partition. NVS controls
physical placement and reclamation. Application slots do not measure erase
cycles or physical write amplification.

## Namespaces and keys

[NvsJournal](../../nanacoin_rs/src/bin/esp32/journal.rs) uses these logical keys:

| Namespace | Key | Value |
|---|---|---|
| `ncmeta` | `head` | 32-byte NCH1 generation/checkpoint publication record |
| `ncmeta` | `https_only` | Transport policy, retained across economy reset |
| `nanacoin` | `e0000`, `e0001`, … | Bank zero NCR2 events, used bytes only, at most 1,024 |
| `nanacoin` | `c0000`, `c0001`, … | Bank zero NCS2 rows, at most 4,096 bytes |
| `ncnext` | Same `e…` and `c…` keys | Bank one events/checkpoints |
| `ncarch` | `p0000` through `p03ff` | Archive slots, each at most 4,096 bytes |

Suffixes are hexadecimal slot indexes, not user or transaction IDs.
`generation % 2` selects the bank. Journal indexes restart after rotation while
domain sequences continue. Archive logical page IDs map modulo 1,024 to slots;
page headers verify the logical ID and economy incarnation.

## Encodings and state

HTTP uses JSON. NCR2 event, NCS2 checkpoint and NCA2 archive payloads use postcard
with caller-owned bounded buffers. CRCs cover critical headers and payloads;
decoders reject extra/truncated bytes. Rust field/enum ordering is part of this
positional schema. Older development data must be reset; there is no migration
or old-format decoder.

Checkpoint rows store settings, members/private credentials, business objects,
retry receipts, correction annotations and per-epoch lifetime counters. On file
and NVS adapters, transaction and sanitized audit history live in the archive,
not duplicate checkpoint history rows. Identity commands are redacted in audit.
Private checkpoints remain sensitive; CRCs are neither encryption nor authentication.

Stored transactions keep original currency units and classifications. Currency
reform changes current balances and obligations. APIs expose original postings
and an exact current-unit projection when possible. Corrections retain original
amounts and refunded units independently of cached history. Exact integer
lifetime counters survive archive pruning.

## Commit, rotate and recover

Each mutation validates and appends durably before changing live state. NVS
appends reject occupied event slots. Failed or ambiguous storage operations
latch the service until replay. Without a publication head, startup begins at
generation zero using the current format; this is not a legacy decoder.

Rotation occurs before the next event at 2,048 journal records, 1,024 unarchived
transactions, or 1,024 pending audits. The service stages archive pages, writes
and verifies an inactive checkpoint, publishes its head, then retires the old
bank. The checkpoint commits the archive interval, transaction floor, archived
counts and incarnation. Only that interval is visible. Unpublished orphan pages
cannot skip journal replay or duplicate balances.

The ring retains at most 768 pages with 256 slots reserved for staging without
overwriting committed pages. Missing/corrupt committed data fails closed.
Restore loads up to 3,000 transactions and 1,024 audits without applying postings,
then replays later events. Pruning also trims live history; commands are
revalidated after automatic rotation so refunds cannot depend on lost originals.

The raw `/state` response omits history. Transaction/account cursor routes scan
at most eight pages and return at most 100 rows; empty results may still have
continuations. Nana-only audit reads have bounded continuation too. The finite
archive is not an off-board backup.

Reset publishes empty state with a new incarnation, revokes sessions and returns
to provisioning. Old pages are ignored, not securely erased. Desktop storage
uses two checkpoint/log banks and a synchronized `.archive` bounded to 4 MiB;
back up all companions together while stopped.

See [the storage contract](../../nanacoin_rs/spec/STORAGE_V2.md) and
[retention/retry rules](../../nanacoin_rs/spec/RETENTION.md). Fault tests cover
orphans, ambiguous publication, pruning and replay. Physical power-cut testing
and board endurance measurement are separate from those deterministic checks.
