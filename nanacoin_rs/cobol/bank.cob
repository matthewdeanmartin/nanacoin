       >>source format free
identification division.
program-id. NCBANK.
data division.
working-storage section.
01 debit-value pic s9(38) comp-3.
01 credit-value pic s9(38) comp-3.
01 wide-value pic s9(38) comp-3.
01 factor-value pic 9(38) comp-3.
01 quotient-value pic s9(38) comp-3.
01 remainder-value pic s9(38) comp-3.
01 denominator-value pic 9(38) comp-3.
01 exponent-value usage binary-long signed.
01 min-i64 pic s9(20) comp-3 value -9223372036854775808.
01 loan-principal usage binary-long-long signed.
01 loan-interest usage binary-long-long signed.
01 loan-remainder usage binary-long-long signed.
01 loan-accrued usage binary-long-long signed.
01 loan-next-due usage binary-long-long signed.
01 loan-principal-due usage binary-long-long signed.
01 loan-interest-due usage binary-long-long signed.
01 loan-requested usage binary-long-long signed.
01 loan-paid usage binary-long-long signed.
01 loan-interest-paid usage binary-long-long signed.
01 loan-principal-paid usage binary-long-long signed.
01 loan-periods pic 9(38) comp-3.
01 loan-interval usage binary-long-long signed.
01 flow-index usage binary-long signed.
01 inbound-index usage binary-long signed.
01 outbound-index usage binary-long signed.
01 nana-direction usage binary-long signed.
01 flow-sign usage binary-long signed.
01 flow-deltas.
   02 flow-delta usage binary-long-long signed occurs 20.
01 selection-index usage binary-long signed.
01 selection-best usage binary-long signed.
01 selection-time usage binary-long-long signed.
01 selection-key usage binary-long-long signed.
01 quote-count usage binary-long signed.
01 quote-room usage binary-long signed.
01 quote-output usage binary-long signed.
01 quote-side usage binary-long signed.
01 quote-price usage binary-long-long signed.
01 quote-used.
   02 quote-selected usage binary-char occurs 16.
01 summary-float.
   02 summary-percent usage float-long.
01 summary-bits redefines summary-float pic x(8).
01 summary-decimal pic s9(20)v9(18) comp-3.
01 stats-first usage float-long.
01 stats-second usage float-long.
01 stats-total usage float-long.
01 stats-total-bits redefines stats-total pic x(8).
linkage section.
01 operation-code usage binary-long signed.
01 values-frame.
   02 slot usage binary-long-long signed occurs 64.
01 values-bytes redefines values-frame.
   02 byte-area pic x(512).
01 result-code usage binary-long signed.
procedure division using operation-code values-frame result-code.
    move 0 to result-code
    evaluate operation-code
        when 1 perform posting-plan
        when 2 perform exact-rescale
        when 3 perform exchange-cents
        when 4 perform annual-rate
        when 5 perform accrue-interest
        when 6 perform settle-loan
        when 7 perform reform-loan-fraction
        when 8 perform loan-posting-plan
        when 9 perform identity-query-policy
        when 10 perform administrator-policy
        when 11 perform protect-last-nana
        when 12 perform key-policy
        when 13 perform member-grant
        when 14 perform configuration-policy
        when 15 perform loan-policy
        when 16 perform loan-projection
        when 17 perform member-update-policy
        when 18 perform loan-book
        when 19 perform loan-summary
        when 20 perform fulfillment-transition
        when 21 perform offer-projection
        when 22 perform offer-transition
        when 23 perform member-import-policy
        when 24 perform oldest-eligible-record
        when 25 perform reversal-offer-links
        when 26 perform listing-transition
        when 27 perform first-eligible-record
        when 28 perform recycling-policy
        when 29 perform identity-transition
        when 30 perform classify-flows
        when 31 perform accumulate-flows
        when 32 perform lotto-transition
        when 33 perform lotto-projection
        when 34 perform lotto-book
        when 35 perform lotto-invariants
        when 36 perform reform-policy
        when 37 perform reform-lotto-consistency
        when 38 perform reform-field
        when 39 perform account-posting
        when 41 perform correction-policy
        when 42 perform historical-amount
        when 43 perform transfer-policy
        when 44 perform actor-policy
        when 48 perform new-member-policy
        when 49 perform identity-text-policy
        when 40 perform correction-plan
        when 45 perform fulfillment-capacity
        when 46 perform fulfillment-creation
        when 47 perform fulfillment-recycling
        when 50 perform quote-projection
        when 51 perform quote-transition
        when 52 perform quote-ordering
        when 53 perform ledger-projection
        when 54 perform activity-projection
        when 55 perform gift-transition
        when 56 perform gift-refund
        when 57 perform audit-policy
        when 58 perform art-transition
        when 59 perform art-metadata
        when 60 perform art-equipment
        when 61 perform bank-invariants
        when 62 perform market-view
        when 63 perform household-statistics
        when other move 1 to result-code
    end-evaluate
    goback.

identity-query-policy.
    *> Only eligibility facts enter this module; hashes and tokens stay Rust.
    *> The count fold fills the frame, including output positions. Preserve
    *> those input pairs until the complete fold has been read.
    if slot(1) not = 6 move 0 to slot(17) slot(18) end-if
    evaluate slot(1)
        when 0
            *> Self, actor Nana, target exists, target bot; delegated-view flag.
            if slot(2) = 0
                if slot(3) = 0 move 5 to result-code exit paragraph end-if
                if slot(4) = 0 move 7 to result-code exit paragraph end-if
                if slot(5) = 0 move 5 to result-code exit paragraph end-if
                move 1 to slot(17)
            end-if
        when 1
            *> Delegated target, read scope, mutation.
            if slot(2) = 1 and slot(3) = 1 and slot(4) = 1 move 5 to result-code end-if
        when 2
            *> Matched key member exists, disabled, has login; eligible flag.
            if slot(2) = 1 and slot(3) = 0 and slot(4) = 1 move 1 to slot(17) end-if
        when 3
            *> Stored key present and creation time; active/time projection.
            if slot(2) = 1 move 1 to slot(17) move slot(3) to slot(18) end-if
        when 4
            *> Wire mutation, reformed bank, reform route, matching epoch.
            if slot(2) = 1 and slot(3) = 1 and slot(4) = 0 and slot(5) = 0
                move 18 to result-code end-if
        when 5
            *> Durable retry already checked: fresh command epoch must match.
            if slot(2) = 0 move 18 to result-code end-if
        when 6
            *> Active Nana fold: previous count, row count, Nana/disabled pairs.
            if slot(2) < 0 or slot(2) > 32 or slot(3) < 0 or slot(3) > 30
                move 1 to result-code exit paragraph end-if
            move slot(2) to selection-best
            perform varying selection-index from 1 by 1 until selection-index > slot(3)
                compute inbound-index = 4 + (selection-index - 1) * 2
                if slot(inbound-index) = 1 and slot(inbound-index + 1) = 0
                    add 1 to selection-best end-if
            end-perform
            if selection-best > 32 move 1 to result-code
            else move selection-best to slot(17) end-if
        when other move 1 to result-code
    end-evaluate.

loan-posting-plan.
    *> Draw flag/amount, principal/interest paid, lender/borrower.
    *> Output 17 count; 18..22 and 23..27 from/to/amount/kind/tx offset.
    *> Outputs 28..31 aggregate validation amount/from/to/present.
    if (slot(1) not = 0 and slot(1) not = 1) or slot(2) < 0 or slot(3) < 0 or slot(4) < 0
        move 1 to result-code exit paragraph end-if
    perform varying selection-index from 17 by 1 until selection-index > 31
        move 0 to slot(selection-index)
    end-perform
    if slot(1) = 1
        if slot(2) > 0
            move 1 to slot(17) move slot(5) to slot(18) move slot(6) to slot(19)
            move slot(2) to slot(20) move 3 to slot(21)
        end-if
    else
        if slot(3) > 0
            move 1 to slot(17) move slot(6) to slot(18) move slot(5) to slot(19)
            move slot(3) to slot(20) move 3 to slot(21)
        end-if
        if slot(4) > 0
            if slot(17) = 0
                move 1 to slot(17) move slot(6) to slot(18) move slot(5) to slot(19)
                move slot(4) to slot(20) move 4 to slot(21)
            else
                move 2 to slot(17) move slot(6) to slot(23) move slot(5) to slot(24)
                move slot(4) to slot(25) move 4 to slot(26) move 4096 to slot(27)
            end-if
        end-if
    end-if
    if slot(1) = 1
        move slot(2) to wide-value move slot(5) to slot(29) move slot(6) to slot(30)
    else
        compute wide-value = slot(3) + slot(4)
        move slot(6) to slot(29) move slot(5) to slot(30)
    end-if
    if wide-value > 9223372036854775807 move 2 to result-code exit paragraph end-if
    move wide-value to slot(28)
    if wide-value > 0 move 1 to slot(31) end-if.

posting-plan.
    *> Inputs: source, target, units, issuance, correction, USD;
    *> slot 9 = prepared publication mode, slot 10 = same-wallet fact.
    *> Slots 11/12 parties, 13 loan-reference fact, 14 incoming credit mask.
    *> Outputs: debit/credit in 7/8, cash leg in 17, next credit mask in 18.
    if operation-code = 1 and slot(9) = 1 and (slot(14) < 0 or slot(14) > 4294967295)
        move 1 to result-code exit paragraph end-if
    if operation-code = 1 move 1 to slot(17) end-if
    if operation-code = 1 and slot(9) = 1 and slot(3) = 0
        move slot(1) to slot(7) move slot(2) to slot(8)
        move 0 to slot(17) perform posting-credit-policy exit paragraph
    end-if
    if slot(3) < 1 or slot(3) > 1000000000000000
        move 1 to result-code
        exit paragraph
    end-if
    if operation-code = 1 and slot(9) = 1 and slot(10) = 1
        if slot(1) not = slot(2) move 1 to result-code exit paragraph end-if
        move slot(1) to slot(7) move slot(2) to slot(8)
        perform posting-credit-policy
        exit paragraph
    end-if
    compute debit-value = slot(1) - slot(3)
    compute credit-value = slot(2) + slot(3)
    if debit-value < -9007199254740991
        or credit-value > 9007199254740991
        or (slot(6) = 1 and
            (debit-value > 9007199254740991
             or credit-value < -9007199254740991))
        move 2 to result-code
        exit paragraph
    end-if
    if slot(5) = 0 and slot(4) = 0 and debit-value < 0
        move 3 to result-code
        exit paragraph
    end-if
    move debit-value to slot(7)
    move credit-value to slot(8)
    perform posting-credit-policy.

posting-credit-policy.
    *> Nested account/transfer validation retains its own metadata slots.
    if operation-code not = 1 exit paragraph end-if
    move slot(14) to slot(18)
    if slot(9) = 1 and slot(6) = 0 and slot(3) > 0
        perform varying selection-index from 11 by 1 until selection-index > 12
            if slot(selection-index) >= 1 and slot(selection-index) <= 32
                compute factor-value = 2 ** (slot(selection-index) - 1)
                divide slot(18) by factor-value giving quotient-value
                compute remainder-value = function mod(quotient-value, 2)
                if slot(13) = 1
                    if remainder-value = 0 add factor-value to slot(18) end-if
                else
                    if remainder-value = 1 subtract factor-value from slot(18) end-if
                end-if
            end-if
        end-perform
    end-if.

exact-rescale.
    *> Inputs: units, decimal exponent, absolute result limit.
    *> Output: exact converted units in slot 4; never round money.
    if slot(1) = 0
        move 0 to slot(4)
        exit paragraph
    end-if
    if slot(2) < -38 or slot(2) > 38
        move 2 to result-code
        exit paragraph
    end-if
    if slot(2) = -38
        move 4 to result-code
        exit paragraph
    end-if
    if slot(2) = 38
        move 2 to result-code
        exit paragraph
    end-if
    compute exponent-value = function abs(slot(2))
    compute factor-value = 10 ** exponent-value
        on size error move 2 to result-code
    end-compute
    if result-code not = 0 exit paragraph end-if
    if slot(2) >= 0
        compute wide-value = slot(1) * factor-value
            on size error move 2 to result-code
        end-compute
    else
        divide slot(1) by factor-value giving quotient-value
            remainder remainder-value
        if remainder-value not = 0
            move 4 to result-code
            exit paragraph
        end-if
        move quotient-value to wide-value
    end-if
    if result-code not = 0 exit paragraph end-if
    if function abs(wide-value) > slot(3)
        move 2 to result-code
        exit paragraph
    end-if
    move wide-value to slot(4).

exchange-cents.
    *> Inputs: coins, cents per coin, NC scale. Output cents slot 4.
    if slot(3) <= 0
        move 1 to result-code
        exit paragraph
    end-if
    compute wide-value = slot(1) * slot(2)
        on size error move 2 to result-code
    end-compute
    if result-code not = 0 exit paragraph end-if
    divide wide-value by slot(3) giving quotient-value
    if quotient-value < min-i64
        or quotient-value > 9223372036854775807
        move 2 to result-code
        exit paragraph
    end-if
    move quotient-value to slot(4).

annual-rate.
    *> Inputs: basis points per period, period days. Output slot 3.
    compute denominator-value = function max(1, slot(2))
    compute quotient-value = slot(1) * 365 / denominator-value
    compute slot(3) = function min(quotient-value, 4294967295).

accrue-interest.
    *> Inputs: principal, rate bps, rate days, now, accrued at,
    *> interest, fractional remainder. Outputs interest/remainder/time 8-10.
    if slot(4) < slot(5)
        move 9 to result-code
        exit paragraph
    end-if
    compute denominator-value = 10000 * slot(3) * 86400
    if denominator-value = 0
        move 1 to result-code
        exit paragraph
    end-if
    compute wide-value = slot(1) * slot(2) * (slot(4) - slot(5))
        + slot(7)
        on size error move 2 to result-code
    end-compute
    if result-code not = 0 exit paragraph end-if
    divide wide-value by denominator-value giving quotient-value
        remainder remainder-value
    compute credit-value = slot(6) + quotient-value
    if credit-value + slot(1) > 9007199254740991
        move 2 to result-code
        exit paragraph
    end-if
    move credit-value to slot(8)
    move remainder-value to slot(9)
    move slot(4) to slot(10)
    move denominator-value to slot(11).

administrator-policy.
    *> Empty bank, actor id, actor exists, Nana, disabled.
    if (slot(1) = 1 and slot(2) = 1)
        or (slot(3) = 1 and slot(4) = 1 and slot(5) = 0)
        continue
    else
        move 5 to result-code
    end-if.

protect-last-nana.
    *> Current Nana/disabled, demoting/disabling, active Nana count.
    if slot(1) = 1 and slot(2) = 0 and slot(3) = 1
        and slot(4) = 1
        move 5 to result-code
    end-if.

key-policy.
    *> Self, administrator, revoke, bot full-key, has login, disabled.
    if slot(1) = 0 and (slot(2) = 0 or
        (slot(3) = 0 and slot(4) = 0))
        move 5 to result-code
        exit paragraph
    end-if
    if slot(3) = 0 and (slot(5) = 0 or slot(6) = 1)
        move 5 to result-code
    end-if.

member-grant.
    *> Issuance balance and initial grant; zero is allowed.
    if slot(2) < 0 or slot(2) > 1000000000000000
        move 1 to result-code
        exit paragraph
    end-if
    compute debit-value = slot(1) - slot(2)
    if debit-value < -9007199254740991
        move 2 to result-code
    end-if.

configuration-policy.
    *> Initial grant, optional settlement interval (-1 absent), now.
    if slot(1) < 0 or slot(1) > 1000000000000000
        move 1 to result-code
        exit paragraph
    end-if
    compute wide-value = function max(0, 9007199254740991 - slot(3))
    if slot(2) > wide-value move 1 to result-code end-if.

settle-loan.
    *> Status/manual/request/now/wallet/credit-block; loan numeric fields
    *> 7-13; amount/rate/rate days/payment days/installment in 15-19.
    *> Outputs 21-33 are the next loan and its principal/interest cash legs.
    *> Raw borrower/lender disabled facts in slots 20 and 34.
    if slot(4) < 1577836800 or slot(4) > 9007199254740991
        move 9 to result-code exit paragraph end-if
    if slot(20) = 1 or slot(34) = 1
        move 6 to result-code exit paragraph end-if
    move slot(7) to loan-principal
    move slot(8) to loan-interest
    move slot(9) to loan-remainder
    move slot(10) to loan-accrued
    move slot(11) to loan-next-due
    move slot(12) to loan-principal-due
    move slot(13) to loan-interest-due
    move 0 to loan-paid loan-interest-paid loan-principal-paid slot(33)
    compute loan-interval = slot(18) * 86400
    if slot(1) = 2 and slot(2) = 0
        if slot(5) not = 0 or slot(6) not = 0
            move 4 to result-code
            exit paragraph
        end-if
        compute wide-value = slot(4) + loan-interval
        if wide-value > 9007199254740991
            move 2 to result-code
            exit paragraph
        end-if
        move wide-value to loan-next-due
        move slot(15) to loan-principal
        move slot(4) to loan-accrued
        move 1 to slot(21) slot(33)
        perform publish-loan
        exit paragraph
    end-if
    if slot(1) not = 1
        move 4 to result-code
        exit paragraph
    end-if
    if slot(4) < loan-accrued
        move 9 to result-code
        exit paragraph
    end-if
    compute denominator-value = 10000 * slot(17) * 86400
    compute wide-value = loan-principal * slot(16)
        * (slot(4) - loan-accrued) + loan-remainder
        on size error move 2 to result-code
    end-compute
    if result-code not = 0 exit paragraph end-if
    divide wide-value by denominator-value giving quotient-value
        remainder remainder-value
    compute credit-value = loan-interest + quotient-value
    if credit-value + loan-principal > 9007199254740991
        move 2 to result-code
        exit paragraph
    end-if
    move credit-value to loan-interest
    move remainder-value to loan-remainder
    move slot(4) to loan-accrued
    if slot(4) >= loan-next-due
        compute loan-periods = (slot(4) - loan-next-due) / loan-interval
        add 1 to loan-periods
        compute wide-value = loan-principal-due
            + loan-periods * slot(19)
        compute loan-principal-due = function min(wide-value, loan-principal)
        move loan-interest to loan-interest-due
        compute wide-value = loan-next-due + loan-periods * loan-interval
        if wide-value > 9007199254740991
            move 2 to result-code
            exit paragraph
        end-if
        move wide-value to loan-next-due
    end-if
    if slot(2) = 1
        if slot(3) < 1 or slot(3) > 1000000000000000
            move 1 to result-code
            exit paragraph
        end-if
        compute loan-requested = function min(slot(3),
            loan-principal + loan-interest)
        if slot(5) < loan-requested
            move 3 to result-code
            exit paragraph
        end-if
    else
        compute loan-requested = function min(1000000000000000,
            loan-principal-due + loan-interest-due)
    end-if
    compute loan-paid = function min(loan-requested, function max(0, slot(5)))
    compute loan-interest-paid = function min(loan-paid, loan-interest)
    compute loan-principal-paid = loan-paid - loan-interest-paid
    subtract loan-interest-paid from loan-interest
    subtract loan-principal-paid from loan-principal
    compute loan-interest-due = function max(0,
        loan-interest-due - loan-interest-paid)
    compute loan-principal-due = function max(0,
        loan-principal-due - loan-principal-paid)
    move 1 to slot(21)
    if loan-principal = 0 and loan-interest = 0
        move 3 to slot(21)
        move 0 to loan-remainder loan-next-due
    end-if
    perform publish-loan.

publish-loan.
    move loan-principal to slot(22)
    move loan-interest to slot(23)
    move loan-remainder to slot(24)
    move loan-accrued to slot(25)
    move loan-next-due to slot(26)
    move loan-principal-due to slot(27)
    move loan-interest-due to slot(28)
    compute slot(29) = slot(5) - loan-paid
    move slot(4) to slot(30)
    move loan-principal-paid to slot(31)
    move loan-interest-paid to slot(32).

reform-loan-fraction.
    *> Interest, remainder, denominator, exponent, converted principal.
    *> Keep the fractional claim exact when reforming the economy.
    if slot(4) < -24 or slot(4) > 24 or slot(3) <= 0
        move 1 to result-code
        exit paragraph
    end-if
    compute factor-value = 10 ** function abs(slot(4))
    compute wide-value = slot(1) * slot(3) + slot(2)
    if slot(4) >= 0
        compute wide-value = wide-value * factor-value
            on size error move 2 to result-code
        end-compute
        if result-code not = 0 exit paragraph end-if
    else
        divide wide-value by factor-value giving quotient-value
            remainder remainder-value
        if remainder-value not = 0
            move 4 to result-code
            exit paragraph
        end-if
        move quotient-value to wide-value
    end-if
    divide wide-value by slot(3) giving quotient-value
        remainder remainder-value
    if quotient-value + slot(5) > 9007199254740991
        move 2 to result-code
        exit paragraph
    end-if
    move quotient-value to slot(6)
    move remainder-value to slot(7).

fulfillment-transition.
    *> Status/action/actor/provider/recipient/empty-reason/control-byte.
    *> Complete is allowed to either party; only recipient disputes.
    if slot(7) = 1
        move 1 to result-code
        exit paragraph
    end-if
    if slot(3) not = slot(5) and
        (slot(2) not = 0 or slot(3) not = slot(4))
        move 5 to result-code
        exit paragraph
    end-if
    evaluate slot(2)
        when 0
            if slot(1) not = 0 move 4 to result-code end-if
            move 1 to slot(8)
        when 1
            if slot(1) not = 0 and slot(1) not = 1
                move 4 to result-code
            end-if
            move 2 to slot(8)
        when 2
            if slot(1) not = 2 move 4 to result-code end-if
            move 1 to slot(8)
        when other move 1 to result-code
    end-evaluate
    if result-code not = 0 exit paragraph end-if
    if slot(2) = 1 and slot(6) = 1 move 1 to result-code end-if.

classify-flows.
    *> Amount/from/to/USD/quote/lotto/reversal/economic kind.
    *> Output slots 9-28: the twenty ledger flow deltas.
    initialize flow-deltas
    move 1 to flow-delta(20) flow-sign
    if slot(7) = 1 move -1 to flow-sign end-if
    move 0 to nana-direction inbound-index outbound-index
    if slot(3) = 1 move 1 to nana-direction
    else if slot(2) = 1 move -1 to nana-direction end-if end-if
    if slot(4) = 1
        evaluate true
            when slot(2) = 0 move slot(1) to flow-delta(7)
            when slot(3) = 0 compute flow-delta(7) = 0 - slot(1)
            when slot(5) = 1
                move 11 to inbound-index
                move 9 to outbound-index
        end-evaluate
    else
        evaluate true
            when slot(2) = 0 or slot(3) = 0
                if slot(2) = 0 move slot(1) to flow-delta(6)
                else compute flow-delta(6) = 0 - slot(1) end-if
                evaluate true
                    when slot(7) = 1 move flow-delta(6) to flow-delta(5)
                    when slot(3) = 0 move slot(1) to flow-delta(4)
                    when slot(6) = 1 move slot(1) to flow-delta(3)
                    when slot(3) = 1 move slot(1) to flow-delta(1)
                    when other move slot(1) to flow-delta(2)
                end-evaluate
            when slot(5) = 1
                move 8 to inbound-index
                move 10 to outbound-index
            when slot(8) = 4
                move 12 to inbound-index
                move 13 to outbound-index
            when slot(8) = 3
                move 15 to inbound-index
                move 14 to outbound-index
            when slot(8) = 1 or slot(8) = 2
                move 17 to inbound-index
                move 16 to outbound-index
            when other
                move 19 to inbound-index
                move 18 to outbound-index
        end-evaluate
    end-if
    if inbound-index not = 0 and nana-direction not = 0
        if nana-direction * flow-sign > 0
            move inbound-index to flow-index
        else
            move outbound-index to flow-index
        end-if
        compute flow-delta(flow-index) = slot(1) * flow-sign
    end-if
    perform varying flow-index from 1 by 1 until flow-index > 20
        move flow-delta(flow-index) to slot(flow-index + 8)
    end-perform.

accumulate-flows.
    *> Slots 1-20: deltas. Slots 21-60: signed high/low pairs in base 10^15.
    *> Update pairs in place, without exposing Rust's i128 representation.
    perform varying flow-index from 1 by 1 until flow-index > 20
        compute inbound-index = 19 + flow-index * 2
        compute wide-value = slot(inbound-index) * 1000000000000000
            + slot(inbound-index + 1) + slot(flow-index)
        if function abs(wide-value) >= 20282409603651670423947251286016
            move 2 to result-code
            exit paragraph
        end-if
        divide wide-value by 1000000000000000 giving quotient-value
            remainder remainder-value
        move quotient-value to slot(inbound-index)
        move remainder-value to slot(inbound-index + 1)
    end-perform.

correction-plan.
    *> Original units, previously refunded units, this correction's units,
    *> correction transaction id. Outputs refunded units and full-link id.
    if slot(1) <= 0 or slot(2) < 0 or slot(3) <= 0
        move 4 to result-code
        exit paragraph
    end-if
    compute wide-value = slot(2) + slot(3)
    if wide-value > slot(1)
        move 4 to result-code
        exit paragraph
    end-if
    move wide-value to slot(5)
    move 0 to slot(6)
    if wide-value = slot(1) move slot(4) to slot(6) end-if.

offer-projection.
    *> Phase/now/settles-at/viewer/Nana/disabled/owner/offerer.
    *> Outputs status/reversible/visible/recyclable/pinned in slots 9-13.
    move slot(1) to slot(9)
    *> Slot 14 optionally supplies listing liveness (-1 means no joined view).
    if slot(1) = 0 and slot(14) = 0 move 6 to slot(9) end-if
    move 0 to slot(10) slot(11) slot(12) slot(13)
    if slot(1) = 1
        if slot(2) >= slot(3)
            move 5 to slot(9)
            move 1 to slot(12)
        else
            move 1 to slot(13)
            if slot(2) >= 1577836800 move 1 to slot(10) end-if
        end-if
    else
        if slot(1) not = 0 move 1 to slot(12) end-if
    end-if
    if slot(6) = 0 and (slot(5) = 1 or slot(4) = slot(7)
        or slot(4) = slot(8)) move 1 to slot(11) end-if.

offer-transition.
    *> Action 0 make, 1 accept, 2 undo, 3 decline, 4 withdraw.
    *> Input record is documented in Rust's offer_plan adapter.
    *> Outputs phase/payer/payee/deadline/overflow/listing-mutation 31-36.
    if slot(2) < 1577836800 or slot(2) > 9007199254740991
        move 9 to result-code
        exit paragraph
    end-if
    move 0 to slot(31) slot(32) slot(33) slot(34) slot(35) slot(36)
    if slot(1) = 0
        if slot(5) < 1 or slot(5) > 1000000000000000 or slot(6) = 1
            move 1 to result-code
            exit paragraph
        end-if
        if slot(7) = 0 move 7 to result-code exit paragraph end-if
        if slot(8) = 0 move 11 to result-code exit paragraph end-if
        if slot(3) = slot(10) move 12 to result-code exit paragraph end-if
        if slot(12) = 1 and slot(15) = 1
            move 13 to result-code exit paragraph
        end-if
        if slot(16) = 1 and slot(17) = 0 move 8 to result-code end-if
        exit paragraph
    end-if
    if slot(18) = 0 move 7 to result-code exit paragraph end-if
    evaluate slot(1)
        when 1
            if slot(3) not = slot(20)
                move 5 to result-code exit paragraph
            end-if
            if slot(19) not = 0 move 14 to result-code exit paragraph end-if
            if slot(7) = 0 move 7 to result-code exit paragraph end-if
            if slot(8) = 0 move 11 to result-code exit paragraph end-if
            if slot(22) = -1 move 7 to result-code exit paragraph end-if
            if slot(22) = 1 move 6 to result-code exit paragraph end-if
            if slot(13) = 1 and
                (((slot(12) = 1 or slot(11) = 1) and slot(30) = 1) or
                 (slot(12) = 0 and slot(11) = 0 and slot(14) = 1))
                move 5 to result-code exit paragraph
            end-if
            evaluate true
                when slot(12) = 1
                    move 0 to slot(32)
                    move slot(21) to slot(33)
                when slot(11) = 1
                    move slot(20) to slot(32)
                    move slot(21) to slot(33)
                when other
                    move slot(21) to slot(32)
                    move slot(20) to slot(33)
            end-evaluate
            move 1 to slot(31)
            compute wide-value = slot(2) + slot(26)
            *> Funds are checked before this deferred overflow result.
            if wide-value > 9007199254740991 move 1 to slot(35)
            else move wide-value to slot(34) end-if
            if slot(12) = 0 move 1 to slot(36) end-if
        when 2
            if slot(3) not = slot(20) and slot(3) not = slot(21)
                and slot(4) = 0 move 5 to result-code exit paragraph end-if
            if slot(19) not = 1 move 14 to result-code exit paragraph end-if
            if slot(2) >= slot(23) move 15 to result-code exit paragraph end-if
            if slot(6) = 1 move 1 to result-code exit paragraph end-if
            if slot(24) not = 0
                if slot(7) = 0 move 7 to result-code exit paragraph end-if
                if slot(9) = 0 move 4 to result-code exit paragraph end-if
                move 1 to slot(36)
            end-if
            if slot(27) = 0 move 8 to result-code exit paragraph end-if
            if slot(28) not = 0 or slot(29) = 1
                move 4 to result-code exit paragraph
            end-if
            move 4 to slot(31)
            move slot(25) to slot(32)
            move slot(24) to slot(33)
        when 3 when 4
            if slot(19) not = 0 move 14 to result-code exit paragraph end-if
            if slot(1) = 3
                move slot(20) to slot(32)
                move 2 to slot(31)
            else
                move slot(21) to slot(32)
                move 3 to slot(31)
            end-if
            if slot(3) not = slot(32) and slot(4) = 0
                move 5 to result-code
            end-if
        when other move 1 to result-code
    end-evaluate.

fulfillment-capacity.
    *> Command category: 0 other, 1 purchase/accept, 2 classified transfer.
    *> amount, physical economic kind (1 work/2 goods), full, recyclable.
    if (slot(1) = 1 or (slot(1) = 2 and slot(2) > 0 and
        (slot(3) = 1 or slot(3) = 2))) and
        slot(4) = 1 and slot(5) = 0
        move 8 to result-code
    end-if.

fulfillment-creation.
    *> reversal, full correction, USD, amount, loan/lotto/quote/art,
    *> payer, listing present, physical kind, cash/service details.
    *> Outputs: action (skip/create/reverse), kind (work/goods/cash), status.
    move 0 to slot(14) slot(15) slot(16)
    if slot(1) = 1
        if slot(2) = 1 move 2 to slot(14) move 3 to slot(16) end-if
        exit paragraph
    end-if
    if slot(3) = 1 or slot(4) = 0 or slot(5) = 1 or
        slot(6) = 1 or slot(7) = 1 or slot(8) = 1 or slot(9) = 0
        exit paragraph
    end-if
    if slot(10) = 0 and slot(11) not = 1 and slot(11) not = 2
        exit paragraph
    end-if
    move 1 to slot(14)
    evaluate true
        when slot(12) = 1 move 2 to slot(15)
        when slot(11) = 1 or slot(13) = 1 move 0 to slot(15)
        when other move 1 to slot(15)
    end-evaluate.

fulfillment-recycling.
    *> Count <=31, then status/history-present pairs. First eligible index.
    if slot(1) < 0 or slot(1) > 31
        move 1 to result-code exit paragraph
    end-if
    move -1 to slot(64)
    perform varying selection-index from 1 by 1 until selection-index > slot(1)
        compute flow-index = selection-index * 2
        if (slot(flow-index) = 1 or slot(flow-index) = 3) and
            slot(flow-index + 1) = 0
            compute slot(64) = selection-index - 1
            exit perform
        end-if
    end-perform.

oldest-eligible-record.
    *> Thirty-two timestamp/id pairs; a timestamp of -1 is ineligible.
    *> Output slot 1 is the selected zero-based index or -1.
    move -1 to selection-best
    perform varying selection-index from 1 by 1 until selection-index > 32
        compute flow-index = selection-index * 2 - 1
        if slot(flow-index) >= 0
            if selection-best = -1 or slot(flow-index) < selection-time
                or (slot(flow-index) = selection-time
                    and slot(flow-index + 1) < selection-key)
                compute selection-best = selection-index - 1
                move slot(flow-index) to selection-time
                move slot(flow-index + 1) to selection-key
            end-if
        end-if
    end-perform
    move selection-best to slot(1).

reversal-offer-links.
    *> Original transaction/count, then at most sixteen phase/transaction pairs.
    *> Output slot 35 is the bit mask of accepted offers linked to a full refund.
    if slot(2) < 0 or slot(2) > 16
        move 1 to result-code exit paragraph
    end-if
    move 0 to slot(35)
    perform varying selection-index from 1 by 1 until selection-index > slot(2)
        compute flow-index = selection-index * 2 + 1
        if slot(flow-index) = 1 and slot(flow-index + 1) = slot(1)
            compute slot(35) = slot(35) + 2 ** (selection-index - 1)
        end-if
    end-perform.

recycling-policy.
    *> Kind (0 listing, 1 thing), active/standard, pinned/live dependency.
    move 0 to slot(4)
    if slot(2) = 0 and slot(3) = 0 move 1 to slot(4) end-if.

first-eligible-record.
    *> Sixty-four eligibility flags; output slot 1 selects the first, or -1.
    move -1 to selection-best
    perform varying selection-index from 1 by 1 until selection-index > 64
        if slot(selection-index) = 1
            compute selection-best = selection-index - 1
            exit perform
        end-if
    end-perform
    move selection-best to slot(1).

listing-transition.
    *> Action (0 list, 1 cancel, 2 edit, 3 buy, 4 classified list).
    *> Facts are documented in the Rust listing_plan adapter.
    *> Outputs status/payer/payee/thing action/standard in 31-35.
    move 0 to slot(31) slot(32) slot(33) slot(34) slot(35)
    if slot(1) = 0 or slot(1) = 4
        if slot(1) = 0 and slot(16) = 1
            move slot(20) to wide-value
            if slot(17) = 4 and (slot(4) = 0 or slot(8) = 0
                or slot(18) = 0 or slot(20) not = 0)
                move 5 to result-code exit paragraph
            end-if
            if slot(17) = -1 or slot(19) = 1
                or function abs(wide-value) > 9007199254740991
                or (slot(17) = 3 and (slot(18) = 1 or slot(20) <= 0))
                move 1 to result-code exit paragraph
            end-if
        end-if
        if slot(13) = 1 or slot(12) < 1 or slot(12) > 1000000000000000
            move 1 to result-code exit paragraph
        end-if
        if slot(1) = 4
            if slot(23) < 1 or slot(23) > 1000000000
                move 1 to result-code exit paragraph
            end-if
            if slot(8) = 0 and slot(10) = 1 and slot(4) = 1
                move 5 to result-code exit paragraph
            end-if
            evaluate true
                when slot(24) not = 0
                    if slot(25) = 0 move 7 to result-code exit paragraph end-if
                    if slot(26) = 0 move 1 to result-code exit paragraph end-if
                    move 0 to slot(34)
                when slot(27) = 1
                    if slot(28) = 0 move 4 to result-code exit paragraph end-if
                    move 1 to slot(34)
                when other
                    if slot(29) = 1 and slot(30) = 0
                        move 8 to result-code exit paragraph
                    end-if
                    move 2 to slot(34)
            end-evaluate
            if slot(37) = 1 or slot(38) = 1 move 1 to slot(35) end-if
        end-if
        if slot(21) = 1 and slot(22) = 0 move 8 to result-code end-if
        exit paragraph
    end-if
    if slot(5) = 0 move 7 to result-code exit paragraph end-if
    if slot(1) = 1 or slot(1) = 2
        if slot(2) not = slot(6) and slot(3) = 0
            move 5 to result-code exit paragraph
        end-if
    end-if
    if slot(7) = 0 move 4 to result-code exit paragraph end-if
    evaluate slot(1)
        when 1 move 2 to slot(31)
        when 2
            if (slot(14) = 1 and slot(13) = 1) or
                (slot(15) = 1 and (slot(12) < 1 or slot(12) > 1000000000000000))
                move 1 to result-code exit paragraph
            end-if
            move -1 to slot(31)
        when 3
            if slot(9) = 1 move 5 to result-code exit paragraph end-if
            if slot(8) = 1
                move slot(6) to slot(32)
                move slot(2) to slot(33)
            else
                move slot(2) to slot(32)
                move slot(6) to slot(33)
            end-if
            if slot(10) = 1 and
                ((slot(8) = 1 and slot(39) = 1) or
                 (slot(8) = 0 and slot(11) = 1))
                move 5 to result-code exit paragraph
            end-if
            move 1 to slot(31)
        when other move 1 to result-code
    end-evaluate.

quote-projection.
    *> status, now, expiry, side (ASK=0), maker, taker; outputs 7-10.
    move 0 to slot(7)
    move slot(1) to slot(8)
    if slot(1) = 0
        if slot(2) >= 1577836800 and slot(2) <= 9007199254740991
            and (slot(3) = 0 or slot(2) < slot(3))
            move 1 to slot(7)
        end-if
        if slot(2) not = 0 and slot(3) not = 0 and slot(2) >= slot(3)
            move 3 to slot(8)
        end-if
    end-if
    if slot(4) = 0
        move slot(5) to slot(9) move slot(6) to slot(10)
    else
        move slot(6) to slot(9) move slot(5) to slot(10)
    end-if.

quote-transition.
    *> action post/take/cancel, actor, admin, now, exists, maker, side,
    *> stored status, live, rate, coins, scale, expiry.
    *> Book inputs 27-58 are sixteen maker/live pairs; empty maker is zero.
    *> Outputs 17 status, 18 seller, 19 buyer, 20 cash total.
    move 0 to slot(17) slot(18) slot(19) slot(20)
    if slot(1) not = 2 and
        (slot(4) < 1577836800 or slot(4) > 9007199254740991)
        move 9 to result-code exit paragraph
    end-if
    if slot(1) = 0
        if slot(10) < 1 or slot(10) > 1000000000000000 or
            slot(11) < 1 or slot(11) > 1000000000000000 or slot(12) < 1 or
            (slot(13) not = 0 and slot(13) <= slot(4)) or
            slot(13) > 9007199254740991
            move 1 to result-code exit paragraph
        end-if
        compute wide-value = slot(10) * slot(11)
        divide wide-value by slot(12) giving quotient-value remainder remainder-value
        if remainder-value not = 0 or quotient-value < 1 or
            quotient-value > 1000000000000000
            move 1 to result-code exit paragraph
        end-if
        move quotient-value to slot(20)
        move 0 to quote-count quote-room
        perform varying selection-index from 1 by 1 until selection-index > 16
            compute flow-index = 25 + selection-index * 2
            if slot(flow-index) = slot(2) and slot(flow-index + 1) = 1
                add 1 to quote-count
            end-if
            if slot(flow-index) = 0 or slot(flow-index + 1) = 0
                move 1 to quote-room
            end-if
        end-perform
        if quote-count >= 2 move 16 to result-code exit paragraph end-if
        if quote-room = 0 move 8 to result-code end-if
        exit paragraph
    end-if
    if slot(5) = 0 move 7 to result-code exit paragraph end-if
    if slot(1) = 2
        if slot(2) not = slot(6) and slot(3) = 0
            move 5 to result-code exit paragraph
        end-if
        if slot(8) not = 0 move 4 to result-code exit paragraph end-if
        move 2 to slot(17) exit paragraph
    end-if
    if slot(1) not = 1 move 1 to result-code exit paragraph end-if
    if slot(2) = slot(6) move 12 to result-code exit paragraph end-if
    if slot(9) = 0 move 4 to result-code exit paragraph end-if
    move 1 to slot(17)
    if slot(7) = 0
        move slot(6) to slot(18) move slot(2) to slot(19)
    else
        move slot(2) to slot(18) move slot(6) to slot(19)
    end-if
    compute wide-value = slot(10) * slot(11)
    divide wide-value by slot(12) giving quotient-value
    move quotient-value to slot(20).

quote-ordering.
    *> Sixteen side/price/id triples; side=-1 denotes an empty row.
    *> Sorted zero-based row indices in slots 49-64, empty outputs=-1.
    initialize quote-used
    perform varying quote-output from 49 by 1 until quote-output > 64
        move -1 to slot(quote-output)
    end-perform
    perform varying quote-output from 49 by 1 until quote-output > 64
        move -1 to slot(quote-output)
        move 0 to selection-best
        perform varying selection-index from 1 by 1 until selection-index > 16
            compute flow-index = selection-index * 3 - 2
            if slot(flow-index) >= 0 and quote-selected(selection-index) = 0
                move slot(flow-index + 1) to wide-value
                if slot(flow-index) = 1 compute wide-value = 0 - wide-value end-if
                if selection-best = 0 or slot(flow-index) < quote-side or
                    (slot(flow-index) = quote-side and wide-value < quote-price) or
                    (slot(flow-index) = quote-side and wide-value = quote-price and
                     slot(flow-index + 2) < selection-key)
                    move selection-index to selection-best
                    move slot(flow-index) to quote-side
                    move wide-value to quote-price
                    move slot(flow-index + 2) to selection-key
                end-if
            end-if
        end-perform
        if selection-best = 0 exit perform end-if
        move 1 to quote-selected(selection-best)
        compute slot(quote-output) = selection-best - 1
    end-perform.

gift-transition.
    *> action create/close/contribute, actor, disabled, now, capacity-full,
    *> exists, owner, closed, deadline-present/value, target-present/value,
    *> title-empty, received, amount. Outputs 21-24: received, overflow,
    *> closed, owner. Contribution overflow is deferred until funds validate.
    move 0 to slot(21) slot(22) slot(23) slot(24)
    if slot(3) = 1 move 6 to result-code exit paragraph end-if
    if slot(1) = 0
        if slot(5) = 1 move 8 to result-code exit paragraph end-if
        if slot(13) = 1 or (slot(11) = 1 and
            (slot(12) < 1 or slot(12) > 1000000000000000)) or
            (slot(9) = 1 and (slot(10) <= slot(4) or
             slot(10) > 9007199254740991 or slot(4) < 1577836800))
            move 1 to result-code exit paragraph
        end-if
        move slot(2) to slot(24)
        exit paragraph
    end-if
    if slot(6) = 0 move 7 to result-code exit paragraph end-if
    move slot(7) to slot(24)
    move slot(14) to slot(21)
    if slot(1) = 1
        if slot(7) not = slot(2) move 5 to result-code exit paragraph end-if
        if slot(8) = 1 move 4 to result-code exit paragraph end-if
        move 1 to slot(23) exit paragraph
    end-if
    if slot(1) not = 2 move 1 to result-code exit paragraph end-if
    if slot(8) = 1 or (slot(9) = 1 and
        (slot(4) >= slot(10) or slot(4) < 1577836800))
        move 4 to result-code exit paragraph
    end-if
    compute wide-value = slot(14) + slot(15)
    if wide-value < min-i64 or wide-value > 9007199254740991
        move 1 to slot(22)
    else
        move wide-value to slot(21)
    end-if.

gift-refund.
    *> received, current-epoch refund amount; output new received in slot 3.
    compute wide-value = slot(1) - slot(2)
    if wide-value < min-i64 or wide-value > 9223372036854775807
        move 2 to result-code exit paragraph
    end-if
    move wide-value to slot(3).

art-transition.
    *> action mint/list/buy/gift/equip, actor, disabled, full, exists,
    *> owner, price-present/value, equipped, revision, expected owner/revision/
    *> price, gift target/exists/disabled, equip requested, asset valid,
    *> event sequence, requested price-present/value.
    *> Outputs 31-37: owner, price-present/value, equipped, revision, seller, amount.
    move 0 to slot(31) slot(32) slot(33) slot(34) slot(35) slot(36) slot(37) slot(38)
    if slot(3) = 1 move 6 to result-code exit paragraph end-if
    if slot(1) = 0
        if slot(4) = 1 move 8 to result-code exit paragraph end-if
        if slot(18) = 0 move 1 to result-code exit paragraph end-if
        move slot(2) to slot(31) move slot(19) to slot(35)
        exit paragraph
    end-if
    if slot(5) = 0 move 7 to result-code exit paragraph end-if
    move slot(6) to slot(31)
    move slot(7) to slot(32) move slot(8) to slot(33)
    move slot(9) to slot(34) move slot(10) to slot(35)
    if slot(1) = 2
        if slot(6) not = slot(11) or slot(10) not = slot(12) or
            slot(7) = 0 or slot(8) not = slot(13)
            move 4 to result-code exit paragraph
        end-if
        move slot(6) to slot(36) move slot(13) to slot(37)
        move slot(2) to slot(31)
    else
        if slot(6) not = slot(2) move 5 to result-code exit paragraph end-if
        evaluate slot(1)
            when 1
                if slot(20) = 1 and (slot(21) < 1 or slot(21) > 1000000000000000)
                    move 1 to result-code exit paragraph
                end-if
                move slot(20) to slot(32) move slot(21) to slot(33)
                move slot(19) to slot(35) exit paragraph
            when 3
                if slot(14) = slot(2) move 1 to result-code exit paragraph end-if
                if slot(15) = 0 move 7 to result-code exit paragraph end-if
                if slot(16) = 1 move 6 to result-code exit paragraph end-if
                move slot(14) to slot(31)
            when 4
                move slot(17) to slot(34) slot(38) exit paragraph
            when other move 1 to result-code exit paragraph
        end-evaluate
    end-if
    move 0 to slot(32) slot(33) slot(34)
    move slot(19) to slot(35).

art-metadata.
    *> title/permission text empty flags, hash/url byte lengths in slots 1-4.
    *> Raw hash bytes at offset 33 (64 bytes), URL at 97 (up to 192 bytes).
    if slot(1) = 1 or slot(2) = 1 or slot(3) not = 64 or
        slot(4) < 9 or slot(4) > 192
        move 1 to result-code exit paragraph
    end-if
    perform varying selection-index from 33 by 1 until selection-index > 96
        compute flow-index = function ord(byte-area(selection-index:1)) - 1
        if not (flow-index >= 48 and flow-index <= 57) and
            not (flow-index >= 65 and flow-index <= 70) and
            not (flow-index >= 97 and flow-index <= 102)
            move 1 to result-code exit paragraph
        end-if
    end-perform
    if byte-area(97:8) not = 'https://' or byte-area(105:1) = '/' or '?' or '#'
        move 1 to result-code exit paragraph
    end-if
    compute selection-best = 96 + slot(4)
    perform varying selection-index from 97 by 1 until selection-index > selection-best
        compute flow-index = function ord(byte-area(selection-index:1)) - 1
        if flow-index < 33 or flow-index > 126 or flow-index = 64
            move 1 to result-code exit paragraph
        end-if
    end-perform.

art-equipment.
    *> actor, count<=32, then owner IDs; output bit mask of rows to unequip.
    if slot(2) < 0 or slot(2) > 32
        move 1 to result-code exit paragraph
    end-if
    move 0 to slot(64)
    perform varying selection-index from 1 by 1 until selection-index > slot(2)
        if slot(selection-index + 2) = slot(1)
            compute slot(64) = slot(64) + 2 ** (selection-index - 1)
        end-if
    end-perform.

loan-projection.
    *> status, Nana, viewer, lender, borrower, borrower/lender exist/disabled,
    *> borrower/lender balances, amount, credit-blocked, next-due, now,
    *> principal/interest due, attempted balance. Outputs visible/terminal/ready 21-23.
    *> Extra inputs: accrual failure, optional profile subject (zero=normal book).
    *> Extra outputs: waiting reason, include, redact, overdue, accrue requested.
    move 0 to slot(21) slot(22) slot(23) slot(24) slot(25) slot(26) slot(28)
    compute slot(27) = slot(16) + slot(17)
    if slot(1) = 6 or slot(2) = 1 or slot(3) = slot(4) or slot(3) = slot(5)
        move 1 to slot(21)
    end-if
    if slot(1) = 3 or 4 or 5 move 1 to slot(22) end-if
    if slot(20) = 0
        move slot(21) to slot(25)
    else
        if (slot(20) = slot(4) or slot(20) = slot(5)) and (slot(1) = 2 or 3)
            move 1 to slot(25)
        end-if
    end-if
    if slot(21) = 0 move 1 to slot(26) end-if
    evaluate true
        when slot(8) = 1 or slot(9) = 1 move 1 to slot(24)
        when slot(1) = 2
            move 1 to slot(28)
            if slot(19) = 1 move 2 to slot(24) end-if
        when slot(1) = 1
            evaluate true
                when slot(10) not = 0 move 3 to slot(24)
                when slot(13) = 1 move 4 to slot(24)
                when slot(11) < slot(12) move 5 to slot(24)
                when other move 6 to slot(24)
            end-evaluate
    end-evaluate
    if slot(6) = 0 or slot(7) = 0 or slot(8) = 1 or slot(9) = 1
        exit paragraph
    end-if
    evaluate slot(1)
        when 1
            if slot(10) = 0 and slot(11) >= slot(12) and slot(13) = 0
                move 1 to slot(23)
            end-if
        when 2
            compute wide-value = slot(16) + slot(17)
            if slot(15) >= slot(14) or (wide-value > 0 and slot(10) > 0 and slot(10) not = slot(18))
                move 1 to slot(23)
            end-if
    end-evaluate.

loan-book.
    *> Actor, count<=32, then lender*8+status words. Outputs count and terminal bitmap.
    if slot(2) < 0 or slot(2) > 32 move 1 to result-code exit paragraph end-if
    move 0 to slot(35) slot(36)
    perform varying selection-index from 1 by 1 until selection-index > slot(2)
        divide slot(selection-index + 2) by 8 giving quotient-value remainder remainder-value
        if quotient-value = slot(1) and remainder-value = 0 add 1 to slot(35) end-if
        if remainder-value = 3 or 4 or 5
            compute slot(36) = slot(36) + 2 ** (selection-index - 1)
        end-if
    end-perform.

loan-policy.
    *> offer/request/respond/accept/close/repay/run, actor, now, borrower exists/
    *> disabled, terms borrower/amount/installment/rate days/payment days/memo control,
    *> loan exists/status/borrower/lender/lender exists/disabled/credit,
    *> mine/full/recyclable, expected updated/updated/ready, principal/accrued/next due.
    *> Outputs 31-39: status/lender/borrower/draw amount/next due/principal/accrued/updated/overflow.
    if slot(3) < 1577836800 or slot(3) > 9007199254740991
        move 9 to result-code exit paragraph
    end-if
    if slot(1) < 0 or slot(1) > 6 move 1 to result-code exit paragraph end-if
    move 0 to slot(31) slot(32) slot(33) slot(34) slot(35) slot(36) slot(37) slot(38) slot(39)
    if slot(1) <= 2
        if slot(1) = 1
            if slot(6) not = slot(2) move 5 to result-code exit paragraph end-if
        else
            if slot(6) = slot(2) move 12 to result-code exit paragraph end-if
        end-if
        if slot(4) = 0 move 7 to result-code exit paragraph end-if
        if slot(5) = 1 move 6 to result-code exit paragraph end-if
        if slot(7) < 1 or slot(7) > 1000000000000000 or slot(8) < 1 or slot(8) > slot(7) or
            (slot(9) not = 1 and 7 and 30 and 365) or
            (slot(10) not = 1 and 7 and 30) or slot(11) = 1
            move 1 to result-code exit paragraph
        end-if
        if slot(1) = 2
            if slot(12) = 0 move 7 to result-code exit paragraph end-if
            if slot(13) not = 6 move 4 to result-code exit paragraph end-if
            if slot(14) not = slot(6) move 1 to result-code exit paragraph end-if
        end-if
        if slot(1) not = 1 and slot(19) >= 4 move 17 to result-code exit paragraph end-if
        if slot(1) not = 2 and slot(20) = 1 and slot(21) = 0
            move 8 to result-code exit paragraph
        end-if
        compute wide-value = slot(3) + slot(10) * 86400
        if wide-value > 9007199254740991 move 2 to result-code exit paragraph end-if
        move slot(6) to slot(33)
        if slot(1) = 1 move 6 to slot(31) else move slot(2) to slot(32) end-if
        move slot(3) to slot(38) exit paragraph
    end-if
    if slot(1) = 6 and slot(2) not = 0 move 5 to result-code exit paragraph end-if
    if slot(12) = 0 move 7 to result-code exit paragraph end-if
    move slot(13) to slot(31) move slot(15) to slot(32) move slot(14) to slot(33)
    move slot(27) to slot(35) move slot(25) to slot(36) move slot(26) to slot(37)
    move slot(3) to slot(38)
    evaluate slot(1)
        when 3
            if slot(2) not = slot(14) move 5 to result-code exit paragraph end-if
            if slot(13) not = 0 move 4 to result-code exit paragraph end-if
            if slot(16) = 0 move 7 to result-code exit paragraph end-if
            if slot(17) = 1 move 6 to result-code exit paragraph end-if
            compute wide-value = slot(3) + slot(10) * 86400
            if wide-value > 9007199254740991 move 1 to slot(39) end-if
            if slot(18) = 1
                move 1 to slot(31)
            else
                move 2 to slot(31) move slot(7) to slot(34) slot(36)
                move wide-value to slot(35) move slot(3) to slot(37)
            end-if
        when 4
            if slot(2) not = slot(14) and slot(2) not = slot(15)
                move 5 to result-code exit paragraph
            end-if
            if slot(13) not = 0 and 1 and 6 move 4 to result-code exit paragraph end-if
            if slot(2) = slot(14) and slot(13) = 0
                move 4 to slot(31)
            else move 5 to slot(31) end-if
        when 5
            if slot(2) not = slot(14) move 5 to result-code end-if
        when 6
            if slot(2) not = 0 move 5 to result-code exit paragraph end-if
            if slot(22) not = slot(23) or slot(24) = 0 move 4 to result-code end-if
        when other move 1 to result-code
    end-evaluate.

loan-summary.
    *> One row: status, principal, principal/interest due, rate bps/days.
    *> Accumulator: principal, overdue, active, weighted numerator high/low.
    *> Numerator uses common rate-days denominator 15330 and base 10^15 parts.
    *> Finalize flag in slot 12; output optional percentage flag/binary64 in 13/14.
    move 0 to slot(13) slot(14)
    compute wide-value = slot(10) * 1000000000000000 + slot(11)
    if slot(1) = 2
        if slot(6) not = 1 and 7 and 30 and 365
            move 1 to result-code exit paragraph
        end-if
        add slot(2) to slot(7)
        compute slot(8) = slot(8) + slot(3) + slot(4)
        add 1 to slot(9)
        compute factor-value = 15330 / slot(6)
        compute wide-value = wide-value + slot(2) * slot(5) * factor-value
    end-if
    divide wide-value by 1000000000000000 giving quotient-value remainder remainder-value
    move quotient-value to slot(10) move remainder-value to slot(11)
    if slot(12) = 1 and slot(7) > 0
        compute denominator-value = slot(7) * 15330 * 100
        compute summary-decimal = wide-value * 365 / denominator-value
        move summary-decimal to summary-percent
        move summary-bits to byte-area(105:8)
        move 1 to slot(13)
    end-if.

lotto-transition.
    *> Inputs 1..23: action (1 create/2 buy/3 run), clock, actor, admin,
    *> exists, kind (0 simple/1 delayed/2 savings), title-valid, price,
    *> closes, rate, house, step, pool, escrow, interest, remaining,
    *> winner (0 none), count/draw-ticket, global escrow, actor/house cash,
    *> expected step, book count, recyclable row (one-based, 0 none).
    *> Inputs 25..56: tickets for all 32 members.
    *> Outputs 24: new actor ticket count; 57..64: amount, pool, escrow,
    *> interest, remaining, winner, next step, parties + interest flag.
    move 0 to slot(24)
    perform varying selection-index from 57 by 1 until selection-index > 64
        move 0 to slot(selection-index)
    end-perform
    if slot(2) < 1577836800 or slot(2) > 9007199254740991
        move 9 to result-code exit paragraph
    end-if
    if slot(1) = 1
        if slot(4) = 0 move 5 to result-code exit paragraph end-if
        if slot(22) = 16 and slot(23) = 0
            move 8 to result-code exit paragraph
        end-if
        if slot(6) < 0 or slot(6) > 2 or slot(7) = 0
            or slot(8) < 1 or slot(8) > 1000000000000000
            or slot(9) <= slot(2) or slot(9) > 9007199252148991
            or slot(10) < 0 or slot(10) > 10000
            or (slot(6) = 0 and slot(10) not = 0)
            move 1 to result-code
        end-if
        exit paragraph
    end-if
    if slot(1) = 3 and slot(3) not = 0
        move 5 to result-code exit paragraph
    end-if
    if slot(5) = 0 move 7 to result-code exit paragraph end-if
    move slot(13) to slot(58) move slot(14) to slot(59)
    move slot(15) to slot(60) move slot(16) to slot(61)
    move slot(17) to slot(62) move slot(12) to slot(63)
    evaluate slot(1)
        when 2
            if slot(3) = slot(11) move 5 to result-code exit paragraph end-if
            if slot(2) >= slot(9) or slot(12) not = 0
                move 4 to result-code exit paragraph
            end-if
            if slot(18) < 1 or slot(18) > 4294967295
                or slot(3) < 1 or slot(3) > 32
                move 1 to result-code exit paragraph
            end-if
            compute selection-index = 24 + slot(3)
            compute wide-value = slot(selection-index) + slot(18)
            if wide-value > 4294967295
                move 2 to result-code exit paragraph
            end-if
            move wide-value to slot(24)
            compute wide-value = slot(8) * slot(18)
            if wide-value > 1000000000000000
                move 2 to result-code exit paragraph
            end-if
            move wide-value to slot(57)
            compute wide-value = slot(13) + slot(57)
            if wide-value > 1000000000000000
                move 2 to result-code exit paragraph
            end-if
            move wide-value to slot(58)
            compute wide-value = slot(58) * slot(10) / 10000
            move wide-value to slot(60) slot(61)
            compute wide-value = slot(58) + slot(60)
            compute credit-value = slot(19) + slot(57)
            if wide-value > 1000000000000000 or credit-value > 9007199254740991
                move 2 to result-code exit paragraph
            end-if
            if slot(20) < slot(57) move 3 to result-code exit paragraph end-if
            compute slot(59) = slot(14) + slot(57)
            compute slot(64) = slot(3) * 256 + 255
        when 3
            compute wide-value = slot(9)
            if slot(6) not = 0 add 2592000 to wide-value end-if
            if slot(21) not = slot(12) or slot(12) = 35 or slot(2) < wide-value
                move 4 to result-code exit paragraph
            end-if
            move 0 to quotient-value
            perform varying selection-index from 25 by 1 until selection-index > 56
                add slot(selection-index) to quotient-value
            end-perform
            if (slot(12) = 0 and slot(18) >= function max(1, quotient-value))
                or (slot(12) not = 0 and slot(18) not = 0) or slot(18) < 0
                move 1 to result-code exit paragraph
            end-if
            if slot(12) = 0
                move slot(18) to remainder-value
                move 35 to slot(63)
                perform varying selection-index from 25 by 1 until selection-index > 56
                    if remainder-value < slot(selection-index)
                        compute slot(62) = selection-index - 24
                        move 1 to slot(63)
                        exit perform
                    end-if
                    subtract slot(selection-index) from remainder-value
                end-perform
            else
                compute slot(63) = slot(12) + 1
                if slot(17) = 0 exit paragraph end-if
                evaluate true
                    when slot(12) >= 1 and slot(12) <= 32
                        if slot(6) = 2
                            compute selection-index = slot(12) + 24
                            compute slot(57) = slot(selection-index) * slot(8)
                        else
                            if slot(12) = slot(17) move slot(13) to slot(57) end-if
                        end-if
                        compute slot(64) = 255 * 256 + slot(12)
                        subtract slot(57) from slot(59)
                    when slot(12) = 33
                        compute slot(57) = function min(slot(16), function max(0, slot(20)))
                        compute slot(64) = 65536 + slot(11) * 256 + slot(17)
                        subtract slot(57) from slot(61)
                    when slot(12) = 34
                        move slot(16) to slot(57)
                        compute slot(64) = 65536 + slot(17)
                        subtract slot(57) from slot(61)
                end-evaluate
            end-if
        when other move 1 to result-code
    end-evaluate.

lotto-projection.
    *> Inputs: kind, closes, step, now, tickets for members 1..32 (5..36).
    *> Outputs 37..41: total tickets, due, status (0 open/1 waiting/
    *> 2 paying/3 settled), eligible for tick, terminal.
    move 0 to slot(37) slot(39) slot(40) slot(41)
    perform varying selection-index from 5 by 1 until selection-index > 36
        add slot(selection-index) to slot(37)
    end-perform
    move slot(2) to slot(38)
    if slot(1) not = 0 add 2592000 to slot(38) end-if
    evaluate true
        when slot(3) = 35 move 3 to slot(39) move 1 to slot(41)
        when slot(3) > 0 move 2 to slot(39)
        when slot(4) >= slot(2) move 1 to slot(39)
    end-evaluate
    if slot(3) < 35 and slot(4) >= slot(38) move 1 to slot(40) end-if
    *> Optional draw input 42; output 43 identifies its owner, zero out of range.
    move 0 to slot(43)
    if slot(42) >= 0 and slot(42) < slot(37)
        move slot(42) to remainder-value
        perform varying selection-index from 5 by 1 until selection-index > 36
            if remainder-value < slot(selection-index)
                compute slot(43) = selection-index - 4 exit perform
            end-if
            subtract slot(selection-index) from remainder-value
        end-perform
    end-if.

lotto-book.
    *> Inputs: count then up to 16 settlement steps; output 18 is first
    *> completed row (one-based), zero if no recyclable row exists.
    move 0 to slot(18)
    if slot(1) < 0 or slot(1) > 16 move 1 to result-code exit paragraph end-if
    perform varying selection-index from 2 by 1 until selection-index > slot(1) + 1
        if slot(selection-index) = 35
            compute slot(18) = selection-index - 1 exit perform
        end-if
    end-perform.

lotto-invariants.
    *> Inputs 1..16: id, sequence, duplicate, house exists, kind, price,
    *> rate, closes, step, pool, escrow, interest, remaining, winner,
    *> member-presence bitmap, accumulated escrow; 17..48: member tickets.
    *> Output 49: accumulated escrow. Invalid facts return 1 (corrupt state).
    *> Final call: input 51 = 1 compares accumulated escrow (16) to global (50).
    if slot(51) = 1
        if slot(16) < 0 or slot(16) not = slot(50) move 1 to result-code end-if
        move slot(16) to slot(49) exit paragraph
    end-if
    if slot(1) < 1 or slot(1) > slot(2) or slot(3) not = 0 or slot(4) = 0
        or slot(5) < 0 or slot(5) > 2 or slot(9) < 0 or slot(9) > 35
        or slot(6) < 1 or slot(6) > 1000000000000000
        or slot(7) < 0 or slot(7) > 10000 or slot(8) < 0 or slot(8) > 9007199252148991
        or (slot(5) = 0 and slot(7) not = 0)
        or slot(10) < 0 or slot(10) > 1000000000000000
        or slot(11) < 0 or slot(11) > slot(10)
        or slot(13) < 0 or slot(13) > slot(12)
        or slot(14) < 0 or slot(14) > 32
        or (slot(9) = 35 and (slot(11) not = 0 or slot(13) not = 0))
        or (slot(9) = 0 and slot(14) not = 0)
        or (slot(9) <= 33 and slot(13) not = slot(12))
        move 1 to result-code exit paragraph
    end-if
    move 0 to quotient-value debit-value
    move 1 to factor-value
    perform varying selection-index from 17 by 1 until selection-index > 48
        if slot(selection-index) < 0 or slot(selection-index) > 4294967295
            move 1 to result-code exit paragraph
        end-if
        add slot(selection-index) to quotient-value
        if slot(selection-index) > 0
            compute remainder-value = function mod(function integer(slot(15) / factor-value), 2)
            if remainder-value = 0 move 1 to result-code exit paragraph end-if
        end-if
        if slot(5) = 2 and slot(9) > selection-index - 16
            compute debit-value = debit-value + slot(selection-index) * slot(6)
        end-if
        multiply 2 by factor-value
    end-perform
    compute wide-value = quotient-value * slot(6)
    compute credit-value = slot(10) * slot(7) / 10000
    if wide-value not = slot(10) or credit-value not = slot(12)
        or (slot(9) > 0 and quotient-value > 0 and slot(14) = 0)
        move 1 to result-code exit paragraph
    end-if
    if slot(14) > 0
        compute selection-index = slot(14) + 16
        if slot(selection-index) = 0 move 1 to result-code exit paragraph end-if
        if slot(5) not = 2 and slot(9) > slot(14) move slot(10) to debit-value end-if
    end-if
    compute wide-value = slot(10) - debit-value
    if wide-value not = slot(11) move 1 to result-code exit paragraph end-if
    compute wide-value = slot(16) + slot(11)
    if wide-value < 0 or wide-value > 9223372036854775807
        move 1 to result-code exit paragraph
    end-if
    move wide-value to slot(49).

reform-policy.
    *> Inputs: requested decimals/power, current decimals, epoch count,
    *> current epoch/sequence, expected epoch/sequence, epoch capacity.
    *> Outputs 10..12: exponent, next epoch, new NC scale.
    if slot(5) not = slot(7) or slot(6) not = slot(8)
        move 4 to result-code exit paragraph
    end-if
    if slot(1) < 0 or slot(1) > 8 or slot(2) < -12 or slot(2) > 12
        or (slot(1) = slot(3) and slot(2) = 0)
        move 1 to result-code exit paragraph
    end-if
    if slot(4) >= slot(9) move 2 to result-code exit paragraph end-if
    compute slot(10) = slot(1) - slot(3) - slot(2)
    compute slot(11) = slot(5) + 1
    compute slot(12) = 10 ** slot(1).

reform-lotto-consistency.
    *> Inputs: converted pool, converted promised interest, rate.
    *> Exact conversion must preserve the original rounded obligation.
    compute wide-value = slot(1) * slot(3) / 10000
    if wide-value not = slot(2) move 4 to result-code end-if.

reform-field.
    *> Inputs: value, old/new decimals, power, field kind; output 6.
    *> 0 gift target, 1 net gifts, 2 art price, 3 lotto money,
    *> 4 global escrow, 5 grant, 6 wallet/issuance, 7 listing/offer,
    *> 8 quote coins, 9 quote price, 10 loan terms, 11 loan balances.
    if slot(5) < 0 or slot(5) > 11 move 1 to result-code exit paragraph end-if
    compute exponent-value = slot(3) - slot(2) - slot(4)
    if slot(5) = 9 move slot(4) to exponent-value end-if
    if slot(5) = 1 or slot(5) = 4 or slot(5) = 6 or slot(5) = 11
        move 9007199254740991 to slot(3)
    else
        move 1000000000000000 to slot(3)
    end-if
    move exponent-value to slot(2)
    perform exact-rescale
    if result-code = 0 move slot(4) to slot(6) end-if.

account-posting.
    *> Inputs: from, to, amount, correction, USD, source exists/disabled,
    *> target exists/disabled, source/target balances. Outputs 7/8: balances.
    if slot(1) = slot(2) or slot(3) < 1 or slot(3) > 1000000000000000
        move 1 to result-code exit paragraph
    end-if
    if slot(1) not = 0
        if slot(6) = 0 move 7 to result-code exit paragraph end-if
        if slot(4) = 0 and slot(7) = 1 move 6 to result-code exit paragraph end-if
    end-if
    if slot(2) not = 0
        if slot(8) = 0 move 7 to result-code exit paragraph end-if
        if slot(4) = 0 and slot(9) = 1 move 6 to result-code exit paragraph end-if
    end-if
    move slot(4) to slot(12) move slot(5) to slot(13)
    if slot(1) = 0 move 1 to slot(4) else move 0 to slot(4) end-if
    move slot(10) to slot(1) move slot(11) to slot(2)
    move slot(12) to slot(5) move slot(13) to slot(6)
    perform posting-plan.

correction-policy.
    *> Inputs: action (0 refund/1 reverse/2 room), original exists, actor,
    *> Nana, original from/to/amount, reverses, USD, loan, lotto, quote,
    *> art, refunded units, reversed, requested units, count/capacity, row exists.
    *> Outputs 20..23: payer, payee, original units, correction-overdraft flag.
    if slot(1) not = 2
        if slot(2) = 0 move 7 to result-code exit paragraph end-if
        evaluate slot(1)
            when 0
                if slot(8) = 1 or slot(7) = 0 or slot(9) = 1
                    or slot(5) = 0 or slot(6) = 0 or slot(10) = 1
                    or slot(11) = 1 or slot(12) = 1 or slot(13) = 1
                    move 5 to result-code exit paragraph
                end-if
                if slot(3) not = slot(6) and slot(4) = 0
                    move 5 to result-code exit paragraph
                end-if
                compute wide-value = slot(7) - slot(14)
                if slot(16) <= 0 or slot(16) > wide-value
                    move 4 to result-code exit paragraph
                end-if
                move slot(16) to slot(22) move 0 to slot(23)
            when 1
                if slot(7) = 0 or slot(10) = 1 or slot(11) = 1
                    move 5 to result-code exit paragraph
                end-if
                if slot(4) = 0 and (slot(6) not = slot(3) or slot(5) = 0
                    or slot(9) = 1 or slot(12) = 1)
                    move 5 to result-code exit paragraph
                end-if
                if slot(14) not = 0 or slot(15) = 1 or slot(8) = 1 or slot(13) = 1
                    move 4 to result-code exit paragraph
                end-if
                move slot(7) to slot(22) move slot(4) to slot(23)
            when other move 1 to result-code exit paragraph
        end-evaluate
        move slot(6) to slot(20) move slot(5) to slot(21)
    end-if
    if slot(17) = slot(18) and slot(19) = 0 move 8 to result-code end-if.

historical-amount.
    *> Inputs 1..5: original units, USD, original epoch, epoch count, limit;
    *> 6..37: epoch exponents (including the original epoch). Outputs 38/39:
    *> converted units, sum of subsequent exponents. USD is never reformed.
    move 0 to slot(39)
    if slot(2) = 1 move slot(1) to slot(38) exit paragraph end-if
    if slot(4) < 1 or slot(4) > 32 or slot(3) < 0 or slot(3) >= slot(4)
        move 1 to result-code exit paragraph
    end-if
    compute selection-index = slot(3) + 7
    perform until selection-index > slot(4) + 5
        add slot(selection-index) to slot(39)
        add 1 to selection-index
    end-perform
    move slot(39) to slot(2) move slot(5) to slot(3)
    perform exact-rescale
    if result-code = 0 move slot(4) to slot(38) end-if.

member-update-policy.
    *> Exists, bad bio, current verifier, new verifier present, self,
    *> role/status present, administrator, name blank/control; Mastodon
    *> empty/user empty/host empty/extra @/host dot/bad text; verifier valid,
    *> current Nana/disabled, demote or disable, active Nana count.
    if slot(1) = 0 move 7 to result-code exit paragraph end-if
    if slot(2) = 1 or (slot(3) = 0 and slot(4) = 1)
        move 1 to result-code exit paragraph
    end-if
    if (slot(5) = 0 or slot(6) = 1 or slot(7) = 1) and slot(8) = 0
        move 5 to result-code exit paragraph
    end-if
    if slot(9) = 1 or slot(10) = 1
        move 1 to result-code exit paragraph
    end-if
    if slot(11) = 0 and (slot(12) = 1 or slot(13) = 1 or slot(14) = 1
        or slot(15) = 0 or slot(16) = 1)
        move 1 to result-code exit paragraph
    end-if
    if slot(4) = 1 and slot(17) = 0 move 1 to result-code exit paragraph end-if
    if slot(18) = 1 and slot(19) = 0 and slot(20) = 1 and slot(21) = 1
        move 5 to result-code
    end-if.

member-import-policy.
    *> Action 0=credential setup, 1=token member. Administrator, self,
    *> target exists/verifier present, name blank/control, new verifier valid,
    *> count/capacity, duplicate username/name/hash, zero token hash.
    evaluate slot(1)
        when 0
            if slot(3) = 0 and slot(2) = 0 move 5 to result-code exit paragraph end-if
            if slot(4) = 0 move 7 to result-code exit paragraph end-if
            if slot(5) = 1 move 4 to result-code exit paragraph end-if
            if slot(6) = 1 or slot(7) = 1 or slot(8) = 0
                move 1 to result-code exit paragraph
            end-if
            if slot(11) = 1 move 4 to result-code end-if
        when 1
            if slot(2) = 0 move 5 to result-code exit paragraph end-if
            if slot(6) = 1 or slot(12) = 1 move 1 to result-code exit paragraph end-if
            if slot(9) >= slot(10) move 8 to result-code exit paragraph end-if
            if slot(11) = 1 move 4 to result-code end-if
        when other move 1 to result-code
    end-evaluate.

identity-transition.
    *> Action: provision/human/bot/token member/update/full key/read key/setup/config.
    *> Count, requested/current role, current disabled, role/status present,
    *> requested disabled, verifier present, zero key, timestamp, household blank,
    *> current/new/default settlement, grant. Outputs 33..42:
    *> id, role, kind, disabled, revoke keys, key timestamp, full key,
    *> default household, settlement, grant cash leg present.
    if slot(1) < 0 or slot(1) > 8 move 1 to result-code exit paragraph end-if
    move 0 to slot(33) slot(34) slot(35) slot(36) slot(37) slot(38)
        slot(39) slot(40) slot(42)
    move slot(13) to slot(41)
    evaluate slot(1)
        when 0 when 1 when 2 when 3
            if slot(2) < 0 or slot(2) >= 32 move 8 to result-code exit paragraph end-if
            compute slot(33) = slot(2) + 1
            move slot(3) to slot(34)
            if slot(1) = 0 or (slot(1) = 3 and slot(33) = 1)
                move 1 to slot(34)
            end-if
            if slot(1) = 2 move 0 to slot(34) move 1 to slot(35) end-if
            if slot(1) = 3 and slot(33) not = 1 move 0 to slot(34) end-if
            if (slot(1) = 1 or slot(1) = 2) and slot(16) > 0 move 1 to slot(42) end-if
        when 4
            move slot(4) to slot(34)
            if slot(6) = 1 move slot(3) to slot(34) end-if
            move slot(5) to slot(36)
            if slot(7) = 1 move slot(8) to slot(36) end-if
            move slot(9) to slot(37)
        when 5 when 6
            if slot(10) = 0 move slot(11) to slot(38) end-if
            if slot(1) = 5 move 1 to slot(39) end-if
        when 7
            move slot(12) to slot(40)
        when 8
            if slot(14) >= 0
                move slot(14) to slot(41)
                if slot(14) = 0 move slot(15) to slot(41) end-if
            end-if
    end-evaluate.

actor-policy.
    *> member count, actor id, exists, disabled, bootstrap command.
    if slot(1) = 0
        if slot(2) not = 1 or slot(5) = 0 move 5 to result-code end-if
    else
        if slot(3) = 0 move 7 to result-code
        else if slot(4) = 1 move 6 to result-code end-if end-if
    end-if.

new-member-policy.
    *> Username blank/control, display blank/control, verifier valid,
    *> member count, capacity, duplicate username. Error order is contractual.
    if slot(1) = 1 or slot(2) = 1 or slot(3) = 1 or slot(4) = 1 or slot(5) = 0
        move 1 to result-code
    else
        if slot(6) >= slot(7) move 8 to result-code
        else if slot(8) = 1 move 4 to result-code end-if end-if
    end-if.

household-statistics.
    *> Selectors and binary64 display aggregates; money remains exact elsewhere.
    move 0 to slot(17) slot(18) slot(19) slot(20)
    evaluate slot(1)
        when 0
            *> Usable retained-year row: USD/created/now/reverse/corrected.
            compute wide-value = function max(0, slot(4) - 31536000)
            if slot(2) = 0 and slot(3) >= wide-value and slot(5) = 0 and slot(6) = 0
                move 1 to slot(17) end-if
        when 1
            *> Eligible employment member: disabled/Nana.
            if slot(2) = 0 and slot(3) = 0 move 1 to slot(17) end-if
        when 2
            *> Labor row: payee/member/amount/kind (1 labor).
            if slot(2) = slot(3) and slot(4) > 0 and slot(5) = 1 move 1 to slot(17) end-if
        when 3
            *> Good sale: kind (2 good), thing/subject, quantity/amount.
            if slot(2) = 2 and slot(3) = slot(4) and slot(5) > 0 and slot(6) > 0
                move 1 to slot(17) end-if
        when 4
            *> Inflation fold: conversion flags, latest amount/quantity, prior
            *> amount/quantity, incoming binary64 total/count. Outputs total/count.
            move byte-area(57:8) to stats-total-bits
            move slot(9) to slot(18)
            if slot(2) = 1 and slot(3) = 1 and slot(6) > 0
                if slot(5) <= 0 or slot(7) <= 0 move 1 to result-code exit paragraph end-if
                move slot(4) to stats-first move slot(5) to stats-second
                compute stats-first = stats-first / stats-second
                move slot(6) to stats-second
                compute stats-second = stats-second / slot(7)
                compute stats-total = stats-total + (stats-first / stats-second - 1) * 100
                add 1 to slot(18)
            end-if
            move stats-total-bits to byte-area(129:8)
        when 5
            *> Employment fold: eligible/labor-found, eligible/employed counts.
            move slot(4) to slot(17) move slot(5) to slot(18)
            if slot(2) = 1
                add 1 to slot(17)
                if slot(3) = 1 add 1 to slot(18) end-if
            end-if
        when 6
            *> Finalize: inflation total/count, eligible/employed counts.
            if slot(3) > 0
                move byte-area(9:8) to stats-total-bits
                compute stats-total = stats-total / slot(3)
                move 1 to slot(17) move stats-total-bits to byte-area(137:8)
            end-if
            if slot(4) > 0
                compute stats-total = slot(5) * 100 / slot(4)
                move 1 to slot(19) move stats-total-bits to byte-area(153:8)
            end-if
        when 7
            *> Filled quote selection: filled, coin/cash corrections, current
            *> present/time, candidate time/rate. Later rows win equal times.
            if slot(2) = 1 and slot(3) = 0 and slot(4) = 0
                and (slot(5) = 0 or slot(7) >= slot(6)) move 1 to slot(17) end-if
        when 8
            *> Coin candidate: USD/quote/reverse/amount/corrected,
            *> current quote present/time, row time. Output 18 stop selection.
            if slot(2) = 0 and slot(3) = 1 and slot(4) = 0 and slot(5) > 0 and slot(6) = 0
                move 1 to slot(17)
                if slot(7) = 1 and slot(8) > slot(9) move 1 to slot(18) end-if
            end-if
        when 9
            *> Matching cash candidate: USD/quote match/reverse/amount/corrected.
            if slot(2) = 1 and slot(3) = 1 and slot(4) = 0 and slot(5) > 0 and slot(6) = 0
                move 1 to slot(17) end-if
        when 10
            *> Paired exchange: conversion valid, current NC/cash, decimals.
            if slot(2) = 1 and slot(3) > 0
                if slot(5) < 0 or slot(5) > 8 move 1 to result-code exit paragraph end-if
                move slot(4) to stats-first
                compute stats-total = stats-first * (10 ** slot(5)) / slot(3) / 100
                move 1 to slot(17) move stats-total-bits to byte-area(137:8)
            end-if
        when 11
            *> Quote cents/coin converted to display dollars/coin.
            compute stats-total = slot(2) / 100
            move 1 to slot(17) move stats-total-bits to byte-area(137:8)
        when 12
            *> Retained sale fold in descending ledger order: eligible, row
            *> index, latest/prior indices and count. Select exactly two rows.
            if slot(6) < 0 or slot(6) > 2 or slot(3) < 0
                move 1 to result-code exit paragraph end-if
            move slot(4) to slot(17) move slot(5) to slot(18)
            move slot(6) to slot(19)
            if slot(2) = 1 and slot(6) < 2
                if slot(6) = 0 move slot(3) to slot(17)
                else move slot(3) to slot(18) end-if
                add 1 to slot(19)
            end-if
        when other move 1 to result-code
    end-evaluate.

market-view.
    *> Action 0: wanted status (-1 all; 0 active, 1 sold, 2 cancelled), row status.
    *> Action 1: count (0..20), triples created time/id/eligible. Outputs 62..64
    *> local index/time/id. Action 2 merges index/time/id pairs; outputs 17..19.
    evaluate slot(1)
        when 0
            if slot(2) < -1 or slot(2) > 2 or slot(3) < 0 or slot(3) > 2
                move 1 to result-code exit paragraph end-if
            move 0 to slot(17)
            if slot(2) = -1 or slot(2) = slot(3) move 1 to slot(17) end-if
        when 1
            if slot(2) < 0 or slot(2) > 20 move 1 to result-code exit paragraph end-if
            move -1 to selection-best move 0 to selection-time selection-key
            perform varying selection-index from 1 by 1 until selection-index > slot(2)
                compute inbound-index = 3 + (selection-index - 1) * 3
                if slot(inbound-index + 2) = 1 and (selection-best < 0
                    or slot(inbound-index) > selection-time
                    or (slot(inbound-index) = selection-time and slot(inbound-index + 1) < selection-key))
                    compute selection-best = selection-index - 1
                    move slot(inbound-index) to selection-time
                    move slot(inbound-index + 1) to selection-key
                end-if
            end-perform
            move selection-best to slot(62) move selection-time to slot(63) move selection-key to slot(64)
        when 2
            move slot(2) to slot(17) move slot(3) to slot(18) move slot(4) to slot(19)
            if slot(5) >= 0 and (slot(2) < 0 or slot(6) > slot(3)
                or (slot(6) = slot(3) and slot(7) < slot(4)))
                move slot(5) to slot(17) move slot(6) to slot(18) move slot(7) to slot(19)
            end-if
        when 3
            *> Disabled profile viewer.
            if slot(2) = 1 move 5 to result-code end-if
        when 4
            *> Offerer/subject/projected offer status; only OPEN belongs in profile.
            move 0 to slot(17)
            if slot(2) = slot(3) and slot(4) = 0 move 1 to slot(17) end-if
        when 5
            *> Active listing count/status fold.
            move slot(2) to slot(17)
            if slot(3) = 0 add 1 to slot(17) end-if
        when 6
            *> Account viewer: Nana, actor, subject, disabled.
            if slot(2) = 0 and slot(3) not = slot(4) and slot(5) = 1
                move 5 to result-code end-if
        when 7
            *> Member id/found/USD, NC/USD issuance, NC/USD member, disabled.
            *> Outputs 17 balance, 18 issuance flag, 19 disabled.
            move 0 to slot(18) slot(19)
            if slot(2) = 0
                move 1 to slot(18)
                move slot(5) to slot(17)
                if slot(4) = 1 move slot(6) to slot(17) end-if
            else
                if slot(3) = 0 move 7 to result-code exit paragraph end-if
                move slot(7) to slot(17) move slot(9) to slot(19)
                if slot(4) = 1 move slot(8) to slot(17) end-if
            end-if
        when 8
            *> Fulfillment viewer: Nana, actor, provider, recipient.
            move 0 to slot(17)
            if slot(2) = 1 or slot(3) = slot(4) or slot(3) = slot(5)
                move 1 to slot(17) end-if
        when 9
            *> Private transaction visibility: amount, actor, payer, payee.
            move 0 to slot(17)
            if slot(2) not = 0 or slot(3) = slot(4) or slot(3) = slot(5)
                move 1 to slot(17) end-if
        when 10
            *> Account transaction matching: USD/wanted USD, payer/payee/subject.
            move 0 to slot(17)
            if slot(2) = slot(3) and (slot(4) = slot(6) or slot(5) = slot(6))
                move 1 to slot(17) end-if
        when 11
            *> Screen mail: amount/USD/blank/reverse/loan/lotto, read, actor/payer/payee.
            if slot(2) not = 0 or slot(3) = 1 or slot(4) = 1 or slot(5) = 1
                or slot(6) = 1 or slot(7) = 1 move 1 to result-code exit paragraph end-if
            if (slot(8) = 1 and slot(9) not = slot(11))
                or (slot(8) = 0 and slot(9) not = slot(10)) move 5 to result-code end-if
        when 12
            *> Newest-first book fold: count<=30, best index/id, then index/id pairs.
            *> Zero id marks a consumed row. Outputs follow the complete input scan.
            if slot(2) < 0 or slot(2) > 30 move 1 to result-code exit paragraph end-if
            move slot(3) to selection-best move slot(4) to selection-key
            perform varying selection-index from 1 by 1 until selection-index > slot(2)
                compute inbound-index = 5 + (selection-index - 1) * 2
                if slot(inbound-index + 1) > 0 and
                    (selection-best < 0 or slot(inbound-index + 1) > selection-key)
                    move slot(inbound-index) to selection-best
                    move slot(inbound-index + 1) to selection-key
                end-if
            end-perform
            move selection-best to slot(17) move selection-key to slot(18)
        when other move 1 to result-code
    end-evaluate.

bank-invariants.
    *> Action 0 currency header; 1 loan; 2 quote; 3 initial NC/USD sums;
    *> 4 member sums; 5 zero sums; 6 ledger header; 7 epoch; 8 correction;
    *> 9 retained transaction metadata; 10 history tail; 11 audit sequence;
    *> 12 audit tail; 13 table capacity; 14 gift; 15 art; 16 fulfillment;
    *> 17 fulfillment update; 19 wide flow; 20 final epoch. Corrupt returns 19.
    evaluate slot(1)
        when 0
            *> Decimals, epoch, epoch count, corrections/count limit.
            if slot(2) > 8 or slot(3) > 9007199254740991
                or slot(4) not = slot(3) + 1 or slot(5) > slot(6)
                move 19 to result-code end-if
        when 1
            *> Loan id, state sequence, duplicate, lender/borrower/status,
            *> lender/borrower exist, amount/installment/rate days/payment days,
            *> principal/interest/principal due/interest due/remainder,
            *> accrued/update/last time/next due.
            if slot(2) < 1 or slot(2) > slot(3) or slot(4) = 1 or slot(5) = slot(6)
                or (slot(5) = 0 and slot(7) not = 6 and slot(7) not = 5)
                or (slot(5) not = 0 and slot(8) = 0) or slot(9) = 0
                or (slot(7) = 6 and (slot(5) not = 0 or slot(14) not = 0 or slot(15) not = 0))
                or slot(10) < 1 or slot(10) > 1000000000000000
                or slot(11) < 1 or slot(11) > slot(10)
                or (slot(12) not = 1 and slot(12) not = 7 and slot(12) not = 30 and slot(12) not = 365)
                or (slot(13) not = 1 and slot(13) not = 7 and slot(13) not = 30)
                or slot(14) < 0 or slot(14) > slot(10) or slot(15) < 0
                or slot(16) < 0 or slot(16) > slot(14) or slot(17) < 0 or slot(17) > slot(15)
                or slot(19) > slot(21) or slot(20) > slot(21)
                or (slot(7) = 2 and (slot(22) < 1577836800 or slot(22) > 9007199254740991))
                move 19 to result-code exit paragraph end-if
            compute wide-value = slot(14) + slot(15)
            compute denominator-value = 10000 * slot(12) * 86400
            if wide-value > 9007199254740991 or slot(18) >= denominator-value
                move 19 to result-code end-if
        when 2
            *> Quote scale, coins, cents per coin, decimals.
            if slot(5) < 0 or slot(5) > 8 or slot(2) not = 10 ** slot(5)
                or slot(3) < 1 or slot(3) > 1000000000000000
                or slot(4) < 1 or slot(4) > 1000000000000000
                move 19 to result-code exit paragraph end-if
            compute wide-value = slot(3) * slot(4)
            divide wide-value by slot(2) giving quotient-value remainder remainder-value
            if remainder-value not = 0 or quotient-value < 1 or quotient-value > 1000000000000000
                move 19 to result-code end-if
        when 3
            *> Issuance, lotto escrow, USD issuance. Output 33/34 NC/USD sum.
            compute wide-value = slot(2) + slot(3)
            if wide-value < min-i64 or wide-value > 9223372036854775807
                move 19 to result-code exit paragraph end-if
            move wide-value to slot(33) move slot(4) to slot(34)
        when 4
            *> Running NC/USD, member NC/USD. Output 33/34 new sums.
            if slot(4) < -9007199254740991 or slot(4) > 9007199254740991
                or slot(5) < -9007199254740991 or slot(5) > 9007199254740991
                move 19 to result-code exit paragraph end-if
            compute wide-value = slot(2) + slot(4)
            compute debit-value = slot(3) + slot(5)
            if wide-value < min-i64 or wide-value > 9223372036854775807
                or debit-value < min-i64 or debit-value > 9223372036854775807
                move 19 to result-code exit paragraph end-if
            move wide-value to slot(33) move debit-value to slot(34)
        when 5
            if slot(2) not = 0 or slot(3) not = 0 move 19 to result-code end-if
        when 6
            *> Transactions, sequence, epoch, epoch count, correction count/cap,
            *> audit count/cap, history count/cap, archived tx/floor/audit seq/first page.
            if slot(2) > slot(3) * 2 or slot(2) > 18014398509481982 or slot(4) >= 32
                or slot(5) not = slot(4) + 1 or slot(6) > slot(7)
                or slot(8) > slot(9) or slot(10) > slot(11)
                or slot(12) > slot(2) or slot(13) > slot(12) or slot(14) > slot(3)
                or (slot(15) = 0 and slot(10) not = function min(slot(2), slot(11)))
                move 19 to result-code end-if
        when 7
            *> Epoch index, decimals, exponent, previous decimals.
            if slot(3) > 8 or (slot(2) = 0 and (slot(3) not = 4 or slot(4) not = 0))
                or (slot(2) > 0 and (slot(3) - slot(5) - slot(4) < -12
                    or slot(3) - slot(5) - slot(4) > 12)) move 19 to result-code end-if
        when 8
            *> Original id/units/refunded, full flag/id, duplicate, retained original
            *> found/reverse flag/amount.
            if slot(2) < 1 or slot(2) > 9007199254740991 or slot(3) < 0 or slot(3) > 1000000000000000
                or slot(4) < 0 or slot(4) > slot(3) or slot(7) = 1
                or (slot(5) = 1 and slot(4) not = slot(3)) or (slot(5) = 0 and slot(4) = slot(3))
                or (slot(5) = 1 and (slot(6) < 1 or slot(6) > 9007199254740991 or slot(6) = slot(2)))
                or (slot(8) = 1 and (slot(9) = 1 or slot(3) not = slot(10) or slot(4) > slot(10)
                    or (slot(5) = 1 and slot(4) not = slot(10)) or (slot(5) = 0 and slot(4) = slot(10))))
                move 19 to result-code end-if
        when 9
            *> Ordinal,total,floor,previous flag/value,epoch/current epoch,group/sequence,
            *> decimals,USD,epoch decimals,refund/original units,reverse flag.
            if slot(2) >= slot(3) or slot(2) < slot(4)
                or (slot(5) = 1 and slot(2) not = slot(6) + 1)
                or slot(7) > slot(8) or slot(9) < 1 or slot(9) > slot(10)
                or (slot(12) = 1 and slot(11) not = 2) or (slot(12) = 0 and slot(11) not = slot(13))
                or slot(14) < 0 or slot(14) > 1000000000000000 or slot(15) < 0 or slot(15) > 1000000000000000
                or (slot(16) = 1 and (slot(14) <= 0 or slot(15) <= 0 or slot(14) > slot(15)))
                move 19 to result-code end-if
        when 10
            *> Last ordinal present/value, transactions, first page.
            if (slot(2) = 1 and slot(3) + 1 not = slot(4))
                or (slot(4) > 0 and slot(2) = 0 and slot(5) = 0) move 19 to result-code end-if
        when 11
            *> Audit sequence/head, timestamp/last time, previous flag/sequence.
            if slot(2) > slot(3) or slot(4) > slot(5) move 19 to result-code exit paragraph end-if
            if slot(6) = 1
                compute wide-value = slot(7) + 1
                if function mod(slot(7), 8192) = 4096 compute wide-value = slot(7) + 4097 end-if
                if wide-value > 9007199254736895 or slot(2) not = wide-value move 19 to result-code end-if
            end-if
        when 12
            if slot(2) > 0 and slot(3) not = slot(2) move 19 to result-code end-if
        when 13
            if slot(2) > slot(3) move 19 to result-code end-if
        when 14
            *> Owner exists, id/sequence, received, target flag/value, title blank, duplicate.
            if slot(2) = 0 move 7 to result-code exit paragraph end-if
            if slot(3) < 1 or slot(3) > slot(4) or slot(5) < 0 or slot(5) > 9007199254740991
                or (slot(6) = 1 and (slot(7) < 1 or slot(7) > 1000000000000000))
                or slot(8) = 1 or slot(9) = 1 move 19 to result-code end-if
        when 15
            *> Owner/creator exist, id/sequence/revision, price flag/value,
            *> valid metadata, duplicate id, equipped, prior equipped owner count.
            if slot(2) = 0 or slot(3) = 0 move 7 to result-code exit paragraph end-if
            if slot(4) < 1 or slot(4) > slot(5) or slot(6) < slot(4) or slot(6) > slot(5)
                or (slot(7) = 1 and (slot(8) < 1 or slot(8) > 1000000000000000))
                or slot(9) = 0 or slot(10) = 1 or (slot(11) = 1 and slot(12) > 0)
                move 19 to result-code end-if
        when 16
            *> Payment id/transaction, to/provider/from/recipient, amount, USD/reverse,
            *> correction present, status reversed, sequence, member existence,
            *> duplicate transaction, last update present/status/current status.
            if slot(2) not = slot(3) or slot(4) not = slot(5) or slot(6) not = slot(7)
                or slot(8) < 1 or slot(8) > 1000000000000000 or slot(9) = 1 or slot(10) = 1
                or slot(11) not = slot(12) or slot(3) < 1 or slot(3) > slot(13)
                or slot(5) = slot(7) or slot(14) = 0 or slot(15) = 0 or slot(16) = 1
                or slot(17) = 0 or slot(18) not = slot(19) move 19 to result-code end-if
        when 17
            *> Update sequence, state sequence, actor exists.
            if slot(2) > slot(3) or slot(4) = 0 move 19 to result-code end-if
        when 19
            *> Wide value encodable, signed high/low base 10^15, tx count, flow index.
            if slot(2) = 0 move 19 to result-code exit paragraph end-if
            compute wide-value = slot(3) * 1000000000000000 + slot(4)
            if function abs(wide-value) > 288230376151711712000000000000000
                or (slot(6) = 19 and (wide-value < 0 or wide-value > slot(5))) move 19 to result-code end-if
        when 20
            *> Last epoch present/decimals, state decimals.
            if slot(2) = 0 or slot(3) not = slot(4) move 19 to result-code end-if
        when other move 1 to result-code
    end-evaluate.

audit-policy.
    *> Action 0=prepare: command tag provision/human/bot/token/update/setup/
    *> full key/read key/business; target/new member ids, role/present,
    *> disabled present/value, password/display present, cache count/capacity.
    *> Outputs 17..26 identity/member/name source/role present/value,
    *> disabled present/value, credentials changed, created, evict.
    *> Name source: 0 empty, 1 display, 2 username, 3 token name, 4 optional display.
    evaluate slot(1)
        when 0
            if slot(2) < 0 or slot(2) > 8 or slot(11) > slot(12) or slot(12) < 1
                move 1 to result-code exit paragraph end-if
            move 0 to slot(17) slot(18) slot(19) slot(20) slot(21)
                slot(22) slot(23) slot(24) slot(25) slot(26)
            if slot(11) = slot(12) move 1 to slot(26) end-if
            if slot(2) = 8 exit paragraph end-if
            move 1 to slot(17)
            move slot(3) to slot(18)
            evaluate slot(2)
                when 0 when 1 when 2
                    move slot(4) to slot(18)
                    move 1 to slot(19) slot(20) slot(24) slot(25)
                    move slot(5) to slot(21)
                    if slot(2) = 0 move 1 to slot(18) slot(21) end-if
                    if slot(2) = 2 move 0 to slot(21) end-if
                when 3
                    move slot(4) to slot(18)
                    move 3 to slot(19) move 1 to slot(24) slot(25)
                when 4
                    if slot(10) = 1 move 4 to slot(19) end-if
                    move slot(6) to slot(20) move slot(5) to slot(21)
                    move slot(7) to slot(22) move slot(8) to slot(23)
                    move slot(9) to slot(24)
                when 5 move 2 to slot(19) move 1 to slot(24)
                when 6 when 7 move 1 to slot(24)
            end-evaluate
        when 1
            *> Sequence, actor, identity, member, created, name/role/status present,
            *> credentials changed, credential-bearing business variant.
            if slot(2) < 1 or slot(2) > 9007199254740991 or slot(3) < 0 or slot(3) > 32
                move 19 to result-code exit paragraph end-if
            if slot(4) = 1
                if slot(5) < 1 or slot(5) > 32 move 19 to result-code exit paragraph end-if
                if slot(6) = 1 and (slot(7) = 0 or slot(9) = 1 or slot(10) = 0)
                    move 19 to result-code end-if
            else if slot(11) = 1 move 19 to result-code end-if end-if
        when other move 1 to result-code
    end-evaluate.

activity-projection.
    *> Action 0=event; tag 0 identity, 1/2 listings, 3 purchase, 4 offer,
    *> 5 create lotto, 6 run lotto, 7 request loan, 8 accept loan, 9 repay/run,
    *> 10/11 quote, 12 gift request, 13 art mint, 14 hidden command.
    *> Facts: exists, name/role/status present, member creation time, good deed,
    *> step/winner, loan status/update time, actor/subject/owner/other ids,
    *> amount present/value, rate/coins/APR/close, side/lotto kind, audit time.
    *> Outputs 33..50: kind/prefix/actor/other/subject, amount flag/value,
    *> rate/coin/APR/close flags, side, lotto flag/kind, rate/coins/APR/close.
    evaluate slot(1)
        when 0
            move 0 to slot(33) slot(34) slot(38) slot(40) slot(41)
                slot(42) slot(43) slot(44) slot(45)
            move slot(13) to slot(35)
            move -1 to slot(36)
            move slot(14) to slot(37)
            move slot(18) to slot(39)
            move slot(24) to slot(46)
            move slot(19) to slot(47)
            move slot(20) to slot(48)
            move slot(21) to slot(49)
            move slot(22) to slot(50)
            evaluate slot(2)
                when 0
                    if slot(3) = 1 and slot(4) = 1 and slot(5) = 1
                        and slot(6) = 0 and slot(7) = slot(26) and slot(29) = 1
                        move 1 to slot(33) move slot(15) to slot(35)
                    end-if
                when 1 when 2
                    move 2 to slot(33) move 1 to slot(34) slot(38)
                    move slot(23) to slot(44)
                    if slot(2) = 1 and slot(8) = 1 move 3 to slot(33) end-if
                when 3 when 4
                    if slot(3) = 1
                        move 4 to slot(33) move 1 to slot(34) slot(38)
                        move slot(16) to slot(36)
                        if slot(2) = 3 move slot(15) to slot(35) end-if
                        if slot(2) = 4 and slot(8) = 1 move 5 to slot(33) end-if
                    end-if
                when 5
                    move 6 to slot(33) move 2 to slot(34)
                    move 1 to slot(38) slot(43) slot(45)
                when 6
                    if slot(3) = 1 and slot(9) = 0 and slot(10) = 1
                        move 7 to slot(33) move 2 to slot(34) move 1 to slot(38)
                        move slot(15) to slot(35) move slot(16) to slot(36)
                    end-if
                when 7
                    move 8 to slot(33) move 3 to slot(34) move 1 to slot(38) slot(42)
                when 8
                    if slot(3) = 1
                        move 9 to slot(33) move 3 to slot(34) move 1 to slot(38) slot(42)
                        move slot(15) to slot(35) move slot(16) to slot(36)
                    end-if
                when 9
                    if slot(3) = 1 and slot(11) = 3 and slot(12) = slot(26)
                        and slot(27) = slot(28)
                        move 10 to slot(33) move 3 to slot(34) move 1 to slot(38)
                        move slot(15) to slot(35) move slot(16) to slot(36)
                    end-if
                when 10 when 11
                    if slot(2) = 10 or slot(3) = 1
                        compute slot(33) = slot(2) + 1
                        move 4 to slot(34) move 1 to slot(40) slot(41)
                        move slot(23) to slot(44)
                        if slot(2) = 11 move slot(16) to slot(36) end-if
                    end-if
                when 12
                    move 13 to slot(33) move 5 to slot(34) move slot(17) to slot(38)
                when 13
                    move 14 to slot(33) move 6 to slot(34)
                when 14 continue
                when other move 1 to result-code
            end-evaluate
        when 1
            *> Disabled, limit, after, row count/capacity, first exists/sequence,
            *> limit present. Outputs 33 limit, 34 truncated.
            if slot(2) = 1 move 5 to result-code exit paragraph end-if
            move 50 to slot(33)
            if slot(9) = 1
                if slot(3) < 1 or slot(3) > 50 move 1 to result-code exit paragraph end-if
                move slot(3) to slot(33)
            end-if
            move 0 to slot(34)
            compute wide-value = slot(4) + 1
            if function mod(slot(4), 8192) = 4096
                compute wide-value = slot(4) + 4097 end-if
            if slot(5) = slot(6) and slot(7) = 1 and wide-value <= 9007199254736895
                and slot(8) > wide-value move 1 to slot(34) end-if
        when 2
            *> Sequence, after. Output 33 visible.
            move 0 to slot(33)
            if slot(2) > slot(3) move 1 to slot(33) end-if
        when 3
            *> Self account, disabled actor.
            if slot(2) = 0 and slot(3) = 1 move 5 to result-code end-if
        when 4
            *> Up to 60 joined payment sequences, select the latest.
            if slot(2) < 0 or slot(2) > 60 move 1 to result-code exit paragraph end-if
            move 0 to selection-best selection-time
            perform varying selection-index from 1 by 1 until selection-index > slot(2)
                if selection-best = 0 or slot(selection-index + 2) > selection-time
                    move selection-index to selection-best
                    move slot(selection-index + 2) to selection-time
                end-if
            end-perform
            compute slot(63) = selection-best - 1
            move selection-time to slot(64)
        when other move 1 to result-code
    end-evaluate.

ledger-projection.
    *> Action 0=transaction: amount, reverse/listing/loan/lotto/quote flags,
    *> from/to. Outputs 17 kind, 18 reference, 19 debit, 20 credit, 21 public.
    *> 1=cursor: incarnation matches, upper/before/page,total/next/first,limit.
    *> 2=row: ordinal,before,upper,amount,actor present/id,from/to,
    *> account present/id,USD/account USD. 3=continuation, 4=retention,
    *> 5=archive insertion, 6=descending ordinal key, 7=circulation.
    evaluate slot(1)
        when 0
            move 5 to slot(17)
            evaluate true
                when slot(2) = 0 move 0 to slot(17)
                when slot(3) = 1 move 1 to slot(17)
                when slot(8) = 0 move 2 to slot(17)
                when slot(4) = 1 move 3 to slot(17)
                when slot(9) = 0 move 4 to slot(17)
            end-evaluate
            move 0 to slot(18) slot(21)
            evaluate true
                when slot(5) = 1 move 1 to slot(18)
                when slot(6) = 1 move 2 to slot(18)
                when slot(7) = 1 move 3 to slot(18)
            end-evaluate
            if slot(2) = min-i64 move 2 to result-code exit paragraph end-if
            compute slot(19) = 0 - slot(2)
            move slot(2) to slot(20)
            if slot(2) not = 0 move 1 to slot(21) end-if
        when 1
            if slot(2) = 0 or slot(3) > slot(6) or slot(4) > slot(3)
                or slot(5) > slot(7) or slot(5) < slot(8)
                move 18 to result-code exit paragraph
            end-if
            compute slot(17) = function min(100, function max(1, slot(9)))
        when 2
            move 0 to slot(17)
            if slot(2) < slot(3) and slot(2) < slot(4)
                and (slot(5) not = 0 or (slot(6) = 1
                    and (slot(8) = slot(7) or slot(9) = slot(7))))
                and (slot(10) = 0 or (slot(12) = slot(13)
                    and (slot(8) = slot(11) or slot(9) = slot(11))))
                move 1 to slot(17)
            end-if
        when 3
            move 0 to slot(17)
            if slot(2) > 0 and (slot(3) = slot(4) or slot(5) > slot(6))
                move 1 to slot(17)
            end-if
        when 4
            move 0 to slot(17)
            if slot(2) not = 0 or (slot(3) = 0 and slot(4) > slot(5))
                move 1 to slot(17)
            end-if
        when 5
            move 0 to slot(17)
            if slot(2) = 1
                if slot(3) < slot(4) move 1 to slot(17)
                else if slot(5) > slot(6) move 2 to slot(17) end-if end-if
            end-if
        when 6 when 7
            if slot(2) = min-i64 move 2 to result-code exit paragraph end-if
            compute slot(17) = 0 - slot(2)
        when 8
            *> Count, up to 60 ordinals. Output zero-based minimum index/value.
            if slot(2) < 0 or slot(2) > 60 move 1 to result-code exit paragraph end-if
            move 0 to selection-best selection-time
            perform varying selection-index from 1 by 1 until selection-index > slot(2)
                if selection-best = 0 or slot(selection-index + 2) < selection-time
                    move selection-index to selection-best
                    move slot(selection-index + 2) to selection-time
                end-if
            end-perform
            compute slot(63) = selection-best - 1
            move selection-time to slot(64)
        when 9
            *> Audit cursor: incarnation matches, page/next/first.
            if slot(2) = 0 or slot(3) > slot(4) or slot(3) < slot(5)
                move 18 to result-code end-if
        when 10
            *> Audit sequence, before.
            move 0 to slot(17)
            if slot(2) < slot(3) move 1 to slot(17) end-if
        when 11
            *> Audit next sequence, count, page/first. Fixed 16-row page.
            move 0 to slot(17)
            if slot(2) > 1 and (slot(3) = 16 or slot(4) > slot(5))
                move 1 to slot(17) end-if
        when 12
            *> Audit retained floor.
            move 0 to slot(17)
            if slot(2) > 0 move 1 to slot(17) end-if
        when 13
            *> Initial audit before value is head sequence plus one.
            compute wide-value = slot(2) + 1
            if wide-value > 9223372036854775807 move 2 to result-code
            else move wide-value to slot(17) end-if
        when other move 1 to result-code
    end-evaluate.

identity-text-policy.
    *> kind 0=name, 1=Mastodon. Name: blank/control facts.
    *> Mastodon: empty, user empty, host empty, extra @, host dot, bad Unicode/slash.
    evaluate slot(1)
        when 0
            if slot(2) = 1 or slot(3) = 1 move 1 to result-code end-if
        when 1
            if slot(2) = 0 and (slot(3) = 1 or slot(4) = 1 or slot(5) = 1
                or slot(6) = 0 or slot(7) = 1) move 1 to result-code end-if
        when 2
            *> Creation: display blank, bot, Nana, grant requested/configured.
            if slot(3) = 1 and slot(4) = 1 move 1 to result-code exit paragraph end-if
            move slot(2) to slot(8)
            move 0 to slot(9)
            if slot(5) = 1 move slot(6) to slot(9) end-if
        when 3
            *> Wire update guard: actor Nana, self, role/status present.
            if slot(2) = 0 and (slot(3) = 0 or slot(4) = 1 or slot(5) = 1)
                move 5 to result-code
            end-if
        when 4
            *> Free-text control fact: blank correction reasons remain allowed.
            if slot(2) = 1 move 1 to result-code end-if
        when 5
            *> Provisioning requires an empty current member table.
            if slot(2) not = 0 move 5 to result-code end-if
        when 6
            *> Parsed decimal quantity: whole/fraction/fraction digit count.
            if slot(2) < 0 or slot(2) > 4294967295 or slot(3) < 0
                or slot(4) < 0 or slot(4) > 3
                move 1 to result-code exit paragraph end-if
            if slot(3) >= 10 ** slot(4)
                move 1 to result-code exit paragraph end-if
            compute wide-value = slot(2) * 1000 + slot(3) * (10 ** (3 - slot(4)))
            if wide-value < 1 or wide-value > 1000000000
                move 1 to result-code
            else move wide-value to slot(17) end-if
        when other move 1 to result-code
    end-evaluate.

transfer-policy.
    *> Inputs 1..11 match account-posting. Metadata 14..24: classified,
    *> recipient Nana, blank/control memo, quantity, kind (1 labor), unit,
    *> thing specified/exists/kind/unit. Outputs 7/8: posting balances.
    if slot(8) = 0 move 7 to result-code exit paragraph end-if
    if slot(14) = 0 and slot(3) = 0
        if slot(1) = slot(2) move 12 to result-code exit paragraph end-if
        if slot(9) = 1 move 6 to result-code exit paragraph end-if
        if slot(16) = 1 or slot(17) = 1 move 1 to result-code exit paragraph end-if
        move slot(10) to slot(7) move slot(11) to slot(8)
        exit paragraph
    end-if
    perform account-posting
    if result-code not = 0 exit paragraph end-if
    if slot(14) = 1
        if slot(18) < 1 or slot(18) > 1000000000
            move 1 to result-code exit paragraph
        end-if
        if slot(19) = 1 and slot(15) = 1 move 5 to result-code exit paragraph end-if
        if slot(21) = 1
            if slot(22) = 0 move 7 to result-code exit paragraph end-if
            if slot(19) not = slot(23) or slot(20) not = slot(24)
                move 1 to result-code
            end-if
        end-if
    end-if.
