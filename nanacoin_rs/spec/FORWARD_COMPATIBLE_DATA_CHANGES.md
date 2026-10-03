# Forward-compatible data changes

How to change what the board stores without making an upgrade throw away the
household. Written after API keys (September 29, 2026) nearly did exactly that.

Scope: the durable formats in `src/journal.rs`, `src/journal/checkpoint.rs` and
`src/journal/archive.rs`. The JSON API and the Angular client are covered at
the end; they are much more forgiving.

Relation to the root `AGENTS.md` policy: that policy says development data is
disposable and migrations are unnecessary. It is still right about fixtures,
demo data and browser storage. It is no longer safe to assume about a board
that a household uses day to day. Where a change can keep existing data at
little cost, keep it, and prove it with a fixture test (below).

## What went wrong

API keys added two fields to each member: `api_key` (a SHA-256) and
`api_key_created`. The first version put them in the middle of the
checkpoint's member row:

```rust
struct StoredMember {
    member: Member,
    password: Option<PasswordVerifier>,
    token_hash: TokenHash,
    api_key: TokenHash,        // new
    api_key_created: u64,      // new
    last_command: TokenHash,
    last_sequence: u64,
}
```

Every test passed, because every test wrote its data with the new code and read
it back with the new code. A board already holding a checkpoint from the
previous firmware would have failed at boot with `CorruptJournal`: the new code
reads a member row expecting at least 33 more bytes (a 32-byte hash and a
varint) than the old code wrote. The only way
back would have been to reset the economy.

The fix (commit after `0a43269`) left the member row exactly as it was and wrote
API keys as extra rows at the end of the checkpoint. An old checkpoint simply
has none. `tests/checkpoint.rs::pre_api_key_checkpoint_still_opens_and_then_keeps_keys`
opens files written by the pre-API-key firmware (`d8301d3`) and proves it.

Caveat for that one release: a board flashed with `0a43269` that then saved a
checkpoint holds the abandoned layout, which current firmware cannot read.

## Why this format is unforgiving

All three durable formats encode records with **postcard**, which is
positional. A struct is its fields in declaration order with no names and no
lengths. An enum is a variant index followed by that variant's fields.

| Format | Magic | Unit | Where |
| --- | --- | --- | --- |
| Journal | `NCR2` | 1024-byte frame holding one `Event` | `journal.rs` `encode`/`decode` |
| Checkpoint | `NCS2` | row with a kind byte (0 header, 1 member, … 16 API key) | `journal/checkpoint.rs` |
| Archive | `NCA2` | page of retired history and audit records | `journal/archive.rs` |
| Checkpoint head | `NCH1` | generation and row count | `journal/checkpoint.rs` |

Each decoder also insists that the bytes are used up exactly
(`remaining.is_empty()`), and the checkpoint restore checks that the header's
counts add up to the rows on disk. Those checks catch corruption. They also
catch every layout change.

Consequences:

- `#[serde(default)]` does **nothing** for postcard. It helps only
  self-describing formats such as JSON. Several structs here carry it (for
  example `Header::things`); it does not make them tolerant of older data.
- `#[serde(skip)]` fields are not stored at all. Adding one is safe for the
  encoding, but its value is lost on restore unless something else saves it.
- Renaming a field or variant is free for storage. Moving, inserting,
  removing or retyping one is not. (Some of the same types, such as `Command`
  and `Member`, are also JSON on the HTTP API, where names do matter.)

## Safe and unsafe changes

| Change | Journal (`Command`, `Event`) | Checkpoint rows | Safe? |
| --- | --- | --- | --- |
| Rename a field or variant | | | Yes for storage; check the JSON API |
| Add a new `Command` variant **at the end** | Old frames keep their indices | | Yes |
| Add a variant anywhere but the end | Every later index shifts | | **No** |
| Remove or reorder variants | Old indices point at the wrong thing | | **No** |
| Add a field to an existing struct or variant | Old frames are too short | Old rows are too short | **No** |
| Change a field's type (`u32` to `u64`, `String<32>` to `String<64>`) | Changes the byte layout | Same | **No**, except where postcard's varint encoding happens to agree; do not rely on it |
| Add a new checkpoint row kind written **after** all counted sections, with restore allowing extra rows | | Old checkpoints have zero such rows | Yes |
| Add a count to `Header` | | Header layout changes | **No** |
| Add `#[serde(skip)]` state rebuilt by replaying the journal | | | Yes, if the journal still holds every event that set it. Retired history does not, so checkpointed state must also be saved somewhere |

## Patterns that work

### 1. New command: append a variant

`Command::SetApiKey` was added as the last variant, so every existing journal
frame decodes as before. New firmware reads old journals. Old firmware cannot
read a journal holding the new variant, which is expected: firmware only moves
forward.

Never insert a variant in the middle for tidiness. Add a comment above the
enum's end if the ordering looks odd.

### 2. New state: a trailing checkpoint row kind

This is how API keys are stored:

```rust
#[derive(Serialize, Deserialize)]
struct ApiKeyRow { member: MemberId, key: TokenHash, created: u64 }
const API_KEY_ROW: u8 = 16;
```

- `save` writes one row per member who has a key, after every counted section.
- `restore` computes the rows the header accounts for, and treats the
  difference between that and the rows on disk as API key rows. It rejects a
  negative difference or more rows than members.
- Each row is validated (known member, non-zero hash, at most once, not in the
  future) like any other row.

For a second piece of new state, do not add another uncounted trailing kind.
Two uncounted kinds cannot be told apart by count. Instead write a single
trailing **extension header** row (a new kind) that carries counts for
everything added after it, followed by those rows. From then on there is one
place to grow.

That extension now exists (bots, October 2026). Layout after the counted
sections: one `Extension` row (kind 17), the rows it counts, then the API key
rows (kind 16) as before. `restore` peeks at the kind of the first row after
the counted sections: 17 means an extension, anything else (or nothing) means
an older checkpoint. `Extension::values` is a list of slots that only ever
grows; a slot an older checkpoint lacks reads as zero:

| Slot | Meaning | Rows |
| --- | --- | --- |
| 0 | read-only API keys | `ReadKeyRow` (18) |
| 1 | bot members (`MemberKind::Bot`) | `BotRow` (19) |
| 2 | lottos with tickets held by members 17-32 | `LottoTicketsRow` (20) |
| 3 | `credit_blocked` bits of members 17-32 (a value, not a count) | — |

To add state: append a slot, write its rows after the existing extension
rows, in slot order. Never reorder or reuse a slot.

The same change raised `MEMBERS` from 16 to 32 without touching old rows:
`Lotto::tickets` serializes only members 1-16 (`lotto.rs`, `first_sixteen`),
`Header::credit_blocked` stays a `u16`, and members 17-32 live in the
extension. Bot members and read keys arrived as new `Command` variants at the
end (`CreateBot`, `SetReadKey`) rather than new fields on `CreateMember` and
`SetApiKey`, and `Member::kind` is a `#[serde(skip)]` sidecar.

### 3. New field on an existing record: a sidecar, not an edit

When a record needs a new field, leave the record alone and keep the value
beside it: in a trailing row keyed by the record's id (pattern 2), or in
`#[serde(skip)]` state set by a new command (pattern 1). API keys are exactly
this: logically a member field, stored as a sidecar.

### 4. When a break is unavoidable: a new magic

If a format genuinely has to change shape, change its magic (`NCS2` to `NCS3`)
and keep a decoder for the old one that converts on load. The next checkpoint
then writes only the new format, so the old decoder can be deleted after every
board has checkpointed once. Never change a layout while keeping the magic.

## Prove it with a fixture

A round-trip test cannot catch these mistakes: it writes with the new code and
reads with the new code. Only bytes written by the **old** code can.

`tests/fixtures/pre-api-keys/` holds a small household written by `d8301d3`:
two members, one checkpoint, and a journal record after it. The directory's
`.gitattributes` marks the files binary so git never rewrites line endings in
them.

To make a fixture for a future change:

1. `git worktree add <scratch>/old <last-released-commit>` (outside the repo).
   If cargo complains about a workspace, append `[workspace]` to the scratch
   copy's `Cargo.toml`.
2. In the old tree, add a throwaway test that builds a household with
   `FileJournal`, calls `checkpoint`, then executes at least one more command so
   both the checkpoint and the journal tail are covered.
3. Run it with an output directory, copy the `economy.journal*` files into
   `tests/fixtures/<name>/`, add the `.gitattributes`, remove the worktree.
4. In the current tree, add a test that copies the fixture to a temporary
   directory, opens it, checks balances and invariants, exercises the new
   feature, checkpoints, and reopens.

Keep old fixtures. Each one guards a released format.

| Fixture | Written by | Guards |
| --- | --- | --- |
| `tests/fixtures/pre-api-keys/` | `d8301d3` | checkpoints and journals from before API keys |
| `tests/fixtures/pre-bots/` | `64f1c7f` | an API key, a lotto with tickets, a funded loan and a quote, from before bots, read keys and 32 members (`tests/bots.rs`) |

## Before merging a storage change

- Did any struct written by `write` in `checkpoint.rs`, or any type reachable
  from `Event`, change shape? If so, which pattern above covers it?
- Is every new `Command` variant at the end?
- Does a fixture from the last released firmware still open?
- Does the change touch credential handling? New credential-bearing commands
  must also be refused by `/api/v1/commands` and by `Audit::validate_public`,
  as `SetApiKey` is.

## JSON API and the Angular client

The HTTP API is JSON, so it is far more tolerant:

- Adding a response field is safe; the client ignores what it does not know.
- Request structs use `#[serde(deny_unknown_fields)]`, so a newer client
  sending a new field to an older board gets `400`. Deploy the board first, or
  send the field only when it is set (as the market does with `side`).
- Browser storage (`localStorage` keys such as `nanacoin:theme`,
  `nanacoin:recipe-units`, `nanacoin:mastodon:*`) is per device. Read it
  defensively: unknown or malformed values fall back to a default, never throw.
