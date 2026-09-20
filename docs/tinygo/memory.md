# Memory: capacity is part of the design

For the implemented Rust counterpart, including its PSRAM allocator settings
and the distinction between inline capacity and heap reservation, see
[Rust memory and ownership](../rust/memory.md).

An ordinary server can often tolerate creating a request object, building a
response, growing a slice and letting the collector clean up afterward. On
this board, a burst of those temporary objects competes with WiFi buffers,
goroutine stacks and the household's permanent state.

The goal is predictable memory use over a long run, including the worst valid
request. Reducing average allocation is helpful but does not establish that.

## Allocation, retention and fragmentation

These are different problems:

- **Allocation rate:** how quickly work creates new heap objects.
- **Retention:** how much remains reachable and therefore cannot be collected.
- **Fragmentation:** free space exists, but its layout cannot satisfy a needed allocation.
- **Peak working set:** everything that must coexist while requests are in flight.

A falling post-GC free-heap reading can suggest retention, but one reading
does not identify its owner. A crash during a burst may be a temporary peak.
A free-byte count does not tell you the largest available contiguous block.
TinyGo collection can recover unreachable objects; it cannot make live data
disappear. See [diagnostics](diagnostics.md) for measurements that separate these cases.

## Allocate the budget at startup

NanaCoin reserves bounded stores and buffers, then reuses their slots:

| Resource | Current capacity | When full |
|---|---|---|
| Transaction history | 365 slots | Evict oldest history; preserve lifetime balances and sequence IDs |
| Event/error log | 58 entries | Overwrite oldest event |
| Users | 16 | Refuse further creation |
| Accounts | 32 | Refuse further creation |
| Listing slots | 48 | Closed listings may be reclaimed; active offers are protected |
| Shared text arena | 12 KiB | Reclaim eligible history/closed data or reject an operation |
| Retry receipts | 16 entries, 4 KiB total response bodies | Evict oldest receipts to satisfy both limits |
| HTTP workers | 4 | Additional work waits on or encounters transport capacity limits |
| TCP slots | 8 | New connections depend on slots becoming available |

These are current implementation limits, not independent promises. For
example, 48 listing slots do not guarantee that 48 maximum-length descriptions
fit in the shared text budget. Long memos can shorten history below 365 records.
See [storage](storage.md) for retention semantics.

Increasing a fixed capacity avoids later growth, but consumes more RAM from
the beginning. Doubling the ledger can leave less room for a single incoming
connection. Choose capacities together and measure after startup allocation.

## Compact storage and public objects

The ledger stores packed records with inline pairs of postings and sequence
numbers. Stable identities use interned strings: store a repeated value once
and refer to it by a small index. Variable text uses an arena: a reserved block
of memory divided into reusable allocations with explicit ownership.

The API still exposes ordinary JSON fields and string IDs. Compact internal
storage need not become an inconvenient public API. It does mean ownership
matters when rendering: a pointer into a recyclable slot cannot safely escape
while another operation may overwrite that slot.

## Temporary buffers also need owners

Each request borrows working state and returns it when finished. Typed JSON
readers and writers avoid reflection and large general-purpose intermediates.
Lists render a record into borrowed storage before sending it, rather than
constructing an entire response array.

There are still allocations in the full stack. Converting bytes to an owned
string, taking an optional parsing path, or allocating inside a dependency can
cost memory even when all domain tables were reserved at startup. A fixed
buffer is not proof that the complete request is allocation-free.

For every reusable buffer, answer:

1. Who owns it now?
2. Can another goroutine access it?
3. Does a returned string or slice still point into it?
4. What happens if the response exceeds it?
5. Is it returned on every error path?

`sync.Pool` is not a fixed-capacity allocator: entries can disappear during GC
and misses can create new objects. Explicit bounded pools make the resource
contract clearer for this application.

## The stack is small too

The current ESP32-S3 goroutine stack is 8 KiB. A large fixed local array, a
struct passed by value, or a two-value range over an array can introduce a
large copy. A program can therefore crash after a change intended to eliminate
heap allocation.

Keep large reserved arrays behind pointers, iterate indices when copying an
array would be costly, and inspect compiled frames after changing large
buffers. Startup preallocation moves the cost to a controlled point; it does
not remove the cost.

## Measure the implementation you ship

TinyGo can report allocation sites with `-print-allocs`. For example:

```powershell
tinygo build -target=esp32s3-generic -print-allocs='internal/core|internal/api|eventlog' -o "$env:TEMP/nanacoin-allocs.elf" ./cmd/nanacoin-esp32
```

The report identifies possible sites, including startup allocations. It does
not say how frequently they execute. See the
[compiler option reference](https://tinygo.org/docs/reference/usage/important-options/).

Desktop benchmarks help compare algorithms, but have a different runtime and
allocator. Exclude the benchmark harness when measuring handler allocations:
creating a fresh `httptest` request and recorder can dominate the result.
Then validate the change on the board with a known workload and serial capture.
