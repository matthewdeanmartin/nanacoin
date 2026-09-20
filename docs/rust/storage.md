# NVS keys, recovery and retirement

## What database is this?

It is an application journal/checkpoint protocol on ESP-IDF's **key/value
store**, not a filesystem, SQL engine, or hierarchical document database.
The [partition table](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/partitions.csv)
assigns `ledger` an 8 MiB NVS partition at `0x410000`. Device configuration's
separate `nvs` partition is not the ledger.

NVS maps a namespace and short string key to a value. Updating a logical key
does not imply rewriting one fixed physical address. NVS manages obsolete
entries, sector reclamation and wear leveling. Its storage-entry statistics
are not a count of application transactions or remaining erase cycles.
[NVS documentation](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32s3/api-reference/storage/nvs_flash.html)
explains its append-oriented internals.

## Exact keys: deliberately simple slot numbers

The implemented [NvsJournal](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/bin/esp32/journal.rs)
opens three namespaces in the same partition:

| Namespace | Key | Value / meaning |
|---|---|---|
| `ncmeta` | `head` | 32-byte `NCH1` publication record: generation, checkpoint row count, checksum |
| `ncmeta` | `https_only` | One-byte device policy: 0/absent allows HTTP; 1 requires HTTPS; survives economy reset |
| `nanacoin` | `e0000`, `e0001`, … | Bank 0 journal event frames, 1,024 bytes each |
| `nanacoin` | `c0000`, `c0001`, … | Bank 0 checkpoint rows, at most 2,048 bytes each |
| `ncnext` | Same `e…` and `c…` keys | Bank 1 journal and checkpoint rows |

The key builder is literally:

```rust
fn key(prefix: char, index: usize) -> Result<heapless::String<8>, Error> {
    let mut key = heapless::String::new();
    core::fmt::Write::write_fmt(&mut key, format_args!("{prefix}{index:04x}"))
        .map_err(|_| Error::Capacity)?;
    Ok(key)
}
```

These are hexadecimal **slot indexes**, starting at zero: slot 10 is `e000a`,
slot 16 is `e0010`. They are not user IDs, transaction IDs, timestamps or
composite primary keys such as `user-transaction-type`. `ncmeta/head` is notation
for namespace plus key, not a path containing a slash.

`generation % 2` selects the active bank. These banks are logical namespaces,
not fixed physical halves of flash. After retirement, journal slot numbering
starts at `e0000` again, while domain IDs remain monotonic. A generation and
slot together locate an event logically; the generation is in `head`, not
encoded into each key string.

## Where users and balances live

The event payload holds the command and its actor/sequence/retry information.
An event can change multiple accounts atomically at the application level.
There are no NVS secondary indexes for “all Alice's transactions.” Boot replays
the checkpoint and following events into bounded RAM state. API queries inspect
that RAM state and retained history, rather than searching NVS by user prefix.

Checkpoint row indexes also run sequentially across types, not per entity.
[checkpoint.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/journal/checkpoint.rs)
writes a header followed by members, listings, history, offers, quotes and retry
receipts. The row's `NCS1` envelope stores kind, payload length and checksum;
the payload is JSON. Kind 0 is the header, 1 member, 2 listing, 3 history,
4 offer, 5 quote, 6 retry receipt. Therefore `c0001`'s meaning comes from the
header counts, row kind and payload—not from a clever key.

Private member rows include recovery credentials; the public member serializer
does not. Flash dumps and desktop checkpoint files are sensitive. A checksum
detects accidental corruption; it is not encryption or authentication.

## Commit and recover

For each accepted mutation, the service validates, encodes a fixed frame and
calls `append` before changing live RAM. The NVS adapter rejects an occupied
event slot, then calls `set_blob`. A persistence error latches the service
unavailable until restart. This is per-mutation persistence, not daily batching.

On boot, the adapter reads `ncmeta/head`; its absence selects legacy generation
0 with no checkpoint. A present invalid head fails. The service restores the
committed checkpoint, reads numbered events until the first absent slot, then
checks domain invariants. Invalid present records fail closed. The desktop
adapter's incomplete-tail truncation is a separate file-specific behavior.

## Closing books is not deleting balances

Before the next append after 2,048 records, supporting adapters save a new
checkpoint. Legacy replay can read up to 4,096 records. Under the service lock:

1. Clear the inactive namespace, preserving the active generation.
2. Write bounded checkpoint rows and read each back for verification.
3. Commit the replacement namespace, then publish the new `head`.
4. Only after publication, reclaim the previous namespace.

Balances, live objects, credentials, recent 365 transactions and up to 4,096
retry receipts survive. Older detailed events are retired; this is not an
archival accounting export. NVS's physical reclamation is separate from this
application-level decision about history.

The code calls `nvs_commit` for the completed checkpoint. This does **not** mean
earlier `nvs_set_blob` calls buffered the entire replacement solely in RAM;
they may already program flash. Safety comes from keeping the old generation
authoritative until the new head is published.

Nana's authenticated `/admin/checkpoint` and `/admin/reset` actions include the
expected generation and sequence to reject stale confirmations. Reset also
requires `RESET ECONOMY`, publishes empty state, revokes sessions and returns
the app to provisioning. It is a logical reset, not physical secure erasure.

The [retention contract](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/RETENTION.md)
describes retry generations, failure recovery and the desktop companion files.
The [checkpoint tests](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/tests/checkpoint.rs)
exercise reboot/replay, retirement and interrupted publication with test stores.
Hardware power-cut validation remains distinct from passing those tests.
