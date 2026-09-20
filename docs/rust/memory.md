# Memory and ownership

## Three different resources

SRAM and PSRAM are volatile working memory. Flash retains the journal after
power loss. Saving a checkpoint does not move the live state out of RAM.

The actual [SDK defaults](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/sdkconfig.defaults)
include:

```text
CONFIG_SPIRAM=y
CONFIG_SPIRAM_MODE_OCT=y
CONFIG_SPIRAM_SPEED_80M=y
CONFIG_SPIRAM_USE_MALLOC=y
CONFIG_SPIRAM_MALLOC_ALWAYSINTERNAL=16384
CONFIG_SPIRAM_MALLOC_RESERVE_INTERNAL=65536
CONFIG_NVS_ALLOCATE_CACHE_IN_SPIRAM=y
```

ESP-IDF's allocator prefers PSRAM for allocations above the configured
threshold, and internal RAM for smaller ones. This is a preference, not a
guarantee that every particular Rust object lives in one region. The reserved
internal pool serves allocations with internal-memory requirements. The NVS
cache is explicitly configured for PSRAM. See
[ESP-IDF's allocator explanation](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32s3/api-guides/external-ram.html).

## Fixed capacity is explicit

From [domain.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/domain.rs):

```rust
use heapless::{String, Vec};
pub const MEMBERS: usize = 16;
pub const LISTINGS: usize = 48;
pub const HISTORY: usize = 365;
pub type Name = String<40>;
pub type Memo = String<96>;
```

Here `String<40>` means a UTF-8 string with inline capacity for 40 **bytes**,
not 40 arbitrary Unicode characters. This `Vec` is `heapless::Vec`, not the
growable standard-library vector. Its capacity is part of its type. The
application handles capacity errors instead of letting live users or listings
be overwritten as if they were disposable history.

Not everything is inline. From
[journal.rs](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/src/journal.rs):

```rust
let mut state = Box::new(State::default());
let mut frame = [0; FRAME_SIZE];
let mut records = 0;
let mut keyed = VecDeque::with_capacity(MAX_RECORDS);
```

`Box<State>` owns one heap-allocated state. Moving the box moves a pointer,
not the full state array. A `VecDeque` allocates its backing storage once here;
service logic bounds it to 4,096 retry receipts. The recent transaction deque
is similarly bounded to 365. A capacity reservation alone is not a hard limit:
the ring's explicit eviction/checks enforce that limit.

## Lifetimes are not a promise of zero allocation

`&State` borrows state without owning or copying it. `&mut State` grants
exclusive mutable access for that borrow. Values normally release owned
resources through `Drop` when their scope ends; Rust has no tracing GC in this
application. A mutex guard unlocks when dropped, including on an early return.

Regular domain HTTP replies reuse a **512 KiB thread-local response buffer**;
that allocation persists for its worker's lifetime. Diagnostics instead use a
4 KiB request-local array. Neither claim means all HTTP or TLS allocations are
gone: ESP-IDF, TLS and NVS have their own memory needs. Stacks are fixed and
large local arrays still consume stack space.

The diagnostic snapshot is at most 256 bytes, its shared container at most
288 bytes, and the dedicated sampler stack is 4 KiB. HTTP task stack is
24 KiB; startup stack is 64 KiB. These are different budgets.

[Allocation tests](https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/tests/allocation.rs)
exercise bounded application paths after startup. They do not establish zero
allocation by the board's network stack. Read internal free bytes, largest
free block and PSRAM separately; abundant PSRAM cannot satisfy every
internal-only allocation.
