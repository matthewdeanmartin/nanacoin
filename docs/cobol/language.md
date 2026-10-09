# COBOL language and coding patterns

## The shape of the program

`bank.cob` uses free-format source, selected by `>>source format free`. Its
`IDENTIFICATION DIVISION` names `NCBANK`. The `DATA DIVISION` declares constants,
temporary values and caller-owned records. The `PROCEDURE DIVISION` initializes
the result, uses `EVALUATE operation-code` to dispatch to a paragraph, and
returns to C with `GOBACK`.

Paragraphs are named procedures invoked with `PERFORM`. Helpers share data
within this invocation; they do not have Rust-style argument lists or lexical
local variables. `EXIT PARAGRAPH` provides an early return from the current
paragraph. The caller must check `result-code` after a helper can reject an
operation. `EVALUATE TRUE` expresses ordered guards, while `PERFORM VARYING`
implements table scans and folds.

## Named fields over the ABI frame

The original implementation referred to numbered `slot(...)` entries. The
current source uses named `REDEFINES` records over the same 512-byte
`values-frame`. For example, `rescale-units`, `rescale-exponent` and
`rescale-converted-units` state the role of each word directly; the
[interop example](interop.md#a-complete-small-example) shows the declaration.

`REDEFINES` aliases storage. Declaring another operation's record does not
allocate another frame, perform a conversion or copy data. `FILLER` reserves
unused bytes so each record preserves the ABI layout. The operation code or
sub-operation selector determines which interpretation is valid. COBOL does
not enforce that choice as a Rust enum would.

Level numbers describe record structure: `01` starts a record, and higher
numbers describe its children. The numbers are hierarchy markers, not byte
offsets. `BINARY-LONG` provides the 32-bit operation/result fields and
`BINARY-LONG-LONG` the 64-bit frame fields on these targets. `PIC X(n)` exposes
a fixed number of raw bytes for text or explicitly encoded display values.

Some operations deliberately repurpose the frame for a helper. They move
inputs into that helper's named view before performing it, then read the
helper's outputs. Because the views overlap, their inputs and outputs must be
reviewed together. A table scan likewise must finish reading any inputs that
its output writes would overwrite.

`OCCURS` creates fixed-size arrays of records, such as quote rows containing
side, price and ID. COBOL subscripts start at one, while the corresponding
Rust arrays start at zero. The named rows replace manual word-offset
arithmetic in business paragraphs; the Rust packing code still needs to
agree with their exact layout.

## Scope and lifetime

| Declaration | How this bank uses it |
|---|---|
| `78` constants | Compile-time names for operations, results, selectors and limits; no mutable payload |
| `WORKING-STORAGE` | Holds those constant declarations in the current program |
| `LOCAL-STORAGE` | Fresh arithmetic, loan, selection and display scratch for each invocation |
| `LINKAGE` | Views of the caller's operation, frame and result; COBOL does not own that memory |

The mutable scratch was moved out of persistent `WORKING-STORAGE` when the
program was humanized. Groups such as `arithmetic-scratch` and `loan-scratch`
organize it by purpose. They improve readability but do not enforce privacy:
all paragraphs in `NCBANK` can see the invocation's declared data. Separate
COBOL subprograms would give stronger separation.

These variables are not process-wide globals in the C sense, and the source
does not declare COBOL `GLOBAL` or `EXTERNAL` data. Nevertheless, shared
program-level declarations are a normal COBOL style. Per-call lifetime and
paragraph-level scope are different questions.

In the current GnuCOBOL 3.2 generated C, local storage is a 624-byte heap
allocation initialized for each call and freed on return. `LOCAL-STORAGE`
therefore must not be described as automatically stack allocated. The
Rust mutex still protects runtime internals, as explained in [interop](interop.md).

## Exact money and explicit failures

The frame carries integer monetary units. Packed-decimal scratch fields such
as `PIC S9(38) COMP-3` provide wider decimal intermediates for multiplication,
interest, exchange and rescaling. They do not expand the ABI's output range:
results must still satisfy bank limits and fit their destination fields.

Named constants include `wallet-limit` (`9007199254740991`, or 2^53 - 1),
`command-amount-limit` (`1000000000000000`), `seconds-per-day` and
`basis-points-per-whole`. Wide flow totals use high/low parts with base 10^15;
that representation is separate from the command amount limit.

An excerpt from exact rescaling shows the rule against rounding:

```cobol
        divide rescale-units by factor-value giving quotient-value
            remainder remainder-value
        if remainder-value not = 0
            move result-conflict to result-code
            exit paragraph
        end-if
        move quotient-value to wide-value
```

`COMPUTE ... ON SIZE ERROR` detects arithmetic size failures. Explicit bounds
checks enforce the bank's narrower limits. `DIVIDE ... REMAINDER` preserves
fractional carry where a policy requires it, and rejects inexact conversions
where exactness is required. `FUNCTION ABS`, `MIN`, `MAX` and `MOD` support
these calculations.

The bank returns named result codes rather than throwing exceptions. A
failure leaves the output unsuitable for application; Rust decodes outputs
only after success. Floating-point fields are used for final display rates
and percentages, with explicit byte views for the ABI. Money and interest
accounting use integer and decimal calculations.
