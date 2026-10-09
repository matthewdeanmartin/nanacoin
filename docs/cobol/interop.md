# Rust, C and COBOL interop

## One in-process function call

The browser still sends JSON to the Rust server. Rust resolves record IDs and
collects facts, then calls the selected bank through an ordinary C ABI:

```text
JSON request
    -> Rust HTTP handler and domain adapter
    -> Rust operation wrapper in src/cobol.rs
    -> nc_bank_v7(operation, frame) in bridge.c
    -> NCBANK(operation address, frame address, result address)
    -> COBOL paragraph selected by operation-code
    -> result code and output fields
    -> Rust prepared records / JSON response
```

There is no subprocess or JSON serialization between Rust and COBOL. Both
execute in the same process. The JSON wire contract belongs to the Rust HTTP
layer; the native function boundary is a separate, private contract.

## The native contract

The current contract is ABI version **7**, with **64 signed 64-bit words** in a
**512-byte frame**. The operation and result codes are signed 32-bit integers.
Rust creates a zeroed `[i64; 64]`, writes inputs, passes its address and reads
outputs after the call. Some operations interpret designated bytes as text or
binary64 display values instead of integers.

Rust's function-pointer type is:

```rust
type BankCall = unsafe extern "C" fn(i32, *mut i64) -> i32;
```

The bridge translates that signature to GnuCOBOL's generated entry point:

```c
extern int NCBANK(unsigned char *, unsigned char *, unsigned char *);

NC_EXPORT int32_t nc_bank_v7(int32_t operation, int64_t *slots)
{
    static int initialized = 0;
    int32_t result = -1;
    if (!slots) return 1;
    if (!initialized) {
        cob_init(0, NULL);
        initialized = 1;
    }
    NCBANK((unsigned char *)&operation, (unsigned char *)slots,
           (unsigned char *)&result);
    return result;
}
```

COBOL's `PROCEDURE DIVISION USING operation-code values-frame result-code`
receives those three addresses. The bridge returns the explicit `result-code`
field; the generated `NCBANK` C return value is not the bank's error protocol.
The casts expose the same bytes, without copying the frame.

This contract uses native integer representation and byte order on each
supported target. It does not pass Rust strings, vectors, references to domain
objects or enum layouts. The C bridge's null check cannot validate a buffer's
length: the Rust wrapper must provide all 512 bytes and valid operation inputs.

## A complete small example

The Rust wrapper for exact decimal rescaling is:

```rust
pub(crate) fn rescale(value: i64, exponent: i16, limit: i64) -> Result<i64, Error> {
    call(2, &[value, exponent.into(), limit]).map(|out| out[3])
}
```

COBOL sees a named record over those bytes:

```cobol
01 rescale-frame redefines values-frame.
   02 rescale-units usage binary-long-long signed.
   02 rescale-exponent usage binary-long-long signed.
   02 rescale-limit usage binary-long-long signed.
   02 rescale-converted-units usage binary-long-long signed.
   02 filler pic x(480).
```

The first three words carry the inputs; the fourth carries the output. For
`1000, -1, 9007199254740991`, COBOL returns success and `100`. For `1001` with
the same exponent, division leaves a remainder and returns a conflict: it
does not silently round money. Rust maps the numeric status to its domain
`Error` enum. Other statuses distinguish overflow, insufficient funds,
forbidden actions, capacity and specific business conflicts.

## Initialization, lifetime and concurrency

On Windows, Rust loads the DLL from the absolute `NANACOIN_COBOL_DLL` path and
keeps the library alive for the process lifetime. On ESP-IDF, the same C
exports are linked statically. Both paths check the ABI version, frame size
and operation capability mask (`0xFFFFFFFFFFFFFFFE`, bits 1 through 63).

A `OnceLock` caches initialization, and a `Mutex` serializes calls, including
the bridge's first `cob_init`. Per-call COBOL scratch storage does not make all
GnuCOBOL runtime internals independently thread-safe. Removing this lock would
require a separate runtime concurrency investigation.

The frame belongs to Rust and remains valid throughout the synchronous call.
COBOL retains no caller pointers. Its local scratch storage is released on
return; Rust then decodes the output into ordinary Rust values. The runtime is
not torn down with `cob_tidy` while the bank is in use.

A missing DLL or incompatible module fails initialization. A COBOL build does
not silently switch to Rust banking rules. See [builds and runtime](workflow.md)
for how the two artifacts are selected.
