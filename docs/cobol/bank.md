# Bank decisions and durable writes

## Facts in, decisions out

The division of work follows the request's meaning. Rust resolves IDs into
bounded live records, obtains time, authenticates the requester and supplies
facts such as balances, roles, disabled status and object state. COBOL decides
whether the action is permitted and calculates the resulting financial or
state transition plan. Rust materializes that plan in its existing record
types.

For example, a transfer needs more than a subtraction. The adapters provide
the member and account facts; COBOL checks the relevant policy, funds and
numeric limits, then produces the posting values. Rust publishes the payment
record and applies those prepared values. Secret hashing and session lookup
stay in Rust; banking authorization rules operate on the resulting facts in
COBOL.

The implementation extends beyond this posting primitive. Its operations
cover currency reforms, loans and interest, foreign exchange, gifts, art,
lotto, marketplace listings and offers, fulfillment, member policy and audit
planning. Read operations also use COBOL for visibility, ordering, selection
and numeric aggregates. Rust retains text storage, UTF-8 comparisons where
needed, object lookup and JSON rendering.

## Prepare before append

The durable command path has three important steps:

1. Prepare and validate the complete plan with the selected bank engine.
2. Append the event through Rust's durable journal layer.
3. Install the already prepared changes in the live Rust records.

All fallible business calculations belong before the durable append. An
overflow or rejected settlement must not first become a committed event and
then fail while updating memory. In the COBOL adapter, `Prepared` caches the
posting legs and object changes, and application consumes them in the
expected order. Internal assertions check that the plan and event agree and
that every prepared component is consumed.

COBOL does not open bank files or write checkpoints. Rust continues to own
journal integrity, durable I/O, replay, retention and checkpoint recovery.
Replay prepares and applies an event using the selected engine, with the
current shared event schema. Tests reopen current data across the two builds
in both directions. This is engine interchangeability for the current schema;
it is not a collection of legacy storage decoders.

Rust also supplies external effects. The scheduler supplies the current time
and asks COBOL which settlements are due. Lotto randomness originates in
Rust, and the selected outcome is recorded durably before its payment is
applied. Replaying the bank must not draw a new winner.

## Tables and multiple calls

The fixed frame is intentionally small. Scalar policies fit into one call;
operations over collections use bounded `OCCURS` rows or fold chunks through
a numeric accumulator. Rust supplies each chunk, COBOL evaluates it, and Rust
uses the returned selection or totals. A larger transaction can also need
separate policy, posting and projection calls.

Consequently, C ABI call counts are not HTTP request counts. A conformance
run exercises many requests, repeated startup and replay, table scans and
individual boundary vectors. A high total needs its workload and call sites
attached before it says anything useful about production latency.

The table adapters carry compact values and indices rather than pointers to
Rust records. The returned indices let Rust retrieve the corresponding text
and construct a response. This keeps memory ownership in Rust while placing
the bank's selection rules in COBOL.

## Keeping both implementations useful

The regular Rust engine remains a build option with its own implementation.
Changing a bank rule means keeping the public behavior aligned across both
engines. Interface conformance tests assert responses, balances, permissions,
retry behavior and recovery through the running server. Differential tests
run equivalent scenarios against the two builds and compare results.

ABI vectors complement those tests by checking exact arithmetic and frame
layouts directly. They cannot by themselves prove JSON behavior or durable
recovery. The command inventory guards coverage when Rust's command surface
changes; it also does not replace execution tests. The [verification workflow](workflow.md)
explains how to run these layers without a board.
