# Optional GnuCOBOL bank

**P4 hardware verified October 7, 2026:** the optional COBOL image was deployed
to the owner's revision-1.3 board with an application-only, hash-verified write.
UART confirms `Banking engine: cobol`; the strict TLS/API/all-75-assets probe
passes and existing bank counts are unchanged. Under that workload internal
heap minimum is 259,671 bytes, PSRAM minimum 30,050,920 bytes and observed free
serving/TLS stacks are 22,500/22,104 bytes. The emulator startup blocker is not
reproduced on hardware. Maximum-capacity financial workloads on P4 remain
unverified; no financial writes were made. See [deployment evidence](../DEPLOY.md).

The default build remains all Rust. `cobol-core` selects the GnuCOBOL banking
kernel in `bank.cob`: a native DLL on Windows, or experimental static linkage
on ESP-IDF. Both builds keep
the same HTTP/JSON interface and current journal/checkpoint schema.

## Build and verify

This machine has 64-bit GnuCOBOL 3.2.0 in `C:/msys64/ucrt64`. From the repository root:

```powershell
python conformance/check.py --cobol
python conformance/check.py --cobol --clock
```

The second command additionally enables the `conformance-clock` Cargo feature
and runs deterministic loan contracts through the launcher clock file. Normal
builds have no clock override. Neither command accesses hardware.

`python nanacoin_rs/cobol/build.py --check` builds the DLL, copies its local
runtime dependencies, checks the fixed-width C ABI, builds the optional server,
and runs the ordinary HTTP profile. Generated artifacts and `bank-engine.json`
(source/module hashes, compiler, ABI and dependency list) are in
`.local/conformance-cobol`. The executable's `--bank-engine` flag prints `rust`
or `cobol` without opening a bank. The default build requires no COBOL runtime.

```powershell
$env:NANACOIN_COBOL_DLL = (Resolve-Path .local/conformance-cobol/ncposting.dll).Path
$env:NANACOIN_JOURNAL = "$PWD/.local/conformance-cobol/manual.journal"
$env:NANACOIN_PORT = "8088"
& .local/conformance-cobol/rust/debug/nanacoin.exe
```

Missing/unloadable modules and ABI mismatches fail startup; there is no silent
engine fallback. Windows dependencies load from the selected module directory.
The DLL must be locally built and trusted.

## Implemented slices and remaining work

### Reading the COBOL and understanding its data scope

The banking code uses named request/result fields such as
`post-source-balance`, `rescale-exponent`, `quote-cents-per-coin` and
`settle-next-interest`. Its `LINKAGE SECTION` declares operation-specific
`REDEFINES` views over the same 512-byte caller-owned ABI frame. They are
aliases, not copies or extra buffers. Bounded books use named `OCCURS` rows,
with logical one-based indices instead of raw slot/byte arithmetic. Text and
binary64 display payloads also have named views. Rust's 64-word ABI, operation
numbers and result codes are unchanged. `78` declarations name the operation,
selector, result and numeric-limit constants. When one operation invokes a
shared numeric helper, its assignments explicitly use that helper's view.

COBOL paragraphs share their program's data namespace; grouping fields helps
organization but does not make them private to a paragraph. `WORKING-STORAGE`
has program lifetime and retains values between calls. Mutable scratch data
now lives in `LOCAL-STORAGE`, grouped as arithmetic, loans, flows, selection,
quotes and display calculations. It is initialized afresh for each invocation
and released on return. `LINKAGE` fields belong to the caller. None of these
fields is declared `GLOBAL` or `EXTERNAL`. Separate subprograms would provide
independent data namespaces; this kernel still uses one program and its
existing internal `PERFORM` helpers. The Rust mutex continues to serialize
access to the GnuCOBOL runtime.

GnuCOBOL 3.2 generates a 624-byte per-call allocation for this scratch data.
The P4 target ABI probe verifies 2,643 calls with unchanged free heap after
warm-up, repetition and both boundary-vector passes. This is kernel/runtime
evidence, not a new board deployment. The attached P4 keeps the October 7
image recorded in `DEPLOY.md` until another application update is requested.

COBOL computes NC/USD posting plans, exact rescaling, exchange cents, annual
rates, exact interest/remainders, loan settlement/catch-up/payoff/credit-draw
numeric transitions, reform of loan fractions, administrator/grant/configuration/
key/last-Nana policies, and physical fulfillment transitions.
Ledger flow classification and wide epoch totals are also prepared in COBOL;
the ABI encodes wide totals as signed high/low parts in base 10^15.
Shared NC/USD posting eligibility now uses COBOL: self/amount checks, system
issuance, member existence/disabled status, correction exceptions and wallet
bounds. Correction policies also decide refund/reversal permissions, protected
transaction classes, remaining original units and bounded annotation capacity.
Their payer/payee/units are prepared before append and consumed during application.
Ledger read projections use COBOL for transaction kind/reference priority, debit
amounts and circulation, public-message exclusion, account/currency/participant
filtering, cursor validation and clamped limits, archive candidate selection,
descending ordinal order, continuation and retention flags. Rust performs archive
I/O and installs/formats the selected rows. Unsigned hostile cursor values above
the signed frame range saturate while preserving comparisons against bounded
current ordinals/pages; incarnation equality is supplied as an identifier fact.
Listing status filters and active counts now use COBOL, as does book ordering
by descending creation time then ascending id. Bounded 20-row selections merge
in COBOL across the whole book; editing a row does not change its creation order.
Outstanding-offer profile selection and disabled-viewer eligibility use COBOL;
offer note visibility already uses the COBOL private projection.
Account queries also use COBOL to select NC/USD and issuance/member balances,
issuance labels and disabled status, plus account-view eligibility. Fulfillment
book inclusion and zero-value private transaction visibility use COBOL selectors.
The typed profile-update wire guard and private-message screen eligibility use
the same COBOL policies; Rust keeps credential hashing and notification delivery.
Household statistics use COBOL for retained-year eligibility, employment counts
and percentages, repeated-sale selection and inflation averages, and filled-quote
or retained-pair exchange selection and display rates. Interest uses the exact
COBOL loan summary. Rust supplies record/identifier joins, renders the selected
metrics and delivers the existing screen HTTP/JSON payload. A loopback mock-screen
contract checks all four metrics after reopening with the other engine.
Administrative audit cursor validation, sequence filtering, candidate selection,
ordering, continuation and truncation also use COBOL. Activity projects public
event eligibility/kinds, subject/party ids and field presence there; Rust joins
records and formats only those selected fields. COBOL selects the latest retained
loan-payment sequence so a partial repayment and payoff in the same second do
not both appear as loan_paid. The Rust engine uses the same corrected policy.
Activity limits/after filtering and retained-cache truncation use COBOL, including
the reserved event-sequence gaps and upper sequence limit.
Identity/business audit classification, member/name/role/status/credential fields
and bounded-cache eviction are prepared in COBOL before journal append. Rust
installs the prepared row, with no post-append classification or member-id fixup.
Current identity audit records explicitly mark creation; profile or password
changes sharing a creation timestamp cannot produce member_joined events. Both
engines use this current schema, with no legacy checkpoint decoder added.
COBOL also validates audit sequence/actor/member bounds, creation markers and
credential-free business records. Current checkpoint-and-tail tests replace an
obsolete pre-bots compatibility fixture, following the root development policy.

COBOL now checks loan debt/due/remainder bounds, exact quote totals, NC/USD
balance conservation, epoch and wide flow bounds, correction consistency,
retained transaction metadata and history/audit sequence continuity. Rust
supplies record lookup and duplicate-identifier facts. The default Rust engine
keeps its own invariant checks. Commerce invariants now check gift totals and
targets, art identity/revision/price/asset metadata, and one equipped artwork
per owner in COBOL. Fulfillment invariants check payment parties, amount,
correction/status consistency, unique transactions and retained update actors
and sequences there. Boundary tests include minimum signed 64-bit
values so native absolute-value overflow cannot admit corrupt balances.

Private zero-value message policy, transfer posting eligibility, economic
quantity bounds, labor-to-Nana restrictions and classified thing kind/unit
consistency now use COBOL. Rust supplies Unicode memo facts and record lookups.
Bootstrap and disabled-actor eligibility, new-member name/password/capacity/duplicate
policies, and name/Mastodon structural text eligibility now use COBOL. Rust supplies
Unicode blank/control/whitespace facts and password-verifier validity, and keeps
password hashing and token handling.
Member-update permission/text/verifier checks and active-Nana protection, plus
current credential-setup/token-member eligibility, also use COBOL. Member ids,
roles/kinds, password-driven key revocation, key timestamps, household defaults
and settlement defaults are prepared there before append and installed by Rust.
COBOL also selects wire-level blank-display fallback and optional initial grants,
rejects bot Nana roles, and applies the typed member-update permission guard.

Historical amount projections and refund conversions also use COBOL to sum
subsequent epoch exponents, convert exactly and leave USD unchanged.

Correction plans compute net refunded units and full-refund links in COBOL
before durable publication; Rust installs the resulting annotation.
Offer creation/acceptance/undo/decline/withdrawal policies, settlement deadlines,
BUY/SELL/good-deed payout direction, direct offer privacy/status/reversibility,
and oldest eligible record selection are also in COBOL. Offer transitions and
recycling indices are prepared before append; application installs their result.
Listing creation/edit/cancel/purchase policies, classified thing reuse/standard
flags, first eligible listing/oldest eligible thing selection, offer pinning,
full-refund offer-link selection and NOT_SELECTED offer views are now in COBOL.
Their record changes and recycling indices are also prepared before append.
Fulfillment creation/exclusion, work/goods/cash classification, capacity,
completed/reversed row retention and full-correction obligation links now use
COBOL plans. Rust applies the selected row and status without a post-append call.
Quote status/liveness and price/id book ordering, exact quote totals, member/book
limits, posting/expiry/cancellation policies, payout parties and recycled row
selection now use COBOL. Exchange legs and quote status changes are prepared
together before append; application uses the prepared parties and amounts.
Gift request creation/closure/contribution policies, capacity, deadlines, target
validation, net contribution totals and refund subtraction now use COBOL.
Contribution totals and refund results are prepared before append. Funds errors
retain precedence over aggregate overflow, and refunds after currency reform
subtract the current-epoch cash amount from the request's current-epoch total.
Digital art mint/list/buy/gift/equip policies, stale owner/revision/price checks,
hash and HTTPS locator byte validation, ownership/price/equipment/revision plans
and selection of editions to unequip now use COBOL. Raw bounded metadata bytes
travel in the fixed frame; the module retains no pointers. Rust supplies Unicode
blank-text facts and installs prepared art fields and payment legs after append.
Loan terms, requests/responses, lender/book limits, oldest terminal recycling,
acceptance/closure policies, visibility and scheduler-readiness predicates now
use COBOL. Loan start/status/draw plans are prepared before append; acceptance
preserves funds-error precedence over deadline overflow. Rust supplies scheduler
ticks and installs the plans, while exact repayment/credit-draw arithmetic uses
the existing COBOL settlement operation.
Loan waiting reasons, profile inclusion and note-redaction predicates, projected
overdue values, and whole-book principal/debt/count/weighted-rate summaries now
use COBOL. Rate weights use exact decimal numerators with the common rate-days
denominator 15330. Only the final display percentage becomes IEEE binary64;
money totals remain integer values. Rust formats the returned totals as JSON.

Lotto creation/purchase/draw policies, ticket and pool bounds, exact fixed-month
interest, escrow movements, winner mapping across all 32 members, house-first
interest and issuance shortfalls now use COBOL. The complete settlement cursor,
cash leg and lotto fields are prepared before append. Application installs the
plan without recomputing a draw or calling the module. Book recycling and public
ticket totals, due dates, status and scheduler readiness also use COBOL. Rust
supplies row identity/member-presence facts to COBOL's lotto invariants, which
check ticket/pool arithmetic, winner/cursor consistency and global escrow totals.
Rust supplies the random ticket, clock and Unicode title facts. Both builds pay
principal through member 32, followed by house interest, issuance and completion
(steps 33, 34 and 35); the old 16-member cursor could strand high-member escrow.

Currency reform stale-request, parameter and epoch-capacity policies now use
COBOL. Field identities select their money limits and decimal exponents inside
the module, including the separate quote-price exponent. COBOL checks that
rescaling a lotto preserves its originally rounded interest obligation. Preview
circulation and the new epoch, exponent and NC scale are computed there too.
All resulting values are prepared before append; Rust traverses the records and
installs them, without computing a new epoch or scale after publication.

Rust prepares bounded cash legs before durable append, then installs the
COBOL-computed balances without another ABI call. Split repayments and exchanges
prepare both legs together. Loan settlements and whole-economy reform values
are cached in transient plans before append. Plans are never serialized or
committed independently. Current-schema replay prepares the same transitions.
The completeness audit also moved loan posting emission into a COBOL plan:
zero-leg suppression, funding versus repayment parties, principal/interest
classification, leg order and the second transaction's reserved offset are
prepared before append. Rust consumes those descriptors without classifying
payments after publication. Member grants likewise use the prepared member id
for both balance preparation and the installed posting. Refund text, provisioning
eligibility and configuration-read permission now use COBOL guards.
The wire adapter also delegates exact decimal quantity scaling and bounds to
COBOL. Unicode/ASCII lexical parsing stays in Rust. For household inflation,
COBOL selects the latest two eligible retained sales before calculating their
unit-price change; Rust supplies record indices and formats the result.

Identity query opcode 9 now owns the API adapter's provisioning defaults (with
existing text-policy guards), delegated bot-key view/read-key mutation rules,
key active/creation-time projections and matched-key member eligibility, plus
wire and fresh-command currency-epoch rejection. Key presence and comparison
facts enter COBOL; credentials, hashes and tokens stay in Rust. Startup requires
the new opcode's capability bit and rejects older partial modules.

Member creation now consumes the prepared ID, role, kind and disabled fields,
including token members. The active-Nana count used by demotion/disabling
policies is folded by opcode 9 from bounded role/status pairs, including members
31 and 32 in the second chunk.

The desktop banking ownership audit is complete: all 42 commands and eight
commerce actions are accounted for, with no remaining banking projection or
publication fallback. `inventory.py` checks that every command
and commerce action remains accounted for. The identity family has been traced
through validation, preparation, publication and reads as described below.
The posting, forex, marketplace, commerce, lending, lotto and reform families
have also been traced through preparation, publication and reads below. Firmware
linking and resource budgets remain an independent open gate.
The posting family now also consumes COBOL's cash-leg, same-wallet balance and
credit-source mask plans. Publication uses the prepared cash availability and
installs the selected mask; it performs no new loan-source classification.
Object lookup/storage, text and identifier joins, HTTP, cryptography,
clock/random input, scheduling and durable I/O stay Rust. Those framework
responsibilities do not themselves imply an unported banking decision.

### Identity ownership audit

The following table records banking ownership in the optional build. Rust's
`State::validate_at` dispatches policies before `Service::commit` prepares the
event; `State::prepare_bank` then caches the identity and audit plans before the
durable append. `State::apply` installs the selected fields and consumes those
plans. Replay follows the same preparation/publication path. Direct string and
identifier lookup, verifier/hash facts, field copying and JSON enum formatting
are adapter responsibilities.

| Commands | Policy and preparation | Publication |
| --- | --- | --- |
| Provision | 44 bootstrap, 48 new-member, 49 text/provision/defaults, 29 identity, 57 audit | Prepared ID/role/kind/disabled; supplied household/name/verifier |
| CreateMember, CreateBot | 10 admin, 48 capacity/duplicate/text/verifier, 49 defaults/bot-role, 13 grant, 29 identity, 39/1 posting, 30/31 flows, 57 audit | Prepared identity and grant eligibility/recipient; prepared balances and flows |
| UpdateMember | 17 update eligibility/text/last-Nana with 9 active-Nana count, 29 transition, 57 audit | Prepared role/disabled/key revocation; supplied optional profile/verifier fields |
| Configure | 10 admin, 14 limits, 49 text, 29 settlement/default, 57 audit | Prepared settlement interval; supplied validated settings |
| SetApiKey, SetReadKey | 12 key policy, 9 wire delegation/scope, 29 key kind/time, 57 audit | Prepared full/read slot and creation time; supplied hash |
| MigrateMember | 23 current-record credential setup, 29 household default, 57 audit | Prepared household-default choice; supplied username/verifier and token clearing |
| AddMember | 23 token-member eligibility/capacity/duplicate, 29 identity, 57 audit | Prepared ID/role/kind/disabled; supplied name/token hash |

Key presence/time and matched-key member eligibility use opcode 9; sessions,
constant-time comparisons and credential issuance remain Rust. Member profile
views copy already installed bank fields. The interface contracts cover grants,
defaults, bot roles/keys, permissions, duplicate/capacity limits, last-Nana
protection across the 32-member boundary, credential revocation, retries and
cross-build restart. Current-record token-member/credential setup also remains
covered by the shared Rust tests; it is not an old storage-format decoder.
This ownership audit certifies the identity family's banking implementation,
not firmware deployment or completion of other families.

### Posting ownership audit

| Commands | Policy and preparation | Publication |
| --- | --- | --- |
| Issue, Retire | 10 admin, 39 account policy, 1 balance/cash/credit-mask plan, 30/31 flows, 57 audit | Supplied validated parties/amount/memo; prepared balances, flows and credit mask |
| Transfer, ClassifiedTransfer | 43 message/payment/economic/party policy, 49 decimal quantity, 39/1 posting, 30/31 flows, 46 fulfillment, 57 audit | Supplied metadata; prepared cash availability, balances, flows, credit mask and obligation |
| Reverse, Refund | 41 permission/capacity/original-unit/overdraft policy, 42 epoch conversion, 1 posting, 40 correction, 25 offer links, 46 fulfillment links, 56 gift totals, 30/31 flows, 57 audit | Prepared parties/units/cash, correction annotations, links and net gift total |

`prepare_leg` selects publication mode in opcode 1. COBOL omits zero cash legs,
preserves same-wallet balances, and updates the 32-member credit-source mask.
The mask treats positive NC loan-reference rows as restricted credit; other
positive NC rows clear the participants' flags. USD and private-message rows
preserve them. Rust supplies party IDs, currency/reference facts and the staged
balances/mask, then installs the outputs. Multi-leg events stage the preceding
leg's outputs for the next leg before journal append. `ledger_post` and
`apply_balances` consume this prepared plan; default builds retain their Rust
rules. Ordinary record ordinals, bounded cache storage and archive I/O stay in
the framework.

The contracts check issuance/retirement, zero/private messages, overdrafts,
partial/full corrections, exact/inexact historical conversion, wallet ceilings,
retry/recovery and balanced postings. The member-32 credit-source contract checks
that USD/messages cannot unblock loan-funded payments and that ordinary NC can,
including reopening in the other engine. Direct ABI cases cover same-wallet,
zero-leg and all 32 mask bits. This certifies the posting family's banking
ownership; the lending/lotto audit is recorded below.

### Forex ownership audit

| Commands or reads | COBOL ownership | Rust publication or rendering |
| --- | --- | --- |
| IssueUsd | 10 administrator, 39 account policy, 1 cash/balance plan, 30/31 flows, 57 audit | Supplied amount/memo; prepared USD balances and flows |
| PostQuote | 51 clock, exact product/divisibility, expiry, per-member and book limits; 50 liveness; 24 oldest eligible recycling | Prepared status/recycle index; supplied quote fields and event identifiers |
| TakeQuote | 51 existence/self-deal/liveness, BID/ASK parties and total; 39 eligibility for both wallets; 1 staged NC/USD legs, 30/31 flows | Prepared parties, status and cash amount; both cached wallet legs, transaction references and quote installed from one event |
| CancelQuote | 51 maker/admin permission and open-status transition | Prepared status and supplied event time |
| Quote book and detail | 50 live/expired/stored status, 3 exact cash amount, 52 ASK/BID price and ID ordering | Selected records, member-name joins, enum/identifier JSON formatting |

`validate_forex` invokes the optional engine's policies; `prepare_bank` caches
the quote transition and both exchange legs before append. `apply_forex` consumes
those plans. Transaction IDs and their disjoint reference range are journal
record conventions in Rust; they do not choose prices or financial outcomes.
Replays prepare the same plans against the current-schema state before applying
the durable event. Optional query paths use COBOL for amounts, status and order.

Contracts cover BID and ASK, exact/inexact and maximum products, failed second
legs, self-dealing, maker/admin cancellation, filled/closed refusal, two-per-member
and sixteen-slot limits, oldest closed-row recycling, deadline equality and
cross-build recovery. The additional cash-leg contract proves that a refused
trade changes neither wallet nor quote, that its key can succeed after funding,
and that the complete two-transaction receipt survives reopening in the other
engine without another payment. Fixed-clock differential traces compare that
entire scenario as well as the baseline exchange. This certifies all four forex
commands and their quote projections; the overall completeness audit stays open.

### Marketplace ownership audit

| Commands or reads | COBOL ownership | Rust publication or rendering |
| --- | --- | --- |
| List, ClassifiedList | 26 title/price/type/good-deed/labor/capacity and thing-reuse policy, 28 dependency protection, 27 first recyclable listing, 24 oldest recyclable thing | Prepared status, selected thing/recycle indices and standard flag; supplied text and economic fields |
| UpdateListing, Cancel, Buy | 26 owner/admin/open policy, participant-role selection and BUY/SELL parties; 39/1 payment; 30/31 flows | Validated optional edits, prepared status/parties and cached wallet/flow plan |
| MakeOffer, AcceptOffer, UnacceptOffer, DeclineOffer, WithdrawOffer | 22 permission/status/clock/bot/role/parties/deadline/listing-mutation plan, 21 liveness/pins/recycling, 24 oldest eligible offer, 25 reversal links, 39/1 payment and correction | Prepared phase, settlement parties/deadline, listing mutation and selected reversal links; cached posting plan |
| SetFulfillment and payment tracking | 20 permissions/status/reason, 45 capacity, 46 creation/type/reversal/status, 47 recycling | Prepared status, kind and selected row; supplied update metadata and payment record |
| Listings, offers and fulfillment books | 62 listing filters/count/order, newest-first offer order, profile selection and fulfillment participants; 21 offer status/visibility/reversibility | Selected rows, member/text joins and enum/identifier formatting |

`State::prepare_bank` caches the validated marketplace transitions, posting,
flow and fulfillment plans before append. Publication consumes them; no Rust
backend handles an unported financial operation. Prospective payee-role policy
uses both participant facts in COBOL. All offer phase publications decode the
prepared phase. Full corrections use the prepared link selection. Initial and
reversed fulfillment statuses come from opcode 46.

Offer retry responses reconstruct the durable operation's immutable phase and
timestamp before COBOL projects its status. That receipt formatting does not
reapply a payment or change a live offer. Live fulfillment is read separately;
the receipt deliberately omits it. Rust retains record copying, event IDs,
bounded storage, text facts/joins, wire routing and serialization.

Contracts cover both payment directions, wrong participants, disabled accounts,
underfunding, private/profile projections, bot good deeds, labor role restrictions,
settlement deadlines, undo/correction overdrafts, no double refund, competing
acceptances, pinned capacity, thing reuse, recycling and cross-build recovery.
The new full-book contract checks 32 offers and 32 loans, including the final
two-row selection chunk and recycling, and 16 lottos. Direct vectors test every
position in the 30-row scan to catch input/output overlap. Marketplace banking
ownership is certified; lending and lotto ownership is recorded below.

### Commerce ownership audit

| Actions or reads | COBOL ownership | Rust publication or rendering |
| --- | --- | --- |
| CreateRequest, CloseRequest | 55 title/target/deadline/capacity/owner/closed policy and initial owner/net total | Prepared owner/received/closed; supplied text, target and deadline |
| Contribute | 55 existence/deadline/closed/net-total plan, 39 account policy, 1 cash/balances, 30/31 gift flows | Prepared recipient and net total, cached wallet/flow plan and transaction metadata |
| MintArt | 59 hash/HTTPS locator bytes and text facts; 58 capacity/owner/price/equipment/revision defaults | Prepared state plus supplied immutable creator, title, permission text, hash and locator |
| ListArt, BuyArt, GiftArt | 58 permission, recipient, expected-owner/revision/price and ownership/price/equipment/revision transitions; 39/1 payment and 30/31 flows for purchase | Prepared art fields, seller and amount; money and ownership installed from the same durable event |
| EquipArt | 58 equipment policy; 60 selection mask across both 32-row chunks | Prepared equipment fields and selected rows to clear |
| Gift corrections and currency reform | 41/42 correction policy and historical/current amounts, 56 net refund, 38 exact reform fields | Prepared net total and converted monetary fields |
| Commerce book | Published gift/art state is already the prepared result; no filtered or derived monetary projection in this endpoint | Serialize the current records; no bank engine fallback or recomputation |

`validate_commerce` calls gift/art policies. `prepare_bank` caches those plans,
payment legs, flow totals and any gift refund before journal append;
`apply_commerce` and `commerce_refund` install them. Artwork owner/price/revision
updates share the journal event with payment. COBOL also validates the current
gift/art tables, amounts, assets, ownership and equipment in opcode 61.
Rust retains Unicode facts, identifiers, record lookup/storage and JSON parsing.

Contracts cover all eight actions, stale forms, wrong owners, disabled targets,
invalid asset metadata, funding refusal, concurrent buyers, retry receipts,
equipment replacement at artwork 64, capacity, deadline equality, cumulative
refunds, reform and cross-build recovery. Gift and art differential scenarios
compare complete banking responses. This audit covers the commerce command and
its public book. Lending and lotto ownership is recorded below.

### Lending, lotto and shared arithmetic ownership audit

| Commands or reads | COBOL ownership | Rust publication or rendering |
| --- | --- | --- |
| RequestLoan, RespondLoan, OfferLoan, AcceptLoan, CloseLoan | 15 terms/permissions/status/limits/clock; 16 transition/default fields; 24 terminal-row recycling; 18 activation eligibility | Prepared loan fields and recycle index, supplied text and event identifiers |
| RepayLoan, RunLoan | 6 participant eligibility, clock, credit activation, interest-first payment, due installments and payoff; 5 exact interest/remainder/denominator; 8 aggregate validation and posting descriptors; 1 staged balances/credit mask; 30/31 flows | Cached settlement, posting and flow plans prepared before append, then installed without recalculating payment rules |
| Loan detail, books and statistics | 17 privacy/status/terminal/readiness/wait reason; 19 exact summaries; 62 newest-first ordering | Selected rows, member-name joins and enum/JSON formatting |
| CreateLotto, BuyTickets, RunLotto | 33 terms/permissions/readiness/escrow/step transitions; 32 ticket-to-winner selection; 34 cash legs; 1 staged balances; 30/31 flows | Supplied random ticket input, durable selected winner and prepared settlement step, cash and flows |
| Lotto detail and books | 33 ticket totals/due/status/readiness; 62 newest-first selection | Selected rows and member/text joins |
| ReformCurrency and money helpers | 2 exact rescaling and currency scale; 9 loan fraction reform; 35 whole-economy consistency; 42 historical epoch conversion; 49 exact wire decimal parsing | Prepared numeric record values and epoch installed atomically; Unicode lexical facts, record lookup and serialization remain Rust |

Rust supplies record existence, balances, roles and disabled-account facts; it
schedules ticks and supplies time and randomness. COBOL decides their banking
meaning. Both disabled settlement participants are checked there. The aggregate
repayment check and each payment leg are computed before append; publication
consumes the complete prepared plan. Shared currency scale and loan denominator
helpers also dispatch to COBOL in the optional build.

Contracts cover terms and lender/book limits, proposals without money movement,
privacy and permissions, funding waits, disabled lenders, failed-key recovery,
interest/remainder/payoff, installment catch-up, member-32 ticket settlement,
savings/delayed/empty draws, settlement restart and compound currency reform.
The fixed-clock differential traces compare full response transcripts through
cross-build reopening. Direct ABI vectors additionally check participant/clock
precedence, aggregate overflow and exact interest denominators.

## Public C boundary

```c
int32_t nc_bank_abi(void);                 /* 7 */
int32_t nc_bank_slots(void);               /* 64 */
uint64_t nc_bank_capabilities(void);        /* 0xFFFFFFFFFFFFFFFE */
int32_t nc_bank_v7(int32_t operation, int64_t slots[64]);
```

Slots are native fixed-width signed integers; COBOL uses packed decimal for
wide intermediates. Art metadata additionally packs bounded raw hash/URL bytes
into words; its documented byte offsets are independent of Rust struct layouts.
Loan summaries return the final display percentage as native IEEE binary64 bits
in one fixed-width slot; no Rust float struct layout crosses the boundary.
Results: 0 success, 1 invalid input, 2 overflow, 3 insufficient
funds, 4 conflict, 5 forbidden, 6 disabled, 7 not found, 8 capacity, 9 unavailable.
Offer-specific results are 11 listing closed, 12 self-deal, 13 bot good-deed
restriction, 14 offer closed and 15 offer settled; Rust maps them to the same
JSON errors as the default engine.
Result 16 is the per-member quote limit.
Result 17 is the per-lender loan offer limit.
Result 18 is a stale cursor/request/epoch and maps to the existing `stale_request` JSON error.
Result 19 is a corrupt bank/ledger/audit record and maps to `CorruptJournal`.
Operation-specific inputs/outputs are documented beside each COBOL paragraph.
Startup verifies the operation capability mask as well as ABI/frame size, so
an older partial module cannot satisfy a newer server's engine requirements.
ABI 7 includes the exact interest denominator, raw settlement participant
eligibility, aggregate loan posting validation, book selection and marketplace plans. Its entry point
is `nc_bank_v7`; a server expecting it rejects older ABIs before opening the journal.
`test_startup.py` also checks incompatible frame sizes and missing capabilities
with isolated metadata-only DLL fixtures. These are engine integration checks,
separate from the HTTP contract count.
The module retains no caller pointers. Rust serializes runtime access and keeps
the library loaded for the process lifetime; the shim initializes libcob once.

ABI vectors check returned values as well as status, mixed calls, integer
boundaries, exact/inexact scaling, fractions, policies and transitions. Two
passes are normal; longer runtime repetition is explicit:

```powershell
python nanacoin_rs/cobol/test_abi.py .local/conformance-cobol/ncposting.dll --repeat 100
```

The original 18-vector/100-pass check made 1,800 calls, not 18,000. Source size
is simply `(Get-Content nanacoin_rs/cobol/bank.cob).Count`; no generated C counts.

## Firmware feasibility evidence

```powershell
python nanacoin_rs/cobol/probe_firmware.py
```

This compile-only probe generates C and tries installed ESP32, S2, S3 and
RISC-V cross compilers. Logs and a source-hashed `report.json` go to
`.local/cobol-firmware-probe`. The current source compiles for all four targets
with target GMP headers. The original missing `gmp.h` result was resolved by
cross-building GMP; it was not evidence that the runtime could not fit.

The runtime investigation now has reproducible tools. Unpack
[GNU GMP 6.3.0](https://gmplib.org/) into
`.local/cobol-firmware-source/gmp-6.3.0` and use the GnuCOBOL 3.2 source in
Downloads. MSYS `make` and `m4` are required. For S3:

```powershell
python nanacoin_rs/cobol/build_target_runtime.py --target esp32s3 --compiler C:/Espressif/tools/xtensa-esp-elf/esp-14.2.0_20251107/xtensa-esp-elf/bin/xtensa-esp32s3-elf-gcc.exe --gmp-source .local/cobol-firmware-source/gmp-6.3.0 --gnucobol-source C:/Users/matth/Downloads/gnucobol-3.2_win
python nanacoin_rs/cobol/probe_idf.py --target esp32s3 --gnucobol-source C:/Users/matth/Downloads/gnucobol-3.2_win
python nanacoin_rs/cobol/probe_qemu.py --target esp32s3
```

Use `esp32s2` and its compiler for S2; use `esp32p4` and
`riscv32-esp-elf-gcc.exe` for P4. The runtime builder isolates each architecture,
uses generic static GMP, and disables unused curses, database, XML/JSON and NLS
features. Its explicit static-loader adapter refuses dynamic module loading;
GnuCOBOL's `--without-dl` alone selects libltdl rather than removing that
requirement. Xtensa needs `-mlongcalls`; P4 needs ESP-IDF's `ilp32f` ABI.
Compiler flag changes clean the prior objects before rebuilding. User source
archives in Downloads are never modified.

`probe_idf.py` uses the installed ESP-IDF 5.5.3 and `esp` Rust toolchain to build
an isolated image containing Rust -> fixed-width C linkage -> the current
COBOL bank, static libcob and GMP. Real ESP-IDF VFS, timer and allocation services
are linked. The probe profile explicitly returns ENOSYS for process signals
and login lookup; neither is a banking service. It generates an ELF, a map,
source/image hashes and JSON size data. It never runs a flash/deploy command.
This minimal caller is separate from the complete NanaCoin firmware integration
described next.

To run the complete boundary workload on target code, export the public C ABI
fixtures and include them in the image. These are additional integration checks,
not new HTTP contracts:

```powershell
python nanacoin_rs/cobol/test_abi.py .local/conformance-cobol/ncposting.dll --export-vectors .local/cobol-firmware-probe/abi-vectors.json
python nanacoin_rs/cobol/probe_idf.py --target esp32s3 --gnucobol-source C:/Users/matth/Downloads/gnucobol-3.2_win --abi-vectors .local/cobol-firmware-probe/abi-vectors.json
python nanacoin_rs/cobol/probe_qemu.py --target esp32s3
```

The S3 image runs in Espressif QEMU on Windows. P4 runs in the checksum-verified
[Espressif emulator](https://github.com/espressif/esp-emulator) v0.46.0 Linux
release through Windows WSL. The HTTP conformance suite remains native Windows.
Both pass all 1,128 ABI vectors twice plus 387 mixed Rust interop calls, for
2,643 calls each. Heap remains unchanged after warm-up through the two complete
boundary passes and subsequent mixed-call repetition.

| Target | Minimal Rust/static COBOL IDF link | Target runtime execution | Warm retained heap | Task stack used out of 16,384 bytes |
| --- | --- | --- | --- | --- |
| ESP32 | Passed | Not measured | Not measured | Not measured |
| S2 | Passed, including boundary-fixture image | Installed emulators do not support S2 | Not measured | Not measured |
| S3 | Passed | QEMU and Espressif emulator passed | 50,052 bytes | 9,344 bytes |
| P4 | Passed | Espressif emulator passed | 50,016 bytes | 9,368 bytes |

Both measured workloads make 216 allocation requests totaling 52,361 requested
bytes. That cumulative traffic is not peak live memory. Boundary-fixture app
images are 687,136 bytes for S2, 719,728 for S3 and 805,936 for P4, including IDF,
the banking runtime, Rust caller, instrumentation and the fixture tables.
These are observations for this workload, not guarantees for every framework
call path or the complete server.

For P4, the installed IDF linker SRAM region extends beyond the size tool's chip
memory map. The probe retains the warning, actual image size and raw allocated
ELF section sizes; it does not advertise the unmapped region as free RAM. This
needs reconciliation before full-bank budgeting. The installed QEMU profiles
support ESP32/S3, not S2/P4; `probe_qemu.py` records unsupported profiles rather
than treating them as successful runtime checks.

For the additional Espressif emulator, download the release into the local probe
directory and verify its published `SHA256SUMS`. With the v0.46.0 release here:

```powershell
python nanacoin_rs/cobol/probe_esp_emu.py --target esp32p4 --emulator .local/cobol-firmware-probe/esp-emulator/esp-emu-0.46.0-x86_64-unknown-linux-gnu/esp-emu
```

Reports distinguish a successful link, emulator execution, full-bank integration
and deployable support. Full server budgets, S2 runtime execution and the P4 SRAM
mapping remain open. Static firmware linkage is experimental until those gates
pass; a successful build does not authorize deployment. No investigation tool accesses, flashes or
erases a physical board.

## Actual Rust server with static COBOL

The same optional `cobol-core` feature now supports ESP-IDF static linkage. It
requires an explicit target module directory; default Rust builds neither load
nor link the COBOL runtime. The static module exports the same ABI 7 entry and
metadata as the Windows DLL. Rust keeps one mutex around every call and runtime
initialization, and verifies ABI/frame/capabilities before journal startup.
The ESP-IDF adapter refuses unsupported process signals, login lookup and fork;
real filesystem, time, allocation and threads remain ESP-IDF/Rust services.

Build a target module from the existing cross-built runtime, then compile the
actual server. These commands build and measure artifacts only:

```powershell
python nanacoin_rs/cobol/build_target_bank.py --target esp32s3 --compiler C:/Espressif/tools/xtensa-esp-elf/esp-14.2.0_20251107/xtensa-esp-elf/bin/xtensa-esp32s3-elf-gcc.exe --gnucobol-source C:/Users/matth/Downloads/gnucobol-3.2_win
python nanacoin_rs/cobol/build_firmware.py --board s3 --engine cobol --target-dir C:/nc-cob-s3 --static-module .local/cobol-target-esp32s3
python nanacoin_rs/cobol/build_firmware.py --board s3 --engine rust --target-dir C:/nc-rust-s3
```

The offline build tool uses the existing board registry, sdkconfig, partitions,
web assets and board-marker/image-size gate. It verifies target/module hashes
and records linked ELF/image hashes and section sizes. It has no deploy or flash
operation. Separate Cargo directories preserve the ordinary Rust artifacts.
The independent default Rust S3 build passes the same image gate at 3,775,984
bytes and does not link the COBOL archives.
The S3 COBOL server currently links and fits its 4,194,304-byte app partition:
its complete image is 4,067,520 bytes in the latest synthetic-network build.
This includes the web bundle and Rust
HTTPS/framework code, not just the isolated banking probe. There is only
126,784 bytes of remaining app-partition space in this measured build.

The P4 complete server also links and fits: 4,010,144 bytes in its 8,388,608-byte
app partition. The S2 server links, but its 2,717,632-byte image exceeds the
existing 2,490,368-byte partition by 227,264 bytes; the image gate correctly
rejects it. S2 COBOL firmware is currently unsupported in that layout. Further small-board size reduction is deferred at the owner's request; the
regular Rust partition layout is unchanged. Firmware workload heap/stack measurements remain
separate from the kernel measurements above; no full-server runtime or
deployment support is inferred from linking alone.

For an isolated full-server emulator startup investigation, add
`--emulator-network` to the firmware build. That embeds synthetic credentials
and writes a separate report under `.local/firmware-emulated-<engine>-<board>`.
`probe_server_emu.py --build-report <report.json> --emulator <esp-emu>` checks
artifact hashes, boots a fresh local S3 flash image with 8 MB emulated PSRAM,
restricts its network and collects public status/diagnostics over a loopback
forward. Startup and bounded banking/TLS measurements are available; they do
not certify every record capacity or hardware failure mode. It never persists emulator flash or invokes a physical board runner.
The initial full S3 server with a 32 KiB serving stack passed startup and
answered public JSON status/diagnostics, including HTTPS. Its first snapshot
reported 137,799 bytes free internal heap,
5,781,220 bytes free PSRAM and 13 tasks. These are emulator startup observations,
not peak workload budgets. The probe exposed a 70 KiB state-construction stack
frame against the 64 KiB main-task stack. Startup now initializes the bank state
directly in its heap allocation from the same defaults, boxes the reusable COBOL
publication cache, and warms libcob before replay. It reaches the listeners
without that startup corruption; both engines' 225 tests and strict Clippy
checks pass. A subsequent banking write exceeded the 32 KiB serving stack.
The optional COBOL build requests a 64 KiB serving stack. The S3 cannot
allocate another contiguous block of that size after startup, so it serves on
its existing 64 KiB main-task stack. P4 retains a separate serving task. This
configuration passes HTTPS identity, issuance, payments, insufficient-funds
rejection and concurrent idempotent retries, plus representative operations in
all eight business families without the earlier stack corruption. One transient
transport failure retried the same key; the four receipts matched and only one
payment occurred. The current emulator-clock image is 4,068,208 bytes (126,096
bytes app-partition headroom). The default Rust
task and stack configuration stays unchanged.
The startup probe's optional `--workload --https` mode reuses the conformance
suite's public JSON client for provisioning, PKCE login, account issuance,
payments, insufficient-funds rejection and four concurrent retries of one key.
It verifies the final balances and conservation through public responses and
collects a post-workload diagnostic snapshot. Transport failures retry the same
idempotency key, and the report counts these separately from banking receipts.
`--families` requires an isolated `build_firmware.py --emulator-network
--emulator-clock 1799996400` image, which opts into the separate `emulator-clock`
Cargo feature. It initializes the system clock before service startup so loan
and lottery contracts do not need external SNTP. Normal firmware omits the
feature and has no override. An unclocked restricted-network probe correctly
rejects time-dependent loans as unavailable.
`--families` adds representative marketplace, loan funding/payoff, two-wallet
forex, gifts, art, ticket escrow and reform operations through the HTTP listener
on the same bank. These probes do not increase the Windows contract count.
The probe records the exact Linux emulator PID and stops it before closing the
WSL wrapper, avoiding leftover listeners between runs. TLS uses the actual board CA;
hostname checking is disabled only for the loopback IP forwarding. This bounded
workload is additional firmware evidence, not a replacement for the full
Windows contract suite or full-capacity resource measurements. The successful
bounded run processed 57 banking/probe requests before extra diagnostic polling. The final observed
minimum was 82,103 bytes free internal RAM and 5,067,836 bytes free PSRAM, with
15 tasks. After all family operations, the serving-stack high-water mark leaves
21,660 bytes free out of 65,536; TLS leaves 21,640 bytes free. The final probe
passes, captures the post-workload samples and cleans up its exact Linux PIDs. These measurements
include the real Rust framework and statically linked libcob/GMP bank.
`--observe-seconds 70` additionally waits for post-workload health samples to
measure the serving stack high-water mark after all business-family operations.
`--capacity` (with `--families --workload`) reuses `conformance/capacity.py`
to fill all bounded business books simultaneously, verify overflow rejections,
and reform the full bank. This scenario also runs as the tenth clock-enabled
Windows HTTP contract and compares all books after cross-build reopening.
`--history` additionally crosses the advertised retained-transaction capacity
with balanced round trips and generation-aware retry keys; it extends the bounded
emulator allowance to two hours. The Windows combined-capacity contract also
checks full retention and current-schema checkpoint/tail recovery in both engine
directions. No private storage decoder or direct state access is used.

The full S3 capacity probe now passes: 32 members, 16 active quotes, 32 gift
requests, 64 artworks, 32 loan applications, 16 live lotto pools, 48 active
listings, 32 pending offers and 128 fulfillment obligations are held together.
A reform with all these books full succeeds, followed by 3,008 balanced payments
that cross the 3,000-row retained-history limit and three journal generations.
The final sample has 79,939 bytes minimum free internal RAM, 5,166,748 bytes
minimum free PSRAM, 21,660 bytes unused main stack, and 15 tasks. It uses HTTP
for capacity traffic; the separate retained representative report verifies
HTTPS and concurrent TLS retries. This run processed 3,594 requests and took
655.59 wall seconds in emulation. Expected capacity/stale-key rejections account
for its 13 reported request errors. The resource probe completes successfully
and stops its exact emulator PID. The later October 7 P4 hardware deployment
verifies revision-1.3 startup and TLS; full-capacity P4 financial workloads
remain unverified.

Size experiments can use `build_target_runtime.py --lto --output-root <dir>`
and `build_target_bank.py --lto --runtime-root <dir> --output-dir <dir>` to keep
experimental archives separate from the verified baseline. Module construction
rejects inconsistent runtime/module LTO modes. An optimization result must pass
the same target ABI vectors before it becomes a supported runtime. The initial
S2 LTO archives compile, but ESP-IDF 5.5.3 explicitly disables GCC LTO at its
final link. The firmware builder rejects these experimental archives until that
integration is resolved; there is no demonstrated flash saving yet.

The owner has accepted using a larger board and excluded further small-board
fit work. P4 is the firmware candidate. The current P4 v1 profile's complete
COBOL image links and fits at 4,009,648 bytes in an 8 MiB app partition. Its
banking kernel already passes all 2,643 target calls in P4 emulation. Full
P4 network workload verification is separate: the P4 uses an emulated C6
ESP-Hosted companion, built from the matching managed component's slave example
by `build_hosted_emulator.py`. No physical companion is flashed.

ESP-EMU models P4 v3 silicon; its [Hosted guide](https://github.com/espressif/esp-emulator/blob/v0.46.0/docs/guides/HOSTED.md)
describes the matching ROM/stack layout. `build_firmware.py
--emulator-network --p4-emulator-rev3` selects an isolated v3.1 SDK configuration;
the real board's v1 profile remains unchanged. Use a separate target directory
such as `C:/nc-emu-p4`. The current v3.1 emulator-clock artifact is 4,011,184 bytes. Its actual allocated HP SRAM sections
total 111,108 bytes and all end within the physical SRAM address range from
ESP-IDF's `soc.h`; this validates static placement, not dynamic heap budgets.
The probe requires its matching C6 report via
`--hosted-companion-report`. The initial pair establishes the SDIO transport,
but does not finish Wi-Fi startup or reach an API listener during a further
five-minute investigation, so it does not yet validate the bank's network
workload. Both owned emulator processes are stopped and the limitation is
recorded separately from banking parity. A revision-specific emulator result cannot certify the owner's
different physical revision.

## Phased plan for two deployable builds

The end state has **two maintained build options**: the default all-Rust bank,
and `cobol-core`, where Rust hosts a COBOL banking engine. Both serve the same
JSON API, use the same current durable event/checkpoint model, and run the same
conformance profile. Selection is at build time, never a per-request switch.
The existing Rust implementation stays usable through every phase. Intermediate
COBOL builds are explicitly partial ports until all banking areas below pass.

### Architecture and commit boundary

Rust continues to own the framework, HTTP/HTTPS, JSON parsing/serialization,
cryptography, sessions, platform clock/randomness, journal/checkpoint/archive
I/O, process/board startup and delivery adapters. COBOL ultimately owns bank
policies, accounting, business state transitions, business read projections
and invariant checks. Session verification stays Rust; roles, eligibility,
visibility and permissions on bank operations move into the selected engine.

Introduce one internal banking-engine interface with Rust and COBOL backends.
Keep the network API unchanged. Do not expose Rust struct layouts, heapless
containers, enums, pointers to owned strings, or Rust `i128` directly to COBOL.
Use bounded C records with explicit tags, lengths, capacities and native
fixed-width integers. Define an exact representation for wide decimal totals
and remainders; size intermediates from the existing operation limits, and
verify them with public economic outcomes. Strings remain bounded UTF-8 bytes;
transport validation handles encoding, while banking validation handles allowed
values. Buffers have one owner and pointers never survive a call.

Use a **prepare → durable append → apply** protocol. Preparation returns a
complete validated event/change plan without modifying live balances. Rust
publishes the existing durable event. The selected engine then applies the
prepared changes through an infallible, allocation-free-after-prepare path.
Replay uses the same engine transitions, with server-owned timestamps and
persisted random outcomes. Rust owns retry-key lookup and canonical command
fingerprints; business receipts and grouped posting semantics must be identical
between backends. The complete plan reserves capacities before publication.
There must be no second COBOL journal or independently committed account store.
An ambiguous write keeps the existing fail-closed recovery behavior.

Current status: desktop banking phases 1–6 and the shared Windows conformance
matrix are implemented. Phase 0 has current-schema parity, artifact identity,
complete ownership inventory, all-target static runtime links and S3/P4 emulator
evidence. Its full firmware resource gate remains open as detailed above.
Phase 7 packaging and board deployment are outside this implementation.

### Phase 0 — deployment feasibility and parity harness

Keep the current Windows DLL prototype and run the same HTTP suite on both
builds, including opening each other's fresh current-schema fixtures. Add
artifact metadata that identifies the selected banking engine without changing
client routes. Inventory every mutation/read family so omissions are visible.

**Investigate firmware feasibility early, before porting the whole bank.**
GnuCOBOL's generated C calls libcob; producing C does not remove the runtime
requirement. Generate C with the host compiler, cross-build a minimal libcob
and its required arithmetic support for the ESP-IDF targets, and link a static
posting module through the existing Rust firmware build. Disable unused screen,
indexed-file, XML/JSON and dynamic-loader facilities where supported; NanaCoin
uses Rust for these services. Measure actual flash, heap, stack, allocation and
threading requirements against each board profile. The Windows DLL is not a
firmware artifact. This is a compile/link/size investigation, not permission to
flash or erase hardware.

Exit: both desktop builds pass the profile; a minimal firmware link and measured
budget exist for each intended deployment board. If libcob cannot fit a board,
record that target as unsupported and resolve runtime reduction or deployment
placement before promising a deployable COBOL firmware. Desktop parity alone
does not satisfy board deployment. C translation/static integration is supported
by the [GnuCOBOL manual](https://gnucobol.sourceforge.io/doc/gnucobol.html);
ESP-IDF runtime viability remains an engineering investigation.

### Phase 1 — complete posting plans and engine integration

Replace the status-only posting prototype with bounded plans containing both
NC and USD legs, updated balances, correction annotations and ledger totals.
Move posting validation/application into the engine while retaining Rust's
durable publication machinery. Port exact scaling primitives and preserve
amount, browser exactness, insufficient-funds and overdraft error precedence.

Exit: issuance, retirement, payments, messages, USD wallets, refunds/reversals,
grouped-leg atomicity, failure/retry behavior and cross-build replay pass. Check
the C boundary with a compact set of width/decimal/capacity cases; repeated ABI
calls supplement the HTTP contracts rather than inflate their reported count.

### Phase 2 — members, policy and basic bank configuration

Port member creation/grants, human/bot eligibility, role/status changes, last
active Nana protection, account policies and bank configuration. Rust hashes
credentials and revokes sessions/keys based on engine outcomes; secret material
never becomes a public bank projection. The Rust backend remains the normal
default feature set.

Exit: identity/policy contracts and unauthorized/no-mutation cases pass on both
backends, including credential revocation and current-schema restart fixtures.

### Phase 3 — marketplace and physical fulfillment

Port listing creation/edit/cancel and direct purchase, BUY/SELL offers, private
offer projections, acceptance/decline/withdrawal, settlement windows, undo and
good deeds. Port fulfillment obligations, completion/disputes and correction
links. One event owns payment and related object-state changes together.

Exit: both payment directions, unfunded/closed cases, correction overdrafts,
no double refund, deadline persistence and history-retention behavior pass.
Add race/expiry and bounded-record-capacity contracts before this phase closes.

### Phase 4 — forex, gifts and digital art

Port quote book ordering, BID/ASK exchange plans, expiry/cancellation and member
limits; gift requests/contributions/net refunds; art metadata validation,
revision-checked purchase, ownership/gifting/equipping. Preserve atomic two-wallet
exchange and money-plus-ownership commits. Media/copyright service I/O stays Rust.

Exit: stale forms and underfunded multi-leg operations change nothing; retries,
ownership and wallet totals survive restart and switching builds. Add quote
expiry, book ordering, art-capacity and concurrent ownership contracts.

### Phase 5 — lending, lotto and whole-economy currency reform

Port applications/proposals/funding, credit activation, exact simple interest
and carried remainders, scheduled installments, overdue behavior and full payoff.
Rust schedules ticks and supplies wall time; COBOL decides eligible transitions.
Port lotto ticket escrow, persisted winner selection input, savings/delayed
interest and stepwise settlement. Rust supplies randomness; a draw's chosen
outcome is durable before payouts. Port atomic reform of every monetary record,
loan fraction, retained correction, quote and historical epoch projection.

Exit: deterministic fixed-time scheduler contracts cover interest, funding waits,
deadlines and restart mid-settlement, without waiting days. Add an opt-in desktop
test clock through a launcher interface if needed, never a public production
HTTP endpoint. Exercise exact/inexact reform, maximum intermediates and recovery
with pending loans, quotes, obligations, gifts and lotto pools on both engines.

### Phase 6 — remaining projections and completeness audit

The desktop matrix now includes 18 fixed-clock differential scenarios. These
compare complete banking HTTP response transcripts and reopen each fixture with
the opposite engine. Authentication responses are excluded and bank incarnation
identifiers are normalized; money, business IDs, times and object states remain
exact. See `conformance/differential.py` and its JSON report. The per-command
ownership audit is recorded above and in `inventory.py`; passing comparisons
alone does not certify ownership. The current matrix passes 110 HTTP contracts
per engine, all 18 differential traces and incompatible-module startup checks.
Both engines also pass their 225 Rust tests and strict all-target Clippy checks.
The direct ABI suite has 1,128 boundary vectors, normally run twice (2,256 calls).
`bank.cob` currently has 4,677 source lines including whitespace. The increase
is primarily explicit named ABI views and constants; the frame remains 512 bytes.

Port business queries and aggregates: account/ledger views, activity privacy,
wealth/flows, loan/exchange summaries, cursor/retention semantics and any remaining
business commands. Rust keeps JSON formatting and archive page I/O. Audit every
command and read projection against the phase-0 inventory. The COBOL backend
must contain the banking decisions and transitions, not call the Rust backend
for unported financial features.

Exit: all banking areas have positive, rejection, permission, boundary, retry
and recovery contracts. Differential comparisons normalize credentials, wall
times and random outcomes, while comparing financial values and object states
exactly. The baseline suite remains independent of implementation internals.

### Phase 7 — packaging and deployment choices

Package desktop Rust and COBOL artifacts separately; the COBOL package carries
its exact runtime dependencies and the Rust package requires none. Integrate
`BANK_ENGINE=rust|cobol` into firmware build/deploy tooling: Rust remains the
default, COBOL opts into the feature and target-specific generated C/static
runtime. Artifact checks prevent accidentally deploying a different engine or
board target. A COBOL artifact reports startup/link failures explicitly and
never silently runs the Rust backend.

Exit: each supported target can build either artifact, both pass conformance,
restart/cross-build replay and recovery tests, and measured runtime/firmware
budgets fit. Hardware timing/power-cut validation is a separate, explicitly
authorized deployment exercise. Publish the supported target matrix, engine
selection commands, package manifests and reproducible compiler/runtime versions.

### Gate shared by every phase

Add the interface contracts before moving its business rules. Run both builds
after each slice; preserve the Rust implementation and the current-schema
accounting/recovery invariants. Breaking disposable development schemas is
allowed if needed, but update both backends, fixtures and docs together—no
legacy decoders or old-data migration scaffolding. For the current shared schema,
cross-build reopening is a parity test, not a migration project. A phase is
complete only when its implementation, rejection paths, replay and applicable
deployment gate pass.


### P4 network isolation evidence

An independent all-Rust P4 v3.1 emulator-clock image was built with SDK debug
logging: `build_firmware.py --board p4 --engine rust --target-dir C:/np4dbg
--emulator-network --p4-emulator-rev3 --emulator-clock 1799996400
--emulator-debug`. It fits the 8 MiB application partition at 3,730,512 bytes.
The flag uses an isolated debug report directory and rejects incompatible
SDK profile reuse; short target paths are checked before compiling dependencies.

This image reproduces the pre-listener failure without the COBOL module.
The debug log repeatedly reports SDIO interrupt wakeups while the companion
receives no Wi-Fi RPC requests. After 326.97 seconds the owned emulator pair
was stopped. This establishes that the observed startup failure also affects
the all-Rust framework/ESP-Hosted path. It does not establish a bank-engine fault
or verify the P4 full-server runtime budget.

The existing single-register fallback was also tested using
`--emulator-single-register` in the disposable SDK copy. It built successfully
but still showed repeated interrupt wakeups and no listeners after 226.19
seconds. This fallback uses separate word-sized register reads; it does not
establish that byte-wise register access was tested. The normal board profile
and banking code are unchanged. Experimental SDK copies require their explicit
flags on subsequent builds, preventing accidental reuse as normal firmware.

`--emulator-packet-rx` then disabled streaming RX in the same isolated all-Rust
experiment (both current and aliased SDK choices were verified). The image
linked and fit, but the 60-second startup probe also failed with the same
interrupt loop and no Wi-Fi RPC requests. The probe stopped both owned emulator
processes. Neither register-read nor packet-read experiments resolve the P4
emulator pre-listener blocker. No hardware was accessed during these experiments.
The subsequently authorized October 7 hardware deployment succeeds with the
normal SDK configuration, as recorded at the top of this document. Asset-load
heap/stack samples are now available from the intended revision-1.3 board;
maximum-capacity financial workload certification on P4 remains open.
