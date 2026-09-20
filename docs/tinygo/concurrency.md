# Concurrency and CPU cores

Compare [Rust's service and snapshot locks](../rust/architecture.md): its
ESP-IDF configuration uses both cores, while this TinyGo target uses one.

Concurrent users, concurrent requests and parallel CPU execution are different
things. A household can have many logged-in users without all of them issuing
a request at once. One browser can also issue several simultaneous requests
while drawing a single screen.

## Four workers, eight connections

The board currently has four HTTP workers and eight TCP connection slots.
A worker executes a request. A TCP slot also has to cover parts of the
connection lifecycle before or after handler execution, including closing.
That is why the counts differ.

More slots are not free throughput: each adds reserved network buffers.
More workers add stacks and working state, and can increase the peak memory
needed by allocations that remain. Slow clients and weak WiFi hold these
resources longer, reducing the rate at which other clients can use them.

Timeout processing must run even while traffic is arriving. Only reclaiming
connections when the accept loop is idle fails under sustained load, when
there may be no idle pass.

## Shared state still needs synchronization

The service lock protects domain reads and writes that must agree. A purchase
checks the listing and funds, commits, and updates the coordinated state under
that protection. Two simultaneous attempts cannot both observe an active
listing and independently sell it.

Idempotency has additional coordination around checking and publishing the
receipt. The receipt check alone is insufficient: two callers could both see
"not present" and then both execute the operation. The current striped gates
serialize matching operations; unrelated keys that share a stripe can also
serialize. This trades some concurrency for bounded lock storage.

Lock ordering matters when combining these mechanisms. Follow the existing
service/idempotency entry points rather than nesting callbacks that reacquire
the same gate or lock.

A single CPU does not remove logical races. A goroutine can yield between
checking state and changing it, allowing another request to intervene. Host
race tests also matter because the same code runs with ordinary Go on a
multi-core desktop.

## Keep slow I/O outside the domain lock

Streaming list responses copies or encodes one record while protected, then
releases the lock before a network write. A client with poor reception cannot
hold the household lock for the full duration of a long response.

This creates a live view rather than a whole-response snapshot. Sequence and
generation checks protect against slot reuse while iteration continues. Tests
must exercise writes, eviction and cancellation during streaming, not only
an unchanged store with a fast writer.

## What the two CPUs actually do

The ESP32-S3 has two main CPU cores. The current NanaCoin TinyGo target uses
the task scheduler on CPU0. Its Go application and radio integration share
that execution environment; it is not a configuration with WiFi assigned one
core and NanaCoin assigned the other.

In the current espradio integration, the core argument to its pinned-task
bridge does not establish hardware affinity: work is started as a goroutine.
CPU1 is not a second application worker pool.

Enabling both cores requires supported runtime and hardware integration:
secondary-core startup, per-core stacks, synchronization, interrupt handling
and garbage collection must agree. Selecting a scheduler flag is not enough
for this target. Upstream
[ESP32-S3 multicore work](https://github.com/tinygo-org/tinygo/issues/5353)
is relevant to follow, but should not be treated as an enabled feature here.
Espressif's own
[ESP-IDF FreeRTOS port](https://docs.espressif.com/projects/esp-idf/en/v5.4/esp32s3/api-reference/system/freertos.html)
supports multicore operation; adopting that environment would be a substantial
runtime integration or migration.

An additional CPU would not provide additional RAM or make shared-state
updates safe automatically. Before increasing concurrency, measure whether
the constraint is CPU work, memory, occupied TCP slots or the network.
