       >>source format free
identification division.
program-id. NCBANK.
data division.
working-storage section.
*> Compile-time constants: no mutable storage and no retained call state.
78 op-posting-plan value 1.
78 op-exact-rescale value 2.
78 op-exchange-cents value 3.
78 op-annual-rate value 4.
78 op-accrue-interest value 5.
78 op-settle-loan value 6.
78 op-reform-loan-fraction value 7.
78 op-loan-posting-plan value 8.
78 op-identity-query-policy value 9.
78 op-administrator-policy value 10.
78 op-protect-last-nana value 11.
78 op-key-policy value 12.
78 op-member-grant value 13.
78 op-configuration-policy value 14.
78 op-loan-policy value 15.
78 op-loan-projection value 16.
78 op-member-update-policy value 17.
78 op-loan-book value 18.
78 op-loan-summary value 19.
78 op-fulfillment-transition value 20.
78 op-offer-projection value 21.
78 op-offer-transition value 22.
78 op-member-import-policy value 23.
78 op-oldest-eligible-record value 24.
78 op-reversal-offer-links value 25.
78 op-listing-transition value 26.
78 op-first-eligible-record value 27.
78 op-recycling-policy value 28.
78 op-identity-transition value 29.
78 op-classify-flows value 30.
78 op-accumulate-flows value 31.
78 op-lotto-transition value 32.
78 op-lotto-projection value 33.
78 op-lotto-book value 34.
78 op-lotto-invariants value 35.
78 op-reform-policy value 36.
78 op-reform-lotto-consistency value 37.
78 op-reform-field value 38.
78 op-account-posting value 39.
78 op-correction-plan value 40.
78 op-correction-policy value 41.
78 op-historical-amount value 42.
78 op-transfer-policy value 43.
78 op-actor-policy value 44.
78 op-fulfillment-capacity value 45.
78 op-fulfillment-creation value 46.
78 op-fulfillment-recycling value 47.
78 op-new-member-policy value 48.
78 op-identity-text-policy value 49.
78 op-quote-projection value 50.
78 op-quote-transition value 51.
78 op-quote-ordering value 52.
78 op-ledger-projection value 53.
78 op-activity-projection value 54.
78 op-gift-transition value 55.
78 op-gift-refund value 56.
78 op-audit-policy value 57.
78 op-art-transition value 58.
78 op-art-metadata value 59.
78 op-art-equipment value 60.
78 op-bank-invariants value 61.
78 op-market-view value 62.
78 op-household-statistics value 63.
78 result-success value 0.
78 result-invalid value 1.
78 result-overflow value 2.
78 result-insufficient-funds value 3.
78 result-conflict value 4.
78 result-forbidden value 5.
78 result-disabled value 6.
78 result-not-found value 7.
78 result-capacity value 8.
78 result-unavailable value 9.
78 result-listing-closed value 11.
78 result-self-deal value 12.
78 result-bot-good-deed value 13.
78 result-offer-closed value 14.
78 result-settled value 15.
78 result-quote-limit value 16.
78 result-loan-limit value 17.
78 result-stale value 18.
78 result-corrupt value 19.
78 max-i64 value 9223372036854775807.
78 wallet-limit value 9007199254740991.
78 command-amount-limit value 1000000000000000.
78 max-u32 value 4294967295.
78 seconds-per-day value 86400.
78 basis-points-per-whole value 10000.
78 minimum-clock value 1577836800.
78 flow-parts-base value 1000000000000000.
78 select-identity-delegated-view value 0.
78 select-identity-key-scope value 1.
78 select-identity-key-member value 2.
78 select-identity-key-view value 3.
78 select-identity-wire-epoch value 4.
78 select-identity-command-epoch value 5.
78 select-identity-nana-fold value 6.
78 select-identity-name value 0.
78 select-identity-mastodon value 1.
78 select-identity-creation value 2.
78 select-identity-wire-update value 3.
78 select-identity-free-text value 4.
78 select-identity-provision value 5.
78 select-identity-quantity value 6.
78 select-market-listing-filter value 0.
78 select-market-listing-select value 1.
78 select-market-listing-merge value 2.
78 select-market-profile-view value 3.
78 select-market-profile-offer value 4.
78 select-market-listing-count value 5.
78 select-market-account-view value 6.
78 select-market-account-balance value 7.
78 select-market-fulfillment-view value 8.
78 select-market-transaction-view value 9.
78 select-market-account-row value 10.
78 select-market-screen-mail value 11.
78 select-market-newest-fold value 12.
78 select-household-retained-year value 0.
78 select-household-employment-member value 1.
78 select-household-labor-row value 2.
78 select-household-sale-row value 3.
78 select-household-inflation-fold value 4.
78 select-household-employment-fold value 5.
78 select-household-finalize value 6.
78 select-household-filled-quote value 7.
78 select-household-coin-row value 8.
78 select-household-cash-row value 9.
78 select-household-exchange-rate value 10.
78 select-household-quote-rate value 11.
78 select-household-sales-fold value 12.
78 select-ledger-transaction value 0.
78 select-ledger-cursor value 1.
78 select-ledger-row value 2.
78 select-ledger-continuation value 3.
78 select-ledger-retention value 4.
78 select-ledger-insertion value 5.
78 select-ledger-descending-key value 6.
78 select-ledger-circulation value 7.
78 select-ledger-ordinal-selection value 8.
78 select-ledger-audit-cursor value 9.
78 select-ledger-audit-row value 10.
78 select-ledger-audit-continuation value 11.
78 select-ledger-audit-retention value 12.
78 select-ledger-audit-initial value 13.
78 select-activity-event value 0.
78 select-activity-page value 1.
78 select-activity-row value 2.
78 select-activity-account value 3.
78 select-activity-payment-selection value 4.
78 select-bank-currency value 0.
78 select-bank-loan value 1.
78 select-bank-quote value 2.
78 select-bank-initial-sums value 3.
78 select-bank-member-sums value 4.
78 select-bank-conservation value 5.
78 select-bank-ledger value 6.
78 select-bank-epoch value 7.
78 select-bank-correction value 8.
78 select-bank-history-row value 9.
78 select-bank-history-tail value 10.
78 select-bank-audit-sequence value 11.
78 select-bank-audit-tail value 12.
78 select-bank-capacity value 13.
78 select-bank-gift value 14.
78 select-bank-art value 15.
78 select-bank-fulfillment value 16.
78 select-bank-fulfillment-update value 17.
78 select-bank-wide-flow value 19.
78 select-bank-epoch-tail value 20.
78 select-audit-prepare value 0.
78 select-audit-validate value 1.

local-storage section.
*> Fresh temporary data per invocation, shared only by this call's paragraphs.
01 arithmetic-scratch.
   05 min-i64 pic s9(20) comp-3 value -9223372036854775808.
   05 debit-value pic s9(38) comp-3.
   05 credit-value pic s9(38) comp-3.
   05 wide-value pic s9(38) comp-3.
   05 factor-value pic 9(38) comp-3.
   05 quotient-value pic s9(38) comp-3.
   05 remainder-value pic s9(38) comp-3.
   05 denominator-value pic 9(38) comp-3.
   05 exponent-value usage binary-long signed.
01 loan-scratch.
   05 loan-principal usage binary-long-long signed.
   05 loan-interest usage binary-long-long signed.
   05 loan-remainder usage binary-long-long signed.
   05 loan-accrued usage binary-long-long signed.
   05 loan-next-due usage binary-long-long signed.
   05 loan-principal-due usage binary-long-long signed.
   05 loan-interest-due usage binary-long-long signed.
   05 loan-requested usage binary-long-long signed.
   05 loan-paid usage binary-long-long signed.
   05 loan-interest-paid usage binary-long-long signed.
   05 loan-principal-paid usage binary-long-long signed.
   05 loan-periods pic 9(38) comp-3.
   05 loan-interval usage binary-long-long signed.
01 flow-scratch.
   05 flow-index usage binary-long signed.
   05 inbound-index usage binary-long signed.
   05 outbound-index usage binary-long signed.
   05 nana-direction usage binary-long signed.
   05 flow-sign usage binary-long signed.
   05 flow-deltas.
      10 flow-delta usage binary-long-long signed occurs 20.
01 selection-scratch.
   05 selection-index usage binary-long signed.
   05 selection-best usage binary-long signed.
   05 selection-time usage binary-long-long signed.
   05 selection-key usage binary-long-long signed.
01 quote-scratch.
   05 quote-count usage binary-long signed.
   05 quote-room usage binary-long signed.
   05 quote-output usage binary-long signed.
   05 selected-quote-side usage binary-long signed.
   05 selected-quote-price usage binary-long-long signed.
   05 quote-used.
      10 quote-selected usage binary-char occurs 16.
01 display-scratch.
   05 summary-float.
      10 summary-percent usage float-long.
   05 summary-bits redefines summary-float pic x(8).
   05 summary-decimal pic s9(20)v9(18) comp-3.
   05 stats-first usage float-long.
   05 stats-second usage float-long.
   05 stats-total usage float-long.
   05 stats-total-bits redefines stats-total pic x(8).
linkage section.
01 operation-code usage binary-long signed.
01 values-frame.
   02 filler pic x(512).

*> Named views of the fixed-width ABI. REDEFINES aliases the caller's
*> buffer; it allocates no new memory and performs no marshaling/copies.
01 post-frame redefines values-frame.
   02 post-source-balance usage binary-long-long signed.
   02 post-target-balance usage binary-long-long signed.
   02 post-amount usage binary-long-long signed.
   02 post-issuance usage binary-long-long signed.
   02 post-correction usage binary-long-long signed.
   02 post-usd usage binary-long-long signed.
   02 post-next-source-balance usage binary-long-long signed.
   02 post-next-target-balance usage binary-long-long signed.
   02 post-next-state usage binary-long-long signed.
   02 post-same-wallet usage binary-long-long signed.
   02 post-source-member usage binary-long-long signed.
   02 post-target-member usage binary-long-long signed.
   02 post-loan-linked usage binary-long-long signed.
   02 post-credit-mask usage binary-long-long signed.
   02 filler pic x(16).
   02 post-cash-leg usage binary-long-long signed.
   02 post-next-credit-mask usage binary-long-long signed.
   02 filler pic x(368).
01 rescale-frame redefines values-frame.
   02 rescale-units usage binary-long-long signed.
   02 rescale-exponent usage binary-long-long signed.
   02 rescale-limit usage binary-long-long signed.
   02 rescale-converted-units usage binary-long-long signed.
   02 filler pic x(480).
01 exchange-frame redefines values-frame.
   02 exchange-coins usage binary-long-long signed.
   02 exchange-cents-per-coin usage binary-long-long signed.
   02 exchange-nc-scale usage binary-long-long signed.
   02 exchange-cash-cents usage binary-long-long signed.
   02 filler pic x(480).
01 annual-frame redefines values-frame.
   02 annual-period-basis-points usage binary-long-long signed.
   02 annual-period-days usage binary-long-long signed.
   02 annual-basis-points usage binary-long-long signed.
   02 filler pic x(488).
01 accrual-frame redefines values-frame.
   02 accrual-principal usage binary-long-long signed.
   02 accrual-rate-basis-points usage binary-long-long signed.
   02 accrual-rate-days usage binary-long-long signed.
   02 accrual-now usage binary-long-long signed.
   02 accrual-accrued-at usage binary-long-long signed.
   02 accrual-interest usage binary-long-long signed.
   02 accrual-remainder usage binary-long-long signed.
   02 accrual-next-interest usage binary-long-long signed.
   02 accrual-next-remainder usage binary-long-long signed.
   02 accrual-next-accrued-at usage binary-long-long signed.
   02 accrual-denominator usage binary-long-long signed.
   02 filler pic x(424).
01 settle-frame redefines values-frame.
   02 settle-status usage binary-long-long signed.
   02 settle-manual usage binary-long-long signed.
   02 settle-requested-amount usage binary-long-long signed.
   02 settle-now usage binary-long-long signed.
   02 settle-wallet usage binary-long-long signed.
   02 settle-credit-blocked usage binary-long-long signed.
   02 settle-principal usage binary-long-long signed.
   02 settle-interest usage binary-long-long signed.
   02 settle-remainder usage binary-long-long signed.
   02 settle-accrued-at usage binary-long-long signed.
   02 settle-next-due usage binary-long-long signed.
   02 settle-principal-due usage binary-long-long signed.
   02 settle-interest-due usage binary-long-long signed.
   02 filler pic x(8).
   02 settle-amount usage binary-long-long signed.
   02 settle-rate-basis-points usage binary-long-long signed.
   02 settle-rate-days usage binary-long-long signed.
   02 settle-payment-days usage binary-long-long signed.
   02 settle-installment usage binary-long-long signed.
   02 settle-borrower-disabled usage binary-long-long signed.
   02 settle-next-status usage binary-long-long signed.
   02 settle-next-principal usage binary-long-long signed.
   02 settle-next-interest usage binary-long-long signed.
   02 settle-next-remainder usage binary-long-long signed.
   02 settle-next-accrued-at usage binary-long-long signed.
   02 settle-next-due-at usage binary-long-long signed.
   02 settle-next-principal-due usage binary-long-long signed.
   02 settle-next-interest-due usage binary-long-long signed.
   02 settle-next-wallet usage binary-long-long signed.
   02 settle-updated-at usage binary-long-long signed.
   02 settle-principal-paid usage binary-long-long signed.
   02 settle-interest-paid usage binary-long-long signed.
   02 settle-draw-amount usage binary-long-long signed.
   02 settle-lender-disabled usage binary-long-long signed.
   02 filler pic x(240).
01 loan-reform-frame redefines values-frame.
   02 loan-reform-interest usage binary-long-long signed.
   02 loan-reform-remainder usage binary-long-long signed.
   02 loan-reform-denominator usage binary-long-long signed.
   02 loan-reform-exponent usage binary-long-long signed.
   02 loan-reform-converted-principal usage binary-long-long signed.
   02 loan-reform-converted-interest usage binary-long-long signed.
   02 loan-reform-converted-remainder usage binary-long-long signed.
   02 filler pic x(456).
01 loan-post-frame redefines values-frame.
   02 loan-post-draw usage binary-long-long signed.
   02 loan-post-draw-amount usage binary-long-long signed.
   02 loan-post-principal-paid usage binary-long-long signed.
   02 loan-post-interest-paid usage binary-long-long signed.
   02 loan-post-lender usage binary-long-long signed.
   02 loan-post-borrower usage binary-long-long signed.
   02 filler pic x(80).
   02 loan-post-posting-count usage binary-long-long signed.
   02 loan-post-first-payer usage binary-long-long signed.
   02 loan-post-first-payee usage binary-long-long signed.
   02 loan-post-first-amount usage binary-long-long signed.
   02 loan-post-first-kind usage binary-long-long signed.
   02 loan-post-first-offset usage binary-long-long signed.
   02 loan-post-second-payer usage binary-long-long signed.
   02 loan-post-second-payee usage binary-long-long signed.
   02 loan-post-second-amount usage binary-long-long signed.
   02 loan-post-second-kind usage binary-long-long signed.
   02 loan-post-second-offset usage binary-long-long signed.
   02 loan-post-aggregate-amount usage binary-long-long signed.
   02 loan-post-aggregate-payer usage binary-long-long signed.
   02 loan-post-aggregate-payee usage binary-long-long signed.
   02 loan-post-aggregate-present usage binary-long-long signed.
   02 filler pic x(264).
01 identity-query-frame redefines values-frame.
   02 identity-query-action usage binary-long-long signed.
   02 filler pic x(120).
   02 identity-query-eligible usage binary-long-long signed.
   02 identity-query-created-at usage binary-long-long signed.
   02 filler pic x(368).
01 delegated-frame redefines values-frame.
   02 filler pic x(8).
   02 delegated-self usage binary-long-long signed.
   02 delegated-actor-nana usage binary-long-long signed.
   02 delegated-target-exists usage binary-long-long signed.
   02 delegated-target-bot usage binary-long-long signed.
   02 filler pic x(472).
01 key-scope-frame redefines values-frame.
   02 filler pic x(8).
   02 key-scope-delegated-target usage binary-long-long signed.
   02 key-scope-read-only usage binary-long-long signed.
   02 key-scope-mutation usage binary-long-long signed.
   02 filler pic x(480).
01 key-member-frame redefines values-frame.
   02 filler pic x(8).
   02 key-member-exists usage binary-long-long signed.
   02 key-member-disabled usage binary-long-long signed.
   02 key-member-has-login usage binary-long-long signed.
   02 filler pic x(480).
01 key-view-frame redefines values-frame.
   02 filler pic x(8).
   02 key-view-present usage binary-long-long signed.
   02 key-view-created-at usage binary-long-long signed.
   02 filler pic x(488).
01 wire-epoch-frame redefines values-frame.
   02 filler pic x(8).
   02 wire-epoch-mutation usage binary-long-long signed.
   02 wire-epoch-reformed usage binary-long-long signed.
   02 wire-epoch-reform-route usage binary-long-long signed.
   02 wire-epoch-epoch-matches usage binary-long-long signed.
   02 filler pic x(472).
01 command-epoch-frame redefines values-frame.
   02 filler pic x(8).
   02 command-epoch-matches usage binary-long-long signed.
   02 filler pic x(496).
01 nana-count-frame redefines values-frame.
   02 filler pic x(8).
   02 nana-count-previous usage binary-long-long signed.
   02 nana-count-row-count usage binary-long-long signed.
   02 filler pic x(488).
01 admin-frame redefines values-frame.
   02 admin-empty-bank usage binary-long-long signed.
   02 admin-actor usage binary-long-long signed.
   02 admin-actor-exists usage binary-long-long signed.
   02 admin-actor-nana usage binary-long-long signed.
   02 admin-actor-disabled usage binary-long-long signed.
   02 filler pic x(472).
01 last-nana-frame redefines values-frame.
   02 last-nana-current-nana usage binary-long-long signed.
   02 last-nana-current-disabled usage binary-long-long signed.
   02 last-nana-removing-nana usage binary-long-long signed.
   02 last-nana-active-count usage binary-long-long signed.
   02 filler pic x(480).
01 key-frame redefines values-frame.
   02 key-self usage binary-long-long signed.
   02 key-administrator usage binary-long-long signed.
   02 key-revoke usage binary-long-long signed.
   02 key-bot-full-key usage binary-long-long signed.
   02 key-has-login usage binary-long-long signed.
   02 key-disabled usage binary-long-long signed.
   02 filler pic x(464).
01 grant-frame redefines values-frame.
   02 grant-issuance-balance usage binary-long-long signed.
   02 grant-amount usage binary-long-long signed.
   02 filler pic x(496).
01 config-frame redefines values-frame.
   02 config-initial-grant usage binary-long-long signed.
   02 config-settlement-interval usage binary-long-long signed.
   02 config-now usage binary-long-long signed.
   02 filler pic x(488).
01 loan-policy-frame redefines values-frame.
   02 loan-policy-action usage binary-long-long signed.
   02 loan-policy-actor usage binary-long-long signed.
   02 loan-policy-now usage binary-long-long signed.
   02 loan-policy-borrower-exists usage binary-long-long signed.
   02 loan-policy-borrower-disabled usage binary-long-long signed.
   02 loan-policy-terms-borrower usage binary-long-long signed.
   02 loan-policy-amount usage binary-long-long signed.
   02 loan-policy-installment usage binary-long-long signed.
   02 loan-policy-rate-days usage binary-long-long signed.
   02 loan-policy-payment-days usage binary-long-long signed.
   02 loan-policy-memo-control usage binary-long-long signed.
   02 loan-policy-exists usage binary-long-long signed.
   02 loan-policy-status usage binary-long-long signed.
   02 loan-policy-borrower usage binary-long-long signed.
   02 loan-policy-lender usage binary-long-long signed.
   02 loan-policy-lender-exists usage binary-long-long signed.
   02 loan-policy-lender-disabled usage binary-long-long signed.
   02 loan-policy-credit-enabled usage binary-long-long signed.
   02 loan-policy-mine-count usage binary-long-long signed.
   02 loan-policy-book-full usage binary-long-long signed.
   02 loan-policy-recyclable usage binary-long-long signed.
   02 loan-policy-expected-updated usage binary-long-long signed.
   02 loan-policy-updated usage binary-long-long signed.
   02 loan-policy-ready usage binary-long-long signed.
   02 loan-policy-principal usage binary-long-long signed.
   02 loan-policy-accrued-at usage binary-long-long signed.
   02 loan-policy-next-due usage binary-long-long signed.
   02 filler pic x(24).
   02 loan-policy-next-status usage binary-long-long signed.
   02 loan-policy-next-lender usage binary-long-long signed.
   02 loan-policy-next-borrower usage binary-long-long signed.
   02 loan-policy-draw-amount usage binary-long-long signed.
   02 loan-policy-next-due-at usage binary-long-long signed.
   02 loan-policy-next-principal usage binary-long-long signed.
   02 loan-policy-next-accrued-at usage binary-long-long signed.
   02 loan-policy-next-updated usage binary-long-long signed.
   02 loan-policy-overflow usage binary-long-long signed.
   02 filler pic x(200).
01 loan-view-frame redefines values-frame.
   02 loan-view-status usage binary-long-long signed.
   02 loan-view-nana usage binary-long-long signed.
   02 loan-view-viewer usage binary-long-long signed.
   02 loan-view-lender usage binary-long-long signed.
   02 loan-view-borrower usage binary-long-long signed.
   02 loan-view-borrower-exists usage binary-long-long signed.
   02 loan-view-lender-exists usage binary-long-long signed.
   02 loan-view-borrower-disabled usage binary-long-long signed.
   02 loan-view-lender-disabled usage binary-long-long signed.
   02 loan-view-borrower-balance usage binary-long-long signed.
   02 loan-view-lender-balance usage binary-long-long signed.
   02 loan-view-amount usage binary-long-long signed.
   02 loan-view-credit-blocked usage binary-long-long signed.
   02 loan-view-next-due usage binary-long-long signed.
   02 loan-view-now usage binary-long-long signed.
   02 loan-view-principal-due usage binary-long-long signed.
   02 loan-view-interest-due usage binary-long-long signed.
   02 loan-view-attempted-balance usage binary-long-long signed.
   02 loan-view-accrual-failed usage binary-long-long signed.
   02 loan-view-profile-subject usage binary-long-long signed.
   02 loan-view-visible usage binary-long-long signed.
   02 loan-view-terminal usage binary-long-long signed.
   02 loan-view-ready usage binary-long-long signed.
   02 loan-view-waiting-reason usage binary-long-long signed.
   02 loan-view-include usage binary-long-long signed.
   02 loan-view-redact usage binary-long-long signed.
   02 loan-view-overdue usage binary-long-long signed.
   02 loan-view-accrue-requested usage binary-long-long signed.
   02 filler pic x(288).
01 member-update-frame redefines values-frame.
   02 member-update-exists usage binary-long-long signed.
   02 member-update-bio-invalid usage binary-long-long signed.
   02 member-update-verifier-present usage binary-long-long signed.
   02 member-update-new-verifier-present usage binary-long-long signed.
   02 member-update-self usage binary-long-long signed.
   02 member-update-role-present usage binary-long-long signed.
   02 member-update-status-present usage binary-long-long signed.
   02 member-update-administrator usage binary-long-long signed.
   02 member-update-name-blank usage binary-long-long signed.
   02 member-update-name-control usage binary-long-long signed.
   02 member-update-mastodon-empty usage binary-long-long signed.
   02 member-update-mastodon-user-empty usage binary-long-long signed.
   02 member-update-mastodon-host-empty usage binary-long-long signed.
   02 member-update-mastodon-extra-at usage binary-long-long signed.
   02 member-update-mastodon-host-dot usage binary-long-long signed.
   02 member-update-mastodon-invalid usage binary-long-long signed.
   02 member-update-verifier-valid usage binary-long-long signed.
   02 member-update-current-nana usage binary-long-long signed.
   02 member-update-current-disabled usage binary-long-long signed.
   02 member-update-removing-nana usage binary-long-long signed.
   02 member-update-active-nana-count usage binary-long-long signed.
   02 filler pic x(344).
01 loan-book-frame redefines values-frame.
   02 loan-book-actor usage binary-long-long signed.
   02 loan-book-row-count usage binary-long-long signed.
   02 filler pic x(256).
   02 loan-book-pending-count usage binary-long-long signed.
   02 loan-book-terminal-mask usage binary-long-long signed.
   02 filler pic x(224).
01 loan-summary-frame redefines values-frame.
   02 loan-summary-status usage binary-long-long signed.
   02 loan-summary-principal usage binary-long-long signed.
   02 loan-summary-principal-due usage binary-long-long signed.
   02 loan-summary-interest-due usage binary-long-long signed.
   02 loan-summary-rate-basis-points usage binary-long-long signed.
   02 loan-summary-rate-days usage binary-long-long signed.
   02 loan-summary-total-principal usage binary-long-long signed.
   02 loan-summary-total-overdue usage binary-long-long signed.
   02 loan-summary-active-count usage binary-long-long signed.
   02 loan-summary-weighted-high usage binary-long-long signed.
   02 loan-summary-weighted-low usage binary-long-long signed.
   02 loan-summary-finalize usage binary-long-long signed.
   02 loan-summary-rate-present usage binary-long-long signed.
   02 loan-summary-rate-bits usage binary-long-long signed.
   02 filler pic x(400).
01 fulfill-frame redefines values-frame.
   02 fulfill-status usage binary-long-long signed.
   02 fulfill-action usage binary-long-long signed.
   02 fulfill-actor usage binary-long-long signed.
   02 fulfill-provider usage binary-long-long signed.
   02 fulfill-recipient usage binary-long-long signed.
   02 fulfill-reason-empty usage binary-long-long signed.
   02 fulfill-reason-control usage binary-long-long signed.
   02 fulfill-next-status usage binary-long-long signed.
   02 filler pic x(448).
01 offer-view-frame redefines values-frame.
   02 offer-view-phase usage binary-long-long signed.
   02 offer-view-now usage binary-long-long signed.
   02 offer-view-settles-at usage binary-long-long signed.
   02 offer-view-viewer usage binary-long-long signed.
   02 offer-view-nana usage binary-long-long signed.
   02 offer-view-disabled usage binary-long-long signed.
   02 offer-view-owner usage binary-long-long signed.
   02 offer-view-offerer usage binary-long-long signed.
   02 offer-view-status usage binary-long-long signed.
   02 offer-view-reversible usage binary-long-long signed.
   02 offer-view-visible usage binary-long-long signed.
   02 offer-view-recyclable usage binary-long-long signed.
   02 offer-view-pinned usage binary-long-long signed.
   02 offer-view-listing-live usage binary-long-long signed.
   02 filler pic x(400).
01 offer-frame redefines values-frame.
   02 offer-action usage binary-long-long signed.
   02 offer-now usage binary-long-long signed.
   02 offer-actor usage binary-long-long signed.
   02 offer-nana usage binary-long-long signed.
   02 offer-amount usage binary-long-long signed.
   02 offer-memo-control usage binary-long-long signed.
   02 offer-listing-exists usage binary-long-long signed.
   02 offer-listing-active usage binary-long-long signed.
   02 offer-listing-sold usage binary-long-long signed.
   02 offer-listing-owner usage binary-long-long signed.
   02 offer-buy-side usage binary-long-long signed.
   02 offer-good-deed usage binary-long-long signed.
   02 offer-labor usage binary-long-long signed.
   02 offer-owner-nana usage binary-long-long signed.
   02 offer-bot usage binary-long-long signed.
   02 offer-book-full usage binary-long-long signed.
   02 offer-recyclable usage binary-long-long signed.
   02 offer-exists usage binary-long-long signed.
   02 offer-phase usage binary-long-long signed.
   02 offer-owner usage binary-long-long signed.
   02 offer-offerer usage binary-long-long signed.
   02 offer-parties-disabled usage binary-long-long signed.
   02 offer-settles-at usage binary-long-long signed.
   02 offer-original-payer usage binary-long-long signed.
   02 offer-original-payee usage binary-long-long signed.
   02 offer-reversal-window usage binary-long-long signed.
   02 offer-correction-room usage binary-long-long signed.
   02 offer-refunded usage binary-long-long signed.
   02 offer-reversed usage binary-long-long signed.
   02 offer-offerer-nana usage binary-long-long signed.
   02 offer-next-phase usage binary-long-long signed.
   02 offer-payer usage binary-long-long signed.
   02 offer-payee usage binary-long-long signed.
   02 offer-deadline usage binary-long-long signed.
   02 offer-overflow usage binary-long-long signed.
   02 offer-mutate-listing usage binary-long-long signed.
   02 filler pic x(224).
01 import-member-frame redefines values-frame.
   02 import-member-action usage binary-long-long signed.
   02 import-member-administrator usage binary-long-long signed.
   02 import-member-self usage binary-long-long signed.
   02 import-member-target-exists usage binary-long-long signed.
   02 import-member-verifier-present usage binary-long-long signed.
   02 import-member-name-blank usage binary-long-long signed.
   02 import-member-name-control usage binary-long-long signed.
   02 import-member-verifier-valid usage binary-long-long signed.
   02 import-member-count usage binary-long-long signed.
   02 import-member-capacity usage binary-long-long signed.
   02 import-member-duplicate usage binary-long-long signed.
   02 import-member-zero-token-hash usage binary-long-long signed.
   02 filler pic x(416).
01 oldest-frame redefines values-frame.
   02 oldest-selected-index usage binary-long-long signed.
   02 filler pic x(504).
01 offer-links-frame redefines values-frame.
   02 offer-links-original-transaction usage binary-long-long signed.
   02 offer-links-row-count usage binary-long-long signed.
   02 filler pic x(256).
   02 offer-links-reversed-mask usage binary-long-long signed.
   02 filler pic x(232).
01 listing-frame redefines values-frame.
   02 listing-action usage binary-long-long signed.
   02 listing-actor usage binary-long-long signed.
   02 listing-administrator usage binary-long-long signed.
   02 listing-create-actor-nana usage binary-long-long signed.
   02 listing-exists usage binary-long-long signed.
   02 listing-owner usage binary-long-long signed.
   02 listing-active usage binary-long-long signed.
   02 listing-buy-side usage binary-long-long signed.
   02 listing-good-deed usage binary-long-long signed.
   02 listing-labor usage binary-long-long signed.
   02 listing-owner-nana usage binary-long-long signed.
   02 listing-price usage binary-long-long signed.
   02 listing-title-blank usage binary-long-long signed.
   02 listing-title-present usage binary-long-long signed.
   02 listing-price-present usage binary-long-long signed.
   02 listing-details-present usage binary-long-long signed.
   02 listing-details-kind usage binary-long-long signed.
   02 listing-currency-empty usage binary-long-long signed.
   02 listing-currency-control usage binary-long-long signed.
   02 listing-minor-units usage binary-long-long signed.
   02 listing-book-full usage binary-long-long signed.
   02 listing-recyclable usage binary-long-long signed.
   02 listing-quantity-milli usage binary-long-long signed.
   02 listing-requested-thing usage binary-long-long signed.
   02 listing-thing-exists usage binary-long-long signed.
   02 listing-thing-matches usage binary-long-long signed.
   02 listing-named-thing-exists usage binary-long-long signed.
   02 listing-named-thing-matches usage binary-long-long signed.
   02 listing-thing-book-full usage binary-long-long signed.
   02 listing-thing-recyclable usage binary-long-long signed.
   02 listing-next-status usage binary-long-long signed.
   02 listing-payer usage binary-long-long signed.
   02 listing-payee usage binary-long-long signed.
   02 listing-thing-action usage binary-long-long signed.
   02 listing-next-standard usage binary-long-long signed.
   02 filler pic x(8).
   02 listing-existing-standard usage binary-long-long signed.
   02 listing-requested-standard usage binary-long-long signed.
   02 listing-buyer-nana usage binary-long-long signed.
   02 filler pic x(200).
01 first-frame redefines values-frame.
   02 first-selected-index usage binary-long-long signed.
   02 filler pic x(504).
01 recycle-frame redefines values-frame.
   02 recycle-kind usage binary-long-long signed.
   02 recycle-active-or-standard usage binary-long-long signed.
   02 recycle-dependency usage binary-long-long signed.
   02 recycle-eligible usage binary-long-long signed.
   02 filler pic x(480).
01 identity-frame redefines values-frame.
   02 identity-action usage binary-long-long signed.
   02 identity-member-count usage binary-long-long signed.
   02 identity-requested-role usage binary-long-long signed.
   02 identity-current-role usage binary-long-long signed.
   02 identity-current-disabled usage binary-long-long signed.
   02 identity-role-present usage binary-long-long signed.
   02 identity-status-present usage binary-long-long signed.
   02 identity-requested-disabled usage binary-long-long signed.
   02 identity-revoke-keys usage binary-long-long signed.
   02 identity-zero-key usage binary-long-long signed.
   02 identity-now usage binary-long-long signed.
   02 identity-household-blank usage binary-long-long signed.
   02 identity-current-settlement usage binary-long-long signed.
   02 identity-requested-settlement usage binary-long-long signed.
   02 identity-default-settlement usage binary-long-long signed.
   02 identity-grant usage binary-long-long signed.
   02 filler pic x(128).
   02 identity-member-id usage binary-long-long signed.
   02 identity-role usage binary-long-long signed.
   02 identity-kind usage binary-long-long signed.
   02 identity-disabled usage binary-long-long signed.
   02 identity-revoke-key-plan usage binary-long-long signed.
   02 identity-key-created-at usage binary-long-long signed.
   02 identity-full-key usage binary-long-long signed.
   02 identity-default-household usage binary-long-long signed.
   02 identity-settlement usage binary-long-long signed.
   02 identity-grant-cash-leg usage binary-long-long signed.
   02 filler pic x(176).
01 flow-frame redefines values-frame.
   02 flow-amount usage binary-long-long signed.
   02 flow-payer usage binary-long-long signed.
   02 flow-payee usage binary-long-long signed.
   02 flow-usd usage binary-long-long signed.
   02 flow-quote usage binary-long-long signed.
   02 flow-lotto usage binary-long-long signed.
   02 flow-reversal usage binary-long-long signed.
   02 flow-economic-kind usage binary-long-long signed.
   02 filler pic x(448).
01 flow-sum-frame redefines values-frame.
   02 filler pic x(512).
01 lotto-frame redefines values-frame.
   02 lotto-action usage binary-long-long signed.
   02 lotto-now usage binary-long-long signed.
   02 lotto-actor usage binary-long-long signed.
   02 lotto-administrator usage binary-long-long signed.
   02 lotto-exists usage binary-long-long signed.
   02 lotto-kind usage binary-long-long signed.
   02 lotto-title-valid usage binary-long-long signed.
   02 lotto-ticket-price usage binary-long-long signed.
   02 lotto-closes-at usage binary-long-long signed.
   02 lotto-rate-basis-points usage binary-long-long signed.
   02 lotto-house usage binary-long-long signed.
   02 lotto-step usage binary-long-long signed.
   02 lotto-pool usage binary-long-long signed.
   02 lotto-escrow usage binary-long-long signed.
   02 lotto-interest usage binary-long-long signed.
   02 lotto-remaining usage binary-long-long signed.
   02 lotto-winner usage binary-long-long signed.
   02 lotto-ticket-count-or-draw usage binary-long-long signed.
   02 lotto-global-escrow usage binary-long-long signed.
   02 lotto-actor-or-house-cash usage binary-long-long signed.
   02 lotto-expected-step usage binary-long-long signed.
   02 lotto-book-count usage binary-long-long signed.
   02 lotto-recyclable-row usage binary-long-long signed.
   02 lotto-actor-ticket-count usage binary-long-long signed.
   02 filler pic x(256).
   02 lotto-payment-amount usage binary-long-long signed.
   02 lotto-next-pool usage binary-long-long signed.
   02 lotto-next-escrow usage binary-long-long signed.
   02 lotto-next-interest usage binary-long-long signed.
   02 lotto-next-remaining usage binary-long-long signed.
   02 lotto-next-winner usage binary-long-long signed.
   02 lotto-next-step usage binary-long-long signed.
   02 lotto-payment-parties usage binary-long-long signed.
01 lotto-view-frame redefines values-frame.
   02 lotto-view-kind usage binary-long-long signed.
   02 lotto-view-closes-at usage binary-long-long signed.
   02 lotto-view-step usage binary-long-long signed.
   02 lotto-view-now usage binary-long-long signed.
   02 filler pic x(256).
   02 lotto-view-total-tickets usage binary-long-long signed.
   02 lotto-view-due-at usage binary-long-long signed.
   02 lotto-view-status usage binary-long-long signed.
   02 lotto-view-tick-eligible usage binary-long-long signed.
   02 lotto-view-terminal usage binary-long-long signed.
   02 lotto-view-draw-ticket usage binary-long-long signed.
   02 lotto-view-winner usage binary-long-long signed.
   02 filler pic x(168).
01 lotto-book-frame redefines values-frame.
   02 lotto-book-row-count usage binary-long-long signed.
   02 filler pic x(128).
   02 lotto-book-recyclable-row usage binary-long-long signed.
   02 filler pic x(368).
01 lotto-check-frame redefines values-frame.
   02 lotto-check-id usage binary-long-long signed.
   02 lotto-check-sequence usage binary-long-long signed.
   02 lotto-check-duplicate usage binary-long-long signed.
   02 lotto-check-house-exists usage binary-long-long signed.
   02 lotto-check-kind usage binary-long-long signed.
   02 lotto-check-ticket-price usage binary-long-long signed.
   02 lotto-check-rate-basis-points usage binary-long-long signed.
   02 lotto-check-closes-at usage binary-long-long signed.
   02 lotto-check-step usage binary-long-long signed.
   02 lotto-check-pool usage binary-long-long signed.
   02 lotto-check-escrow usage binary-long-long signed.
   02 lotto-check-interest usage binary-long-long signed.
   02 lotto-check-remaining usage binary-long-long signed.
   02 lotto-check-winner usage binary-long-long signed.
   02 lotto-check-member-mask usage binary-long-long signed.
   02 lotto-check-escrow-sum usage binary-long-long signed.
   02 filler pic x(256).
   02 lotto-check-next-escrow-sum usage binary-long-long signed.
   02 lotto-check-global-escrow usage binary-long-long signed.
   02 lotto-check-finalize usage binary-long-long signed.
   02 filler pic x(104).
01 reform-frame redefines values-frame.
   02 reform-requested-decimals usage binary-long-long signed.
   02 reform-power usage binary-long-long signed.
   02 reform-current-decimals usage binary-long-long signed.
   02 reform-epoch-count usage binary-long-long signed.
   02 reform-current-epoch usage binary-long-long signed.
   02 reform-current-sequence usage binary-long-long signed.
   02 reform-expected-epoch usage binary-long-long signed.
   02 reform-expected-sequence usage binary-long-long signed.
   02 reform-epoch-capacity usage binary-long-long signed.
   02 reform-exponent usage binary-long-long signed.
   02 reform-next-epoch usage binary-long-long signed.
   02 reform-nc-scale usage binary-long-long signed.
   02 filler pic x(416).
01 reform-lotto-frame redefines values-frame.
   02 reform-lotto-pool usage binary-long-long signed.
   02 reform-lotto-promised-interest usage binary-long-long signed.
   02 reform-lotto-rate-basis-points usage binary-long-long signed.
   02 filler pic x(488).
01 reform-field-frame redefines values-frame.
   02 reform-field-value usage binary-long-long signed.
   02 reform-field-old-decimals usage binary-long-long signed.
   02 reform-field-new-decimals usage binary-long-long signed.
   02 reform-field-power usage binary-long-long signed.
   02 reform-field-kind usage binary-long-long signed.
   02 reform-field-converted-value usage binary-long-long signed.
   02 filler pic x(464).
01 account-frame redefines values-frame.
   02 account-payer usage binary-long-long signed.
   02 account-payee usage binary-long-long signed.
   02 account-amount usage binary-long-long signed.
   02 account-correction usage binary-long-long signed.
   02 account-usd usage binary-long-long signed.
   02 account-source-exists usage binary-long-long signed.
   02 account-source-disabled usage binary-long-long signed.
   02 account-target-exists usage binary-long-long signed.
   02 account-target-disabled usage binary-long-long signed.
   02 account-source-balance usage binary-long-long signed.
   02 account-target-balance usage binary-long-long signed.
   02 account-saved-correction usage binary-long-long signed.
   02 account-saved-usd usage binary-long-long signed.
   02 filler pic x(408).
01 correction-frame redefines values-frame.
   02 correction-original-amount usage binary-long-long signed.
   02 correction-already-refunded usage binary-long-long signed.
   02 correction-amount usage binary-long-long signed.
   02 correction-transaction-id usage binary-long-long signed.
   02 correction-next-refunded usage binary-long-long signed.
   02 correction-full-correction-id usage binary-long-long signed.
   02 filler pic x(464).
01 correction-policy-frame redefines values-frame.
   02 correction-policy-action usage binary-long-long signed.
   02 correction-policy-original-exists usage binary-long-long signed.
   02 correction-policy-actor usage binary-long-long signed.
   02 correction-policy-nana usage binary-long-long signed.
   02 correction-policy-original-payer usage binary-long-long signed.
   02 correction-policy-original-payee usage binary-long-long signed.
   02 correction-policy-original-amount usage binary-long-long signed.
   02 correction-policy-original-reversal usage binary-long-long signed.
   02 correction-policy-usd usage binary-long-long signed.
   02 correction-policy-loan usage binary-long-long signed.
   02 correction-policy-lotto usage binary-long-long signed.
   02 correction-policy-quote usage binary-long-long signed.
   02 correction-policy-art usage binary-long-long signed.
   02 correction-policy-refunded usage binary-long-long signed.
   02 correction-policy-reversed usage binary-long-long signed.
   02 correction-policy-requested-amount usage binary-long-long signed.
   02 correction-policy-annotation-count usage binary-long-long signed.
   02 correction-policy-annotation-capacity usage binary-long-long signed.
   02 correction-policy-annotation-exists usage binary-long-long signed.
   02 correction-policy-payer usage binary-long-long signed.
   02 correction-policy-payee usage binary-long-long signed.
   02 correction-policy-amount usage binary-long-long signed.
   02 correction-policy-overdraft usage binary-long-long signed.
   02 filler pic x(328).
01 historical-frame redefines values-frame.
   02 historical-original-units usage binary-long-long signed.
   02 historical-usd usage binary-long-long signed.
   02 historical-amount-epoch usage binary-long-long signed.
   02 historical-epoch-count usage binary-long-long signed.
   02 historical-limit usage binary-long-long signed.
   02 filler pic x(256).
   02 historical-converted-amount usage binary-long-long signed.
   02 historical-total-exponent usage binary-long-long signed.
   02 filler pic x(200).
01 transfer-frame redefines values-frame.
   02 transfer-payer usage binary-long-long signed.
   02 transfer-payee usage binary-long-long signed.
   02 transfer-amount usage binary-long-long signed.
   02 transfer-correction usage binary-long-long signed.
   02 transfer-usd usage binary-long-long signed.
   02 transfer-source-exists usage binary-long-long signed.
   02 transfer-source-disabled usage binary-long-long signed.
   02 transfer-target-exists usage binary-long-long signed.
   02 transfer-target-disabled usage binary-long-long signed.
   02 transfer-source-balance usage binary-long-long signed.
   02 transfer-target-balance usage binary-long-long signed.
   02 filler pic x(16).
   02 transfer-classified usage binary-long-long signed.
   02 transfer-recipient-nana usage binary-long-long signed.
   02 transfer-memo-blank usage binary-long-long signed.
   02 transfer-memo-control usage binary-long-long signed.
   02 transfer-quantity usage binary-long-long signed.
   02 transfer-economic-kind usage binary-long-long signed.
   02 transfer-unit usage binary-long-long signed.
   02 transfer-thing-present usage binary-long-long signed.
   02 transfer-thing-exists usage binary-long-long signed.
   02 transfer-thing-kind usage binary-long-long signed.
   02 transfer-thing-unit usage binary-long-long signed.
   02 filler pic x(320).
01 actor-frame redefines values-frame.
   02 actor-member-count usage binary-long-long signed.
   02 actor-id usage binary-long-long signed.
   02 actor-exists usage binary-long-long signed.
   02 actor-disabled usage binary-long-long signed.
   02 actor-bootstrap-command usage binary-long-long signed.
   02 filler pic x(472).
01 fulfill-room-frame redefines values-frame.
   02 fulfill-room-command-category usage binary-long-long signed.
   02 fulfill-room-amount usage binary-long-long signed.
   02 fulfill-room-economic-kind usage binary-long-long signed.
   02 fulfill-room-full usage binary-long-long signed.
   02 fulfill-room-recyclable usage binary-long-long signed.
   02 filler pic x(472).
01 fulfill-create-frame redefines values-frame.
   02 fulfill-create-reversal usage binary-long-long signed.
   02 fulfill-create-full-correction usage binary-long-long signed.
   02 fulfill-create-usd usage binary-long-long signed.
   02 fulfill-create-amount usage binary-long-long signed.
   02 fulfill-create-loan usage binary-long-long signed.
   02 fulfill-create-lotto usage binary-long-long signed.
   02 fulfill-create-quote usage binary-long-long signed.
   02 fulfill-create-art usage binary-long-long signed.
   02 fulfill-create-payer usage binary-long-long signed.
   02 fulfill-create-listing-present usage binary-long-long signed.
   02 fulfill-create-economic-kind usage binary-long-long signed.
   02 fulfill-create-cash-kind usage binary-long-long signed.
   02 fulfill-create-service-kind usage binary-long-long signed.
   02 fulfill-create-action usage binary-long-long signed.
   02 fulfill-create-kind usage binary-long-long signed.
   02 fulfill-create-status usage binary-long-long signed.
   02 filler pic x(384).
01 fulfill-recycle-frame redefines values-frame.
   02 fulfill-recycle-row-count usage binary-long-long signed.
   02 filler pic x(496).
   02 fulfill-recycle-selected-index usage binary-long-long signed.
01 new-member-frame redefines values-frame.
   02 new-member-username-blank usage binary-long-long signed.
   02 new-member-username-control usage binary-long-long signed.
   02 new-member-display-blank usage binary-long-long signed.
   02 new-member-display-control usage binary-long-long signed.
   02 new-member-verifier-valid usage binary-long-long signed.
   02 new-member-member-count usage binary-long-long signed.
   02 new-member-capacity usage binary-long-long signed.
   02 new-member-duplicate-username usage binary-long-long signed.
   02 filler pic x(448).
01 text-frame redefines values-frame.
   02 text-action usage binary-long-long signed.
   02 filler pic x(504).
01 name-frame redefines values-frame.
   02 filler pic x(8).
   02 name-blank usage binary-long-long signed.
   02 name-control usage binary-long-long signed.
   02 filler pic x(488).
01 mastodon-frame redefines values-frame.
   02 filler pic x(8).
   02 mastodon-empty usage binary-long-long signed.
   02 mastodon-user-empty usage binary-long-long signed.
   02 mastodon-host-empty usage binary-long-long signed.
   02 mastodon-extra-at usage binary-long-long signed.
   02 mastodon-host-has-dot usage binary-long-long signed.
   02 mastodon-invalid-text usage binary-long-long signed.
   02 filler pic x(456).
01 creation-frame redefines values-frame.
   02 filler pic x(8).
   02 creation-display-blank usage binary-long-long signed.
   02 creation-bot usage binary-long-long signed.
   02 creation-nana usage binary-long-long signed.
   02 creation-grant-requested usage binary-long-long signed.
   02 creation-configured-grant usage binary-long-long signed.
   02 filler pic x(8).
   02 creation-default-display usage binary-long-long signed.
   02 creation-grant usage binary-long-long signed.
   02 filler pic x(440).
01 wire-update-frame redefines values-frame.
   02 filler pic x(8).
   02 wire-update-actor-nana usage binary-long-long signed.
   02 wire-update-self usage binary-long-long signed.
   02 wire-update-role-present usage binary-long-long signed.
   02 wire-update-status-present usage binary-long-long signed.
   02 filler pic x(472).
01 free-text-frame redefines values-frame.
   02 filler pic x(8).
   02 free-text-control usage binary-long-long signed.
   02 filler pic x(496).
01 provision-frame redefines values-frame.
   02 filler pic x(8).
   02 provision-member-count usage binary-long-long signed.
   02 filler pic x(496).
01 quantity-frame redefines values-frame.
   02 filler pic x(8).
   02 quantity-whole usage binary-long-long signed.
   02 quantity-fraction usage binary-long-long signed.
   02 quantity-fraction-digits usage binary-long-long signed.
   02 filler pic x(96).
   02 quantity-thousandths usage binary-long-long signed.
   02 filler pic x(376).
01 quote-view-frame redefines values-frame.
   02 quote-view-status usage binary-long-long signed.
   02 quote-view-now usage binary-long-long signed.
   02 quote-view-expires-at usage binary-long-long signed.
   02 quote-view-side usage binary-long-long signed.
   02 quote-view-maker usage binary-long-long signed.
   02 quote-view-taker usage binary-long-long signed.
   02 quote-view-live usage binary-long-long signed.
   02 quote-view-next-status usage binary-long-long signed.
   02 quote-view-seller usage binary-long-long signed.
   02 quote-view-buyer usage binary-long-long signed.
   02 filler pic x(432).
01 quote-frame redefines values-frame.
   02 quote-action usage binary-long-long signed.
   02 quote-actor usage binary-long-long signed.
   02 quote-administrator usage binary-long-long signed.
   02 quote-now usage binary-long-long signed.
   02 quote-exists usage binary-long-long signed.
   02 quote-maker usage binary-long-long signed.
   02 quote-side usage binary-long-long signed.
   02 quote-status usage binary-long-long signed.
   02 quote-live usage binary-long-long signed.
   02 quote-cents-per-coin usage binary-long-long signed.
   02 quote-coins usage binary-long-long signed.
   02 quote-nc-scale usage binary-long-long signed.
   02 quote-expires-at usage binary-long-long signed.
   02 filler pic x(24).
   02 quote-next-status usage binary-long-long signed.
   02 quote-seller usage binary-long-long signed.
   02 quote-buyer usage binary-long-long signed.
   02 quote-cash-total usage binary-long-long signed.
   02 filler pic x(352).
01 quote-order-frame redefines values-frame.
   02 filler pic x(512).
01 ledger-frame redefines values-frame.
   02 ledger-action usage binary-long-long signed.
   02 filler pic x(504).
01 ledger-tx-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-tx-amount usage binary-long-long signed.
   02 ledger-tx-reversal usage binary-long-long signed.
   02 ledger-tx-listing usage binary-long-long signed.
   02 ledger-tx-loan usage binary-long-long signed.
   02 ledger-tx-lotto usage binary-long-long signed.
   02 ledger-tx-quote usage binary-long-long signed.
   02 ledger-tx-payer usage binary-long-long signed.
   02 ledger-tx-payee usage binary-long-long signed.
   02 filler pic x(56).
   02 ledger-tx-kind usage binary-long-long signed.
   02 ledger-tx-reference usage binary-long-long signed.
   02 ledger-tx-debit usage binary-long-long signed.
   02 ledger-tx-credit usage binary-long-long signed.
   02 ledger-tx-public usage binary-long-long signed.
   02 filler pic x(344).
01 ledger-cursor-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-cursor-incarnation-matches usage binary-long-long signed.
   02 ledger-cursor-upper usage binary-long-long signed.
   02 ledger-cursor-before usage binary-long-long signed.
   02 ledger-cursor-page usage binary-long-long signed.
   02 ledger-cursor-total usage binary-long-long signed.
   02 ledger-cursor-next-page usage binary-long-long signed.
   02 ledger-cursor-first-page usage binary-long-long signed.
   02 ledger-cursor-limit usage binary-long-long signed.
   02 filler pic x(56).
   02 ledger-cursor-effective-limit usage binary-long-long signed.
   02 filler pic x(376).
01 ledger-row-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-row-ordinal usage binary-long-long signed.
   02 ledger-row-before usage binary-long-long signed.
   02 ledger-row-upper usage binary-long-long signed.
   02 ledger-row-amount usage binary-long-long signed.
   02 ledger-row-actor-present usage binary-long-long signed.
   02 ledger-row-actor usage binary-long-long signed.
   02 ledger-row-payer usage binary-long-long signed.
   02 ledger-row-payee usage binary-long-long signed.
   02 ledger-row-account-present usage binary-long-long signed.
   02 ledger-row-account usage binary-long-long signed.
   02 ledger-row-usd usage binary-long-long signed.
   02 ledger-row-account-usd usage binary-long-long signed.
   02 filler pic x(24).
   02 ledger-row-include usage binary-long-long signed.
   02 filler pic x(376).
01 ledger-more-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-more-count usage binary-long-long signed.
   02 ledger-more-count-limit usage binary-long-long signed.
   02 ledger-more-retained-limit usage binary-long-long signed.
   02 ledger-more-page usage binary-long-long signed.
   02 ledger-more-first-page usage binary-long-long signed.
   02 filler pic x(80).
   02 ledger-more-present usage binary-long-long signed.
   02 filler pic x(376).
01 ledger-retention-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-retention-archive-present usage binary-long-long signed.
   02 ledger-retention-first-page usage binary-long-long signed.
   02 ledger-retention-total usage binary-long-long signed.
   02 ledger-retention-retained-capacity usage binary-long-long signed.
   02 filler pic x(88).
   02 ledger-retention-truncated usage binary-long-long signed.
   02 filler pic x(376).
01 ledger-insert-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-insert-archive-enabled usage binary-long-long signed.
   02 ledger-insert-ordinal usage binary-long-long signed.
   02 ledger-insert-floor usage binary-long-long signed.
   02 ledger-insert-last-page usage binary-long-long signed.
   02 ledger-insert-next-page usage binary-long-long signed.
   02 filler pic x(80).
   02 ledger-insert-position usage binary-long-long signed.
   02 filler pic x(376).
01 ledger-key-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-key-ordinal usage binary-long-long signed.
   02 filler pic x(112).
   02 ledger-key-descending usage binary-long-long signed.
   02 filler pic x(376).
01 ledger-select-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-select-row-count usage binary-long-long signed.
   02 filler pic x(480).
   02 ledger-select-selected-index usage binary-long-long signed.
   02 ledger-select-ordinal usage binary-long-long signed.
01 audit-cursor-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-cursor-incarnation-matches usage binary-long-long signed.
   02 audit-cursor-page usage binary-long-long signed.
   02 audit-cursor-next-page usage binary-long-long signed.
   02 audit-cursor-first-page usage binary-long-long signed.
   02 filler pic x(472).
01 audit-row-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-row-sequence usage binary-long-long signed.
   02 audit-row-before usage binary-long-long signed.
   02 filler pic x(104).
   02 audit-row-include usage binary-long-long signed.
   02 filler pic x(376).
01 audit-more-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-more-next-sequence usage binary-long-long signed.
   02 audit-more-count usage binary-long-long signed.
   02 audit-more-page usage binary-long-long signed.
   02 audit-more-first-page usage binary-long-long signed.
   02 filler pic x(88).
   02 audit-more-present usage binary-long-long signed.
   02 filler pic x(376).
01 audit-retention-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-retention-floor usage binary-long-long signed.
   02 filler pic x(112).
   02 audit-retention-truncated usage binary-long-long signed.
   02 filler pic x(376).
01 audit-initial-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-initial-head-sequence usage binary-long-long signed.
   02 filler pic x(112).
   02 audit-initial-before usage binary-long-long signed.
   02 filler pic x(376).
01 activity-frame redefines values-frame.
   02 activity-action usage binary-long-long signed.
   02 filler pic x(504).
01 activity-event-frame redefines values-frame.
   02 filler pic x(8).
   02 activity-event-tag usage binary-long-long signed.
   02 activity-event-exists usage binary-long-long signed.
   02 activity-event-name-present usage binary-long-long signed.
   02 activity-event-role-present usage binary-long-long signed.
   02 activity-event-status-present usage binary-long-long signed.
   02 activity-event-created-at usage binary-long-long signed.
   02 activity-event-good-deed usage binary-long-long signed.
   02 activity-event-step usage binary-long-long signed.
   02 activity-event-winner usage binary-long-long signed.
   02 activity-event-loan-status usage binary-long-long signed.
   02 activity-event-loan-updated usage binary-long-long signed.
   02 activity-event-actor usage binary-long-long signed.
   02 activity-event-subject usage binary-long-long signed.
   02 activity-event-owner usage binary-long-long signed.
   02 activity-event-other usage binary-long-long signed.
   02 activity-event-amount-present usage binary-long-long signed.
   02 activity-event-amount usage binary-long-long signed.
   02 activity-event-rate usage binary-long-long signed.
   02 activity-event-coins usage binary-long-long signed.
   02 activity-event-apr usage binary-long-long signed.
   02 activity-event-closes-at usage binary-long-long signed.
   02 activity-event-side usage binary-long-long signed.
   02 activity-event-lotto-kind usage binary-long-long signed.
   02 filler pic x(8).
   02 activity-event-audit-time usage binary-long-long signed.
   02 activity-event-latest-payment usage binary-long-long signed.
   02 activity-event-selected-payment usage binary-long-long signed.
   02 activity-event-creation-marker usage binary-long-long signed.
   02 filler pic x(24).
   02 activity-event-kind usage binary-long-long signed.
   02 activity-event-prefix usage binary-long-long signed.
   02 activity-event-next-actor usage binary-long-long signed.
   02 activity-event-next-other usage binary-long-long signed.
   02 activity-event-next-subject usage binary-long-long signed.
   02 activity-event-next-amount-present usage binary-long-long signed.
   02 activity-event-next-amount usage binary-long-long signed.
   02 activity-event-rate-present usage binary-long-long signed.
   02 activity-event-coins-present usage binary-long-long signed.
   02 activity-event-apr-present usage binary-long-long signed.
   02 activity-event-closes-present usage binary-long-long signed.
   02 activity-event-next-side usage binary-long-long signed.
   02 activity-event-lotto-present usage binary-long-long signed.
   02 activity-event-next-lotto-kind usage binary-long-long signed.
   02 activity-event-next-rate usage binary-long-long signed.
   02 activity-event-next-coins usage binary-long-long signed.
   02 activity-event-next-apr usage binary-long-long signed.
   02 activity-event-next-closes-at usage binary-long-long signed.
   02 filler pic x(112).
01 activity-page-frame redefines values-frame.
   02 filler pic x(8).
   02 activity-page-disabled usage binary-long-long signed.
   02 activity-page-limit usage binary-long-long signed.
   02 activity-page-after usage binary-long-long signed.
   02 activity-page-row-count usage binary-long-long signed.
   02 activity-page-capacity usage binary-long-long signed.
   02 activity-page-first-present usage binary-long-long signed.
   02 activity-page-first-sequence usage binary-long-long signed.
   02 activity-page-limit-present usage binary-long-long signed.
   02 filler pic x(184).
   02 activity-page-next-limit usage binary-long-long signed.
   02 activity-page-truncated usage binary-long-long signed.
   02 filler pic x(240).
01 activity-row-frame redefines values-frame.
   02 filler pic x(8).
   02 activity-row-sequence usage binary-long-long signed.
   02 activity-row-after usage binary-long-long signed.
   02 filler pic x(232).
   02 activity-row-visible usage binary-long-long signed.
   02 filler pic x(248).
01 activity-account-frame redefines values-frame.
   02 filler pic x(8).
   02 activity-account-self usage binary-long-long signed.
   02 activity-account-disabled usage binary-long-long signed.
   02 filler pic x(488).
01 activity-payment-frame redefines values-frame.
   02 filler pic x(8).
   02 activity-payment-row-count usage binary-long-long signed.
   02 filler pic x(480).
   02 activity-payment-selected-index usage binary-long-long signed.
   02 activity-payment-sequence usage binary-long-long signed.
01 gift-frame redefines values-frame.
   02 gift-action usage binary-long-long signed.
   02 gift-actor usage binary-long-long signed.
   02 gift-disabled usage binary-long-long signed.
   02 gift-now usage binary-long-long signed.
   02 gift-full usage binary-long-long signed.
   02 gift-exists usage binary-long-long signed.
   02 gift-owner usage binary-long-long signed.
   02 gift-closed usage binary-long-long signed.
   02 gift-deadline-present usage binary-long-long signed.
   02 gift-deadline usage binary-long-long signed.
   02 gift-target-present usage binary-long-long signed.
   02 gift-target usage binary-long-long signed.
   02 gift-title-empty usage binary-long-long signed.
   02 gift-received usage binary-long-long signed.
   02 gift-amount usage binary-long-long signed.
   02 filler pic x(40).
   02 gift-next-received usage binary-long-long signed.
   02 gift-overflow usage binary-long-long signed.
   02 gift-next-closed usage binary-long-long signed.
   02 gift-next-owner usage binary-long-long signed.
   02 filler pic x(320).
01 gift-refund-frame redefines values-frame.
   02 gift-refund-received usage binary-long-long signed.
   02 gift-refund-amount usage binary-long-long signed.
   02 gift-refund-next-received usage binary-long-long signed.
   02 filler pic x(488).
01 audit-frame redefines values-frame.
   02 audit-action usage binary-long-long signed.
   02 filler pic x(504).
01 audit-prepare-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-prepare-command usage binary-long-long signed.
   02 audit-prepare-target usage binary-long-long signed.
   02 audit-prepare-new-member usage binary-long-long signed.
   02 audit-prepare-role usage binary-long-long signed.
   02 audit-prepare-role-present usage binary-long-long signed.
   02 audit-prepare-disabled-present usage binary-long-long signed.
   02 audit-prepare-disabled usage binary-long-long signed.
   02 audit-prepare-password-present usage binary-long-long signed.
   02 audit-prepare-display-present usage binary-long-long signed.
   02 audit-prepare-cache-count usage binary-long-long signed.
   02 audit-prepare-cache-capacity usage binary-long-long signed.
   02 filler pic x(32).
   02 audit-prepare-identity usage binary-long-long signed.
   02 audit-prepare-member usage binary-long-long signed.
   02 audit-prepare-name-source usage binary-long-long signed.
   02 audit-prepare-next-role-present usage binary-long-long signed.
   02 audit-prepare-next-role usage binary-long-long signed.
   02 audit-prepare-next-disabled-present usage binary-long-long signed.
   02 audit-prepare-next-disabled usage binary-long-long signed.
   02 audit-prepare-credentials-changed usage binary-long-long signed.
   02 audit-prepare-created usage binary-long-long signed.
   02 audit-prepare-evict usage binary-long-long signed.
   02 filler pic x(304).
01 audit-check-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-check-sequence usage binary-long-long signed.
   02 audit-check-actor usage binary-long-long signed.
   02 audit-check-identity usage binary-long-long signed.
   02 audit-check-member usage binary-long-long signed.
   02 audit-check-created usage binary-long-long signed.
   02 audit-check-name-present usage binary-long-long signed.
   02 audit-check-role-present usage binary-long-long signed.
   02 audit-check-status-present usage binary-long-long signed.
   02 audit-check-credentials-changed usage binary-long-long signed.
   02 audit-check-credential-business usage binary-long-long signed.
   02 filler pic x(424).
01 art-frame redefines values-frame.
   02 art-action usage binary-long-long signed.
   02 art-actor usage binary-long-long signed.
   02 art-disabled usage binary-long-long signed.
   02 art-full usage binary-long-long signed.
   02 art-exists usage binary-long-long signed.
   02 art-owner usage binary-long-long signed.
   02 art-price-present usage binary-long-long signed.
   02 art-price usage binary-long-long signed.
   02 art-equipped usage binary-long-long signed.
   02 art-revision usage binary-long-long signed.
   02 art-expected-owner usage binary-long-long signed.
   02 art-expected-revision usage binary-long-long signed.
   02 art-expected-price usage binary-long-long signed.
   02 art-gift-target usage binary-long-long signed.
   02 art-gift-target-exists usage binary-long-long signed.
   02 art-gift-target-disabled usage binary-long-long signed.
   02 art-equip-requested usage binary-long-long signed.
   02 art-asset-valid usage binary-long-long signed.
   02 art-event-sequence usage binary-long-long signed.
   02 art-requested-price-present usage binary-long-long signed.
   02 art-requested-price usage binary-long-long signed.
   02 filler pic x(72).
   02 art-next-owner usage binary-long-long signed.
   02 art-next-price-present usage binary-long-long signed.
   02 art-next-price usage binary-long-long signed.
   02 art-next-equipped usage binary-long-long signed.
   02 art-next-revision usage binary-long-long signed.
   02 art-seller usage binary-long-long signed.
   02 art-payment-amount usage binary-long-long signed.
   02 art-clear-equipment usage binary-long-long signed.
   02 filler pic x(208).
01 art-meta-frame redefines values-frame.
   02 art-meta-title-empty usage binary-long-long signed.
   02 art-meta-permission-empty usage binary-long-long signed.
   02 art-meta-hash-length usage binary-long-long signed.
   02 art-meta-url-length usage binary-long-long signed.
   02 filler pic x(480).
01 equipment-frame redefines values-frame.
   02 equipment-actor usage binary-long-long signed.
   02 equipment-row-count usage binary-long-long signed.
   02 filler pic x(488).
   02 equipment-clear-mask usage binary-long-long signed.
01 check-frame redefines values-frame.
   02 check-action usage binary-long-long signed.
   02 filler pic x(504).
01 currency-check-frame redefines values-frame.
   02 filler pic x(8).
   02 currency-check-decimals usage binary-long-long signed.
   02 currency-check-epoch usage binary-long-long signed.
   02 currency-check-epoch-count usage binary-long-long signed.
   02 currency-check-correction-count usage binary-long-long signed.
   02 currency-check-correction-capacity usage binary-long-long signed.
   02 filler pic x(464).
01 loan-check-frame redefines values-frame.
   02 filler pic x(8).
   02 loan-check-id usage binary-long-long signed.
   02 loan-check-sequence usage binary-long-long signed.
   02 loan-check-duplicate usage binary-long-long signed.
   02 loan-check-lender usage binary-long-long signed.
   02 loan-check-borrower usage binary-long-long signed.
   02 loan-check-status usage binary-long-long signed.
   02 loan-check-lender-exists usage binary-long-long signed.
   02 loan-check-borrower-exists usage binary-long-long signed.
   02 loan-check-amount usage binary-long-long signed.
   02 loan-check-installment usage binary-long-long signed.
   02 loan-check-rate-days usage binary-long-long signed.
   02 loan-check-payment-days usage binary-long-long signed.
   02 loan-check-principal usage binary-long-long signed.
   02 loan-check-interest usage binary-long-long signed.
   02 loan-check-principal-due usage binary-long-long signed.
   02 loan-check-interest-due usage binary-long-long signed.
   02 loan-check-remainder usage binary-long-long signed.
   02 loan-check-accrued-at usage binary-long-long signed.
   02 loan-check-updated-at usage binary-long-long signed.
   02 loan-check-last-time usage binary-long-long signed.
   02 loan-check-next-due usage binary-long-long signed.
   02 filler pic x(336).
01 quote-check-frame redefines values-frame.
   02 filler pic x(8).
   02 quote-check-scale usage binary-long-long signed.
   02 quote-check-coins usage binary-long-long signed.
   02 quote-check-cents-per-coin usage binary-long-long signed.
   02 quote-check-decimals usage binary-long-long signed.
   02 filler pic x(472).
01 initial-sums-frame redefines values-frame.
   02 filler pic x(8).
   02 initial-sums-nc-issuance usage binary-long-long signed.
   02 initial-sums-lotto-escrow usage binary-long-long signed.
   02 initial-sums-usd-issuance usage binary-long-long signed.
   02 filler pic x(224).
   02 initial-sums-nc usage binary-long-long signed.
   02 initial-sums-usd usage binary-long-long signed.
   02 filler pic x(240).
01 member-sums-frame redefines values-frame.
   02 filler pic x(8).
   02 member-sums-nc usage binary-long-long signed.
   02 member-sums-usd usage binary-long-long signed.
   02 member-sums-member-nc usage binary-long-long signed.
   02 member-sums-member-usd usage binary-long-long signed.
   02 filler pic x(216).
   02 member-sums-next-nc usage binary-long-long signed.
   02 member-sums-next-usd usage binary-long-long signed.
   02 filler pic x(240).
01 conservation-frame redefines values-frame.
   02 filler pic x(8).
   02 conservation-nc usage binary-long-long signed.
   02 conservation-usd usage binary-long-long signed.
   02 filler pic x(488).
01 ledger-check-frame redefines values-frame.
   02 filler pic x(8).
   02 ledger-check-transactions usage binary-long-long signed.
   02 ledger-check-sequence usage binary-long-long signed.
   02 ledger-check-epoch usage binary-long-long signed.
   02 ledger-check-epoch-count usage binary-long-long signed.
   02 ledger-check-correction-count usage binary-long-long signed.
   02 ledger-check-correction-capacity usage binary-long-long signed.
   02 ledger-check-audit-count usage binary-long-long signed.
   02 ledger-check-audit-capacity usage binary-long-long signed.
   02 ledger-check-history-count usage binary-long-long signed.
   02 ledger-check-history-capacity usage binary-long-long signed.
   02 ledger-check-archived-count usage binary-long-long signed.
   02 ledger-check-retained-floor usage binary-long-long signed.
   02 ledger-check-audit-head usage binary-long-long signed.
   02 ledger-check-first-page usage binary-long-long signed.
   02 filler pic x(392).
01 epoch-check-frame redefines values-frame.
   02 filler pic x(8).
   02 epoch-check-index usage binary-long-long signed.
   02 epoch-check-decimals usage binary-long-long signed.
   02 epoch-check-exponent usage binary-long-long signed.
   02 epoch-check-previous-decimals usage binary-long-long signed.
   02 filler pic x(472).
01 correction-check-frame redefines values-frame.
   02 filler pic x(8).
   02 correction-check-original-id usage binary-long-long signed.
   02 correction-check-original-amount usage binary-long-long signed.
   02 correction-check-refunded usage binary-long-long signed.
   02 correction-check-full usage binary-long-long signed.
   02 correction-check-full-id usage binary-long-long signed.
   02 correction-check-duplicate usage binary-long-long signed.
   02 correction-check-original-retained usage binary-long-long signed.
   02 correction-check-original-reversal usage binary-long-long signed.
   02 correction-check-retained-amount usage binary-long-long signed.
   02 filler pic x(432).
01 history-check-frame redefines values-frame.
   02 filler pic x(8).
   02 history-check-ordinal usage binary-long-long signed.
   02 history-check-total usage binary-long-long signed.
   02 history-check-floor usage binary-long-long signed.
   02 history-check-previous-present usage binary-long-long signed.
   02 history-check-previous-ordinal usage binary-long-long signed.
   02 history-check-epoch usage binary-long-long signed.
   02 history-check-current-epoch usage binary-long-long signed.
   02 history-check-group usage binary-long-long signed.
   02 history-check-sequence usage binary-long-long signed.
   02 history-check-decimals usage binary-long-long signed.
   02 history-check-usd usage binary-long-long signed.
   02 history-check-epoch-decimals usage binary-long-long signed.
   02 history-check-refunded usage binary-long-long signed.
   02 history-check-original-amount usage binary-long-long signed.
   02 history-check-reversal usage binary-long-long signed.
   02 filler pic x(384).
01 history-tail-frame redefines values-frame.
   02 filler pic x(8).
   02 history-tail-last-present usage binary-long-long signed.
   02 history-tail-last-ordinal usage binary-long-long signed.
   02 history-tail-transactions usage binary-long-long signed.
   02 history-tail-first-page usage binary-long-long signed.
   02 filler pic x(472).
01 audit-sequence-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-sequence-sequence usage binary-long-long signed.
   02 audit-sequence-head usage binary-long-long signed.
   02 audit-sequence-time usage binary-long-long signed.
   02 audit-sequence-last-time usage binary-long-long signed.
   02 audit-sequence-previous-present usage binary-long-long signed.
   02 audit-sequence-previous-sequence usage binary-long-long signed.
   02 filler pic x(456).
01 audit-tail-frame redefines values-frame.
   02 filler pic x(8).
   02 audit-tail-head usage binary-long-long signed.
   02 audit-tail-last-sequence usage binary-long-long signed.
   02 filler pic x(488).
01 table-check-frame redefines values-frame.
   02 filler pic x(8).
   02 table-check-count usage binary-long-long signed.
   02 table-check-capacity usage binary-long-long signed.
   02 filler pic x(488).
01 gift-check-frame redefines values-frame.
   02 filler pic x(8).
   02 gift-check-owner-exists usage binary-long-long signed.
   02 gift-check-id usage binary-long-long signed.
   02 gift-check-sequence usage binary-long-long signed.
   02 gift-check-received usage binary-long-long signed.
   02 gift-check-target-present usage binary-long-long signed.
   02 gift-check-target usage binary-long-long signed.
   02 gift-check-title-blank usage binary-long-long signed.
   02 gift-check-duplicate usage binary-long-long signed.
   02 filler pic x(440).
01 art-check-frame redefines values-frame.
   02 filler pic x(8).
   02 art-check-owner-exists usage binary-long-long signed.
   02 art-check-creator-exists usage binary-long-long signed.
   02 art-check-id usage binary-long-long signed.
   02 art-check-sequence usage binary-long-long signed.
   02 art-check-revision usage binary-long-long signed.
   02 art-check-price-present usage binary-long-long signed.
   02 art-check-price usage binary-long-long signed.
   02 art-check-metadata-valid usage binary-long-long signed.
   02 art-check-duplicate usage binary-long-long signed.
   02 art-check-equipped usage binary-long-long signed.
   02 art-check-prior-equipped-count usage binary-long-long signed.
   02 filler pic x(416).
01 fulfill-check-frame redefines values-frame.
   02 filler pic x(8).
   02 fulfill-check-payment-id usage binary-long-long signed.
   02 fulfill-check-transaction-id usage binary-long-long signed.
   02 fulfill-check-payee usage binary-long-long signed.
   02 fulfill-check-provider usage binary-long-long signed.
   02 fulfill-check-payer usage binary-long-long signed.
   02 fulfill-check-recipient usage binary-long-long signed.
   02 fulfill-check-amount usage binary-long-long signed.
   02 fulfill-check-usd usage binary-long-long signed.
   02 fulfill-check-reversal usage binary-long-long signed.
   02 fulfill-check-correction-present usage binary-long-long signed.
   02 fulfill-check-reversed-status usage binary-long-long signed.
   02 fulfill-check-sequence usage binary-long-long signed.
   02 fulfill-check-provider-exists usage binary-long-long signed.
   02 fulfill-check-recipient-exists usage binary-long-long signed.
   02 fulfill-check-duplicate usage binary-long-long signed.
   02 fulfill-check-update-present usage binary-long-long signed.
   02 fulfill-check-updated-status usage binary-long-long signed.
   02 fulfill-check-current-status usage binary-long-long signed.
   02 filler pic x(360).
01 fulfill-update-frame redefines values-frame.
   02 filler pic x(8).
   02 fulfill-update-sequence usage binary-long-long signed.
   02 fulfill-update-state-sequence usage binary-long-long signed.
   02 fulfill-update-actor-exists usage binary-long-long signed.
   02 filler pic x(480).
01 wide-check-frame redefines values-frame.
   02 filler pic x(8).
   02 wide-check-encodable usage binary-long-long signed.
   02 wide-check-high usage binary-long-long signed.
   02 wide-check-low usage binary-long-long signed.
   02 wide-check-transaction-count usage binary-long-long signed.
   02 wide-check-flow-index usage binary-long-long signed.
   02 filler pic x(464).
01 epoch-tail-frame redefines values-frame.
   02 filler pic x(8).
   02 epoch-tail-present usage binary-long-long signed.
   02 epoch-tail-decimals usage binary-long-long signed.
   02 epoch-tail-state-decimals usage binary-long-long signed.
   02 filler pic x(480).
01 market-frame redefines values-frame.
   02 market-action usage binary-long-long signed.
   02 filler pic x(504).
01 listing-filter-frame redefines values-frame.
   02 filler pic x(8).
   02 listing-filter-wanted-status usage binary-long-long signed.
   02 listing-filter-status usage binary-long-long signed.
   02 filler pic x(104).
   02 listing-filter-include usage binary-long-long signed.
   02 filler pic x(376).
01 listing-select-frame redefines values-frame.
   02 filler pic x(8).
   02 listing-select-row-count usage binary-long-long signed.
   02 filler pic x(472).
   02 listing-select-index usage binary-long-long signed.
   02 listing-select-time usage binary-long-long signed.
   02 listing-select-id usage binary-long-long signed.
01 listing-merge-frame redefines values-frame.
   02 filler pic x(8).
   02 listing-merge-current-index usage binary-long-long signed.
   02 listing-merge-current-time usage binary-long-long signed.
   02 listing-merge-current-id usage binary-long-long signed.
   02 listing-merge-candidate-index usage binary-long-long signed.
   02 listing-merge-candidate-time usage binary-long-long signed.
   02 listing-merge-candidate-id usage binary-long-long signed.
   02 filler pic x(72).
   02 listing-merge-index usage binary-long-long signed.
   02 listing-merge-time usage binary-long-long signed.
   02 listing-merge-id usage binary-long-long signed.
   02 filler pic x(360).
01 profile-view-frame redefines values-frame.
   02 filler pic x(8).
   02 profile-view-disabled usage binary-long-long signed.
   02 filler pic x(496).
01 profile-offer-frame redefines values-frame.
   02 filler pic x(8).
   02 profile-offer-offerer usage binary-long-long signed.
   02 profile-offer-subject usage binary-long-long signed.
   02 profile-offer-status usage binary-long-long signed.
   02 filler pic x(96).
   02 profile-offer-include usage binary-long-long signed.
   02 filler pic x(376).
01 listing-count-frame redefines values-frame.
   02 filler pic x(8).
   02 listing-count-previous usage binary-long-long signed.
   02 listing-count-status usage binary-long-long signed.
   02 filler pic x(104).
   02 listing-count-next usage binary-long-long signed.
   02 filler pic x(376).
01 account-view-frame redefines values-frame.
   02 filler pic x(8).
   02 account-view-nana usage binary-long-long signed.
   02 account-view-actor usage binary-long-long signed.
   02 account-view-subject usage binary-long-long signed.
   02 account-view-disabled usage binary-long-long signed.
   02 filler pic x(472).
01 account-balance-frame redefines values-frame.
   02 filler pic x(8).
   02 account-balance-member usage binary-long-long signed.
   02 account-balance-found usage binary-long-long signed.
   02 account-balance-usd usage binary-long-long signed.
   02 account-balance-nc-issuance usage binary-long-long signed.
   02 account-balance-usd-issuance usage binary-long-long signed.
   02 account-balance-member-nc usage binary-long-long signed.
   02 account-balance-member-usd usage binary-long-long signed.
   02 account-balance-disabled usage binary-long-long signed.
   02 filler pic x(56).
   02 account-balance-balance usage binary-long-long signed.
   02 account-balance-issuance usage binary-long-long signed.
   02 account-balance-next-disabled usage binary-long-long signed.
   02 filler pic x(360).
01 fulfill-view-frame redefines values-frame.
   02 filler pic x(8).
   02 fulfill-view-nana usage binary-long-long signed.
   02 fulfill-view-actor usage binary-long-long signed.
   02 fulfill-view-provider usage binary-long-long signed.
   02 fulfill-view-recipient usage binary-long-long signed.
   02 filler pic x(88).
   02 fulfill-view-visible usage binary-long-long signed.
   02 filler pic x(376).
01 transaction-view-frame redefines values-frame.
   02 filler pic x(8).
   02 transaction-view-amount usage binary-long-long signed.
   02 transaction-view-actor usage binary-long-long signed.
   02 transaction-view-payer usage binary-long-long signed.
   02 transaction-view-payee usage binary-long-long signed.
   02 filler pic x(88).
   02 transaction-view-visible usage binary-long-long signed.
   02 filler pic x(376).
01 account-row-frame redefines values-frame.
   02 filler pic x(8).
   02 account-row-usd usage binary-long-long signed.
   02 account-row-wanted-usd usage binary-long-long signed.
   02 account-row-payer usage binary-long-long signed.
   02 account-row-payee usage binary-long-long signed.
   02 account-row-subject usage binary-long-long signed.
   02 filler pic x(80).
   02 account-row-include usage binary-long-long signed.
   02 filler pic x(376).
01 screen-mail-frame redefines values-frame.
   02 filler pic x(8).
   02 screen-mail-amount usage binary-long-long signed.
   02 screen-mail-usd usage binary-long-long signed.
   02 screen-mail-blank usage binary-long-long signed.
   02 screen-mail-reversal usage binary-long-long signed.
   02 screen-mail-loan usage binary-long-long signed.
   02 screen-mail-lotto usage binary-long-long signed.
   02 screen-mail-read usage binary-long-long signed.
   02 screen-mail-actor usage binary-long-long signed.
   02 screen-mail-payer usage binary-long-long signed.
   02 screen-mail-payee usage binary-long-long signed.
   02 filler pic x(424).
01 newest-frame redefines values-frame.
   02 filler pic x(8).
   02 newest-row-count usage binary-long-long signed.
   02 newest-current-index usage binary-long-long signed.
   02 newest-current-id usage binary-long-long signed.
   02 filler pic x(96).
   02 newest-index usage binary-long-long signed.
   02 newest-id usage binary-long-long signed.
   02 filler pic x(368).
01 statistics-frame redefines values-frame.
   02 statistics-action usage binary-long-long signed.
   02 filler pic x(120).
   02 statistics-result usage binary-long-long signed.
   02 statistics-first-count usage binary-long-long signed.
   02 statistics-second-count usage binary-long-long signed.
   02 statistics-second-result usage binary-long-long signed.
   02 filler pic x(352).
01 retained-year-frame redefines values-frame.
   02 filler pic x(8).
   02 retained-year-usd usage binary-long-long signed.
   02 retained-year-created-at usage binary-long-long signed.
   02 retained-year-now usage binary-long-long signed.
   02 retained-year-reversal usage binary-long-long signed.
   02 retained-year-corrected usage binary-long-long signed.
   02 filler pic x(80).
   02 retained-year-include usage binary-long-long signed.
   02 filler pic x(376).
01 employment-member-frame redefines values-frame.
   02 filler pic x(8).
   02 employment-member-disabled usage binary-long-long signed.
   02 employment-member-nana usage binary-long-long signed.
   02 filler pic x(104).
   02 employment-member-eligible usage binary-long-long signed.
   02 filler pic x(376).
01 labor-row-frame redefines values-frame.
   02 filler pic x(8).
   02 labor-row-payee usage binary-long-long signed.
   02 labor-row-member usage binary-long-long signed.
   02 labor-row-amount usage binary-long-long signed.
   02 labor-row-economic-kind usage binary-long-long signed.
   02 filler pic x(88).
   02 labor-row-include usage binary-long-long signed.
   02 filler pic x(376).
01 sale-row-frame redefines values-frame.
   02 filler pic x(8).
   02 sale-row-economic-kind usage binary-long-long signed.
   02 sale-row-thing usage binary-long-long signed.
   02 sale-row-subject usage binary-long-long signed.
   02 sale-row-quantity usage binary-long-long signed.
   02 sale-row-amount usage binary-long-long signed.
   02 filler pic x(80).
   02 sale-row-include usage binary-long-long signed.
   02 filler pic x(376).
01 inflation-frame redefines values-frame.
   02 filler pic x(8).
   02 inflation-latest-converted usage binary-long-long signed.
   02 inflation-prior-converted usage binary-long-long signed.
   02 inflation-latest-amount usage binary-long-long signed.
   02 inflation-latest-quantity usage binary-long-long signed.
   02 inflation-prior-amount usage binary-long-long signed.
   02 inflation-prior-quantity usage binary-long-long signed.
   02 inflation-total-bits usage binary-long-long signed.
   02 inflation-count usage binary-long-long signed.
   02 filler pic x(56).
   02 inflation-next-total-bits usage binary-long-long signed.
   02 inflation-next-count usage binary-long-long signed.
   02 filler pic x(368).
01 employment-frame redefines values-frame.
   02 filler pic x(8).
   02 employment-eligible usage binary-long-long signed.
   02 employment-labor-found usage binary-long-long signed.
   02 employment-eligible-count usage binary-long-long signed.
   02 employment-employed-count usage binary-long-long signed.
   02 filler pic x(88).
   02 employment-next-eligible-count usage binary-long-long signed.
   02 employment-next-employed-count usage binary-long-long signed.
   02 filler pic x(368).
01 statistics-final-frame redefines values-frame.
   02 filler pic x(8).
   02 statistics-final-inflation-total-bits usage binary-long-long signed.
   02 statistics-final-inflation-count usage binary-long-long signed.
   02 statistics-final-eligible-count usage binary-long-long signed.
   02 statistics-final-employed-count usage binary-long-long signed.
   02 filler pic x(88).
   02 statistics-final-inflation-present usage binary-long-long signed.
   02 statistics-final-inflation-bits usage binary-long-long signed.
   02 statistics-final-employment-present usage binary-long-long signed.
   02 statistics-final-employment-bits usage binary-long-long signed.
   02 filler pic x(352).
01 filled-quote-frame redefines values-frame.
   02 filler pic x(8).
   02 filled-quote-filled usage binary-long-long signed.
   02 filled-quote-coins-corrected usage binary-long-long signed.
   02 filled-quote-cash-corrected usage binary-long-long signed.
   02 filled-quote-current-present usage binary-long-long signed.
   02 filled-quote-current-time usage binary-long-long signed.
   02 filled-quote-candidate-time usage binary-long-long signed.
   02 filled-quote-candidate-rate usage binary-long-long signed.
   02 filler pic x(64).
   02 filled-quote-include usage binary-long-long signed.
   02 filler pic x(376).
01 exchange-coin-frame redefines values-frame.
   02 filler pic x(8).
   02 exchange-coin-usd usage binary-long-long signed.
   02 exchange-coin-quote usage binary-long-long signed.
   02 exchange-coin-reversal usage binary-long-long signed.
   02 exchange-coin-amount usage binary-long-long signed.
   02 exchange-coin-corrected usage binary-long-long signed.
   02 exchange-coin-current-quote-present usage binary-long-long signed.
   02 exchange-coin-current-quote-time usage binary-long-long signed.
   02 exchange-coin-row-time usage binary-long-long signed.
   02 filler pic x(56).
   02 exchange-coin-include usage binary-long-long signed.
   02 exchange-coin-stop usage binary-long-long signed.
   02 filler pic x(368).
01 exchange-cash-frame redefines values-frame.
   02 filler pic x(8).
   02 exchange-cash-usd usage binary-long-long signed.
   02 exchange-cash-quote-matches usage binary-long-long signed.
   02 exchange-cash-reversal usage binary-long-long signed.
   02 exchange-cash-amount usage binary-long-long signed.
   02 exchange-cash-corrected usage binary-long-long signed.
   02 filler pic x(80).
   02 exchange-cash-include usage binary-long-long signed.
   02 filler pic x(376).
01 exchange-display-frame redefines values-frame.
   02 filler pic x(8).
   02 exchange-display-converted usage binary-long-long signed.
   02 exchange-display-nc usage binary-long-long signed.
   02 exchange-display-cash usage binary-long-long signed.
   02 exchange-display-decimals usage binary-long-long signed.
   02 filler pic x(88).
   02 exchange-display-present usage binary-long-long signed.
   02 exchange-display-rate-bits usage binary-long-long signed.
   02 filler pic x(368).
01 quote-display-frame redefines values-frame.
   02 filler pic x(8).
   02 quote-display-cents-per-coin usage binary-long-long signed.
   02 filler pic x(112).
   02 quote-display-present usage binary-long-long signed.
   02 quote-display-rate-bits usage binary-long-long signed.
   02 filler pic x(368).
01 sales-fold-frame redefines values-frame.
   02 filler pic x(8).
   02 sales-fold-eligible usage binary-long-long signed.
   02 sales-fold-row-index usage binary-long-long signed.
   02 sales-fold-latest-index usage binary-long-long signed.
   02 sales-fold-prior-index usage binary-long-long signed.
   02 sales-fold-count usage binary-long-long signed.
   02 filler pic x(80).
   02 sales-fold-next-latest-index usage binary-long-long signed.
   02 sales-fold-next-prior-index usage binary-long-long signed.
   02 sales-fold-next-count usage binary-long-long signed.
   02 filler pic x(360).

*> Bounded table views use logical rows rather than raw word offsets.
01 nana-count-nana-table redefines values-frame.
   02 filler pic x(24).
   02 nana-count-row occurs 30.
      03 nana-count-nana usage binary-long-long signed.
      03 nana-count-disabled usage binary-long-long signed.
   02 filler pic x(8).

01 post-member-table redefines values-frame.
   02 filler pic x(80).
   02 post-member usage binary-long-long signed occurs 2.
   02 filler pic x(416).

01 loan-post-output-word-table redefines values-frame.
   02 filler pic x(128).
   02 loan-post-output-word usage binary-long-long signed occurs 15.
   02 filler pic x(264).

01 flow-output-delta-table redefines values-frame.
   02 filler pic x(64).
   02 flow-output-delta usage binary-long-long signed occurs 20.
   02 filler pic x(288).

01 flow-sum-delta-table redefines values-frame.
   02 flow-sum-delta usage binary-long-long signed occurs 20.
   02 filler pic x(352).

01 flow-sum-high-table redefines values-frame.
   02 filler pic x(160).
   02 flow-sum-row occurs 20.
      03 flow-sum-high usage binary-long-long signed.
      03 flow-sum-low usage binary-long-long signed.
   02 filler pic x(32).

01 fulfill-recycle-status-table redefines values-frame.
   02 filler pic x(8).
   02 fulfill-recycle-row occurs 31.
      03 fulfill-recycle-status usage binary-long-long signed.
      03 fulfill-recycle-history-present usage binary-long-long signed.
   02 filler pic x(8).

01 oldest-time-table redefines values-frame.
   02 oldest-row occurs 32.
      03 oldest-time usage binary-long-long signed.
      03 oldest-id usage binary-long-long signed.

01 offer-links-phase-table redefines values-frame.
   02 filler pic x(16).
   02 offer-links-row occurs 16.
      03 offer-links-phase usage binary-long-long signed.
      03 offer-links-transaction usage binary-long-long signed.
   02 filler pic x(240).

01 first-eligible-table redefines values-frame.
   02 first-eligible usage binary-long-long signed occurs 64.

01 quote-book-maker-table redefines values-frame.
   02 filler pic x(208).
   02 quote-row occurs 16.
      03 quote-book-maker usage binary-long-long signed.
      03 quote-book-live usage binary-long-long signed.
   02 filler pic x(48).

01 quote-order-side-table redefines values-frame.
   02 quote-order-row occurs 16.
      03 quote-order-side usage binary-long-long signed.
      03 quote-order-price usage binary-long-long signed.
      03 quote-order-id usage binary-long-long signed.
   02 filler pic x(128).

01 quote-order-output-index-table redefines values-frame.
   02 filler pic x(384).
   02 quote-order-output-index usage binary-long-long signed occurs 16.

01 equipment-owner-table redefines values-frame.
   02 filler pic x(16).
   02 equipment-owner usage binary-long-long signed occurs 32.
   02 filler pic x(240).

01 loan-book-lender-status-table redefines values-frame.
   02 filler pic x(16).
   02 loan-book-lender-status usage binary-long-long signed occurs 32.
   02 filler pic x(240).

01 lotto-tickets-table redefines values-frame.
   02 filler pic x(192).
   02 lotto-tickets usage binary-long-long signed occurs 32.
   02 filler pic x(64).

01 lotto-output-word-table redefines values-frame.
   02 filler pic x(448).
   02 lotto-output-word usage binary-long-long signed occurs 8.

01 lotto-view-tickets-table redefines values-frame.
   02 filler pic x(32).
   02 lotto-view-tickets usage binary-long-long signed occurs 32.
   02 filler pic x(224).

01 lotto-book-step-table redefines values-frame.
   02 filler pic x(8).
   02 lotto-book-step usage binary-long-long signed occurs 16.
   02 filler pic x(376).

01 lotto-check-tickets-table redefines values-frame.
   02 filler pic x(128).
   02 lotto-check-tickets usage binary-long-long signed occurs 32.
   02 filler pic x(128).

01 historical-exponent-table redefines values-frame.
   02 filler pic x(40).
   02 historical-exponent usage binary-long-long signed occurs 32.
   02 filler pic x(216).

01 listing-candidate-created-at-table redefines values-frame.
   02 filler pic x(16).
   02 listing-candidate-row occurs 20.
      03 listing-candidate-created-at usage binary-long-long signed.
      03 listing-candidate-id usage binary-long-long signed.
      03 listing-candidate-eligible usage binary-long-long signed.
   02 filler pic x(16).

01 newest-candidate-row-index-table redefines values-frame.
   02 filler pic x(32).
   02 newest-candidate-row occurs 30.
      03 newest-candidate-row-index usage binary-long-long signed.
      03 newest-candidate-id usage binary-long-long signed.

01 activity-payment-candidate-sequence-table redefines values-frame.
   02 filler pic x(16).
   02 activity-payment-candidate-sequence usage binary-long-long signed occurs 60.
   02 filler pic x(16).

01 ledger-candidate-ordinal-table redefines values-frame.
   02 filler pic x(16).
   02 ledger-candidate-ordinal usage binary-long-long signed occurs 60.
   02 filler pic x(16).

*> Byte views preserve binary64 display bits and raw asset text.
01 loan-summary-display-frame redefines values-frame.
   02 filler pic x(104).
   02 loan-summary-rate-bytes pic x(8).
   02 filler pic x(400).
01 inflation-input-frame redefines values-frame.
   02 filler pic x(56).
   02 inflation-total-bytes pic x(8).
   02 filler pic x(448).
01 inflation-output-frame redefines values-frame.
   02 filler pic x(128).
   02 inflation-next-total-bytes pic x(8).
   02 filler pic x(376).
01 statistics-input-frame redefines values-frame.
   02 filler pic x(8).
   02 statistics-total-bytes pic x(8).
   02 filler pic x(496).
01 statistics-display-frame redefines values-frame.
   02 filler pic x(136).
   02 statistics-rate-bytes pic x(8).
   02 filler pic x(8).
   02 statistics-employment-bytes pic x(8).
   02 filler pic x(352).
01 asset-text-frame redefines values-frame.
   02 filler pic x(32).
   02 asset-hash pic x(64).
   02 asset-url pic x(192).
   02 filler pic x(224).

01 result-code usage binary-long signed.
procedure division using operation-code values-frame result-code.
    move result-success to result-code
    evaluate operation-code
        when op-posting-plan perform posting-plan
        when op-exact-rescale perform exact-rescale
        when op-exchange-cents perform exchange-cents
        when op-annual-rate perform annual-rate
        when op-accrue-interest perform accrue-interest
        when op-settle-loan perform settle-loan
        when op-reform-loan-fraction perform reform-loan-fraction
        when op-loan-posting-plan perform loan-posting-plan
        when op-identity-query-policy perform identity-query-policy
        when op-administrator-policy perform administrator-policy
        when op-protect-last-nana perform protect-last-nana
        when op-key-policy perform key-policy
        when op-member-grant perform member-grant
        when op-configuration-policy perform configuration-policy
        when op-loan-policy perform loan-policy
        when op-loan-projection perform loan-projection
        when op-member-update-policy perform member-update-policy
        when op-loan-book perform loan-book
        when op-loan-summary perform loan-summary
        when op-fulfillment-transition perform fulfillment-transition
        when op-offer-projection perform offer-projection
        when op-offer-transition perform offer-transition
        when op-member-import-policy perform member-import-policy
        when op-oldest-eligible-record perform oldest-eligible-record
        when op-reversal-offer-links perform reversal-offer-links
        when op-listing-transition perform listing-transition
        when op-first-eligible-record perform first-eligible-record
        when op-recycling-policy perform recycling-policy
        when op-identity-transition perform identity-transition
        when op-classify-flows perform classify-flows
        when op-accumulate-flows perform accumulate-flows
        when op-lotto-transition perform lotto-transition
        when op-lotto-projection perform lotto-projection
        when op-lotto-book perform lotto-book
        when op-lotto-invariants perform lotto-invariants
        when op-reform-policy perform reform-policy
        when op-reform-lotto-consistency perform reform-lotto-consistency
        when op-reform-field perform reform-field
        when op-account-posting perform account-posting
        when op-correction-policy perform correction-policy
        when op-historical-amount perform historical-amount
        when op-transfer-policy perform transfer-policy
        when op-actor-policy perform actor-policy
        when op-new-member-policy perform new-member-policy
        when op-identity-text-policy perform identity-text-policy
        when op-correction-plan perform correction-plan
        when op-fulfillment-capacity perform fulfillment-capacity
        when op-fulfillment-creation perform fulfillment-creation
        when op-fulfillment-recycling perform fulfillment-recycling
        when op-quote-projection perform quote-projection
        when op-quote-transition perform quote-transition
        when op-quote-ordering perform quote-ordering
        when op-ledger-projection perform ledger-projection
        when op-activity-projection perform activity-projection
        when op-gift-transition perform gift-transition
        when op-gift-refund perform gift-refund
        when op-audit-policy perform audit-policy
        when op-art-transition perform art-transition
        when op-art-metadata perform art-metadata
        when op-art-equipment perform art-equipment
        when op-bank-invariants perform bank-invariants
        when op-market-view perform market-view
        when op-household-statistics perform household-statistics
        when other move result-invalid to result-code
    end-evaluate
    goback.

identity-query-policy.
    *> Only eligibility facts enter this module; hashes and tokens stay Rust.
    *> The count fold fills the frame, including output positions. Preserve
    *> those input pairs until the complete fold has been read.
    if identity-query-action not = 6 move 0 to identity-query-eligible identity-query-created-at end-if
    evaluate identity-query-action
        when select-identity-delegated-view
            *> Self, actor Nana, target exists, target bot; delegated-view flag.
            if delegated-self = 0
                if delegated-actor-nana = 0 move result-forbidden to result-code exit paragraph end-if
                if delegated-target-exists = 0 move result-not-found to result-code exit paragraph end-if
                if delegated-target-bot = 0 move result-forbidden to result-code exit paragraph end-if
                move 1 to identity-query-eligible
            end-if
        when select-identity-key-scope
            *> Delegated target, read scope, mutation.
            if key-scope-delegated-target = 1 and key-scope-read-only = 1 and key-scope-mutation = 1 move
                result-forbidden to result-code end-if
        when select-identity-key-member
            *> Matched key member exists, disabled, has login; eligible flag.
            if key-member-exists = 1 and key-member-disabled = 0 and key-member-has-login = 1 move 1 to
                identity-query-eligible end-if
        when select-identity-key-view
            *> Stored key present and creation time; active/time projection.
            if key-view-present = 1 move 1 to identity-query-eligible move key-view-created-at to
                identity-query-created-at end-if
        when select-identity-wire-epoch
            *> Wire mutation, reformed bank, reform route, matching epoch.
            if wire-epoch-mutation = 1 and wire-epoch-reformed = 1 and wire-epoch-reform-route = 0 and
                wire-epoch-epoch-matches = 0
                move result-stale to result-code end-if
        when select-identity-command-epoch
            *> Durable retry already checked: fresh command epoch must match.
            if command-epoch-matches = 0 move result-stale to result-code end-if
        when select-identity-nana-fold
            *> Active Nana fold: previous count, row count, Nana/disabled pairs.
            if nana-count-previous < 0 or nana-count-previous > 32 or nana-count-row-count < 0 or
                nana-count-row-count > 30
                move result-invalid to result-code exit paragraph end-if
            move nana-count-previous to selection-best
            perform varying selection-index from 1 by 1 until selection-index > nana-count-row-count
                if nana-count-nana(selection-index) = 1 and nana-count-disabled(selection-index) = 0
                    add 1 to selection-best end-if
            end-perform
            if selection-best > 32 move result-invalid to result-code
            else move selection-best to identity-query-eligible end-if
        when other move result-invalid to result-code
    end-evaluate.

loan-posting-plan.
    *> Draw flag/amount, principal/interest paid, lender/borrower.
    *> Output 17 count; 18..22 and 23..27 from/to/amount/kind/tx offset.
    *> Outputs 28..31 aggregate validation amount/from/to/present.
    if (loan-post-draw not = 0 and loan-post-draw not = 1) or loan-post-draw-amount < 0 or
        loan-post-principal-paid < 0 or loan-post-interest-paid < 0
        move result-invalid to result-code exit paragraph end-if
    initialize loan-post-output-word-table
    if loan-post-draw = 1
        if loan-post-draw-amount > 0
            move 1 to loan-post-posting-count move loan-post-lender to loan-post-first-payer move
                loan-post-borrower to loan-post-first-payee
            move loan-post-draw-amount to loan-post-first-amount move 3 to loan-post-first-kind
        end-if
    else
        if loan-post-principal-paid > 0
            move 1 to loan-post-posting-count move loan-post-borrower to loan-post-first-payer move
                loan-post-lender to loan-post-first-payee
            move loan-post-principal-paid to loan-post-first-amount move 3 to loan-post-first-kind
        end-if
        if loan-post-interest-paid > 0
            if loan-post-posting-count = 0
                move 1 to loan-post-posting-count move loan-post-borrower to loan-post-first-payer move
                    loan-post-lender to loan-post-first-payee
                move loan-post-interest-paid to loan-post-first-amount move 4 to loan-post-first-kind
            else
                move 2 to loan-post-posting-count move loan-post-borrower to loan-post-second-payer move
                    loan-post-lender to loan-post-second-payee
                move loan-post-interest-paid to loan-post-second-amount move 4 to loan-post-second-kind move
                    4096 to loan-post-second-offset
            end-if
        end-if
    end-if
    if loan-post-draw = 1
        move loan-post-draw-amount to wide-value move loan-post-lender to loan-post-aggregate-payer move
            loan-post-borrower to loan-post-aggregate-payee
    else
        compute wide-value = loan-post-principal-paid + loan-post-interest-paid
        move loan-post-borrower to loan-post-aggregate-payer move loan-post-lender to
            loan-post-aggregate-payee
    end-if
    if wide-value > max-i64 move result-overflow to result-code exit paragraph end-if
    move wide-value to loan-post-aggregate-amount
    if wide-value > 0 move 1 to loan-post-aggregate-present end-if.

posting-plan.
    *> Inputs: source, target, units, issuance, correction, USD;
    *> slot 9 = prepared publication mode, slot 10 = same-wallet fact.
    *> Slots 11/12 parties, 13 loan-reference fact, 14 incoming credit mask.
    *> Outputs: debit/credit in 7/8, cash leg in 17, next credit mask in 18.
    if operation-code = op-posting-plan and post-next-state = 1 and (post-credit-mask < 0 or
        post-credit-mask > max-u32)
        move result-invalid to result-code exit paragraph end-if
    if operation-code = op-posting-plan move 1 to post-cash-leg end-if
    if operation-code = op-posting-plan and post-next-state = 1 and post-amount = 0
        move post-source-balance to post-next-source-balance move post-target-balance to
            post-next-target-balance
        move 0 to post-cash-leg perform posting-credit-policy exit paragraph
    end-if
    if post-amount < 1 or post-amount > command-amount-limit
        move result-invalid to result-code
        exit paragraph
    end-if
    if operation-code = op-posting-plan and post-next-state = 1 and post-same-wallet = 1
        if post-source-balance not = post-target-balance move result-invalid to result-code exit paragraph
            end-if
        move post-source-balance to post-next-source-balance move post-target-balance to
            post-next-target-balance
        perform posting-credit-policy
        exit paragraph
    end-if
    compute debit-value = post-source-balance - post-amount
    compute credit-value = post-target-balance + post-amount
    if debit-value < -9007199254740991
        or credit-value > wallet-limit
        or (post-usd = 1 and
            (debit-value > wallet-limit
             or credit-value < -9007199254740991))
        move result-overflow to result-code
        exit paragraph
    end-if
    if post-correction = 0 and post-issuance = 0 and debit-value < 0
        move result-insufficient-funds to result-code
        exit paragraph
    end-if
    move debit-value to post-next-source-balance
    move credit-value to post-next-target-balance
    perform posting-credit-policy.

posting-credit-policy.
    *> Nested account/transfer validation retains its own metadata slots.
    if operation-code not = op-posting-plan exit paragraph end-if
    move post-credit-mask to post-next-credit-mask
    if post-next-state = 1 and post-usd = 0 and post-amount > 0
        perform varying selection-index from 1 by 1 until selection-index > 2
            if post-member(selection-index) >= 1 and post-member(selection-index) <= 32
                compute factor-value = 2 ** (post-member(selection-index) - 1)
                divide post-next-credit-mask by factor-value giving quotient-value
                compute remainder-value = function mod(quotient-value, 2)
                if post-loan-linked = 1
                    if remainder-value = 0 add factor-value to post-next-credit-mask end-if
                else
                    if remainder-value = 1 subtract factor-value from post-next-credit-mask end-if
                end-if
            end-if
        end-perform
    end-if.

exact-rescale.
    *> Inputs: units, decimal exponent, absolute result limit.
    *> Output: exact converted units in slot 4; never round money.
    if rescale-units = 0
        move 0 to rescale-converted-units
        exit paragraph
    end-if
    if rescale-exponent < -38 or rescale-exponent > 38
        move result-overflow to result-code
        exit paragraph
    end-if
    if rescale-exponent = -38
        move result-conflict to result-code
        exit paragraph
    end-if
    if rescale-exponent = 38
        move result-overflow to result-code
        exit paragraph
    end-if
    compute exponent-value = function abs(rescale-exponent)
    compute factor-value = 10 ** exponent-value
        on size error move result-overflow to result-code
    end-compute
    if result-code not = result-success exit paragraph end-if
    if rescale-exponent >= 0
        compute wide-value = rescale-units * factor-value
            on size error move result-overflow to result-code
        end-compute
    else
        divide rescale-units by factor-value giving quotient-value
            remainder remainder-value
        if remainder-value not = 0
            move result-conflict to result-code
            exit paragraph
        end-if
        move quotient-value to wide-value
    end-if
    if result-code not = result-success exit paragraph end-if
    if function abs(wide-value) > rescale-limit
        move result-overflow to result-code
        exit paragraph
    end-if
    move wide-value to rescale-converted-units.

exchange-cents.
    *> Inputs: coins, cents per coin, NC scale. Output cents slot 4.
    if exchange-nc-scale <= 0
        move result-invalid to result-code
        exit paragraph
    end-if
    compute wide-value = exchange-coins * exchange-cents-per-coin
        on size error move result-overflow to result-code
    end-compute
    if result-code not = result-success exit paragraph end-if
    divide wide-value by exchange-nc-scale giving quotient-value
    if quotient-value < min-i64
        or quotient-value > max-i64
        move result-overflow to result-code
        exit paragraph
    end-if
    move quotient-value to exchange-cash-cents.

annual-rate.
    *> Inputs: basis points per period, period days. Output slot 3.
    compute denominator-value = function max(1, annual-period-days)
    compute quotient-value = annual-period-basis-points * 365 / denominator-value
    compute annual-basis-points = function min(quotient-value, max-u32).

accrue-interest.
    *> Inputs: principal, rate bps, rate days, now, accrued at,
    *> interest, fractional remainder. Outputs interest/remainder/time 8-10.
    if accrual-now < accrual-accrued-at
        move result-unavailable to result-code
        exit paragraph
    end-if
    compute denominator-value = basis-points-per-whole * accrual-rate-days * seconds-per-day
    if denominator-value = 0
        move result-invalid to result-code
        exit paragraph
    end-if
    compute wide-value = accrual-principal * accrual-rate-basis-points * (accrual-now - accrual-accrued-at)
        + accrual-remainder
        on size error move result-overflow to result-code
    end-compute
    if result-code not = result-success exit paragraph end-if
    divide wide-value by denominator-value giving quotient-value
        remainder remainder-value
    compute credit-value = accrual-interest + quotient-value
    if credit-value + accrual-principal > wallet-limit
        move result-overflow to result-code
        exit paragraph
    end-if
    move credit-value to accrual-next-interest
    move remainder-value to accrual-next-remainder
    move accrual-now to accrual-next-accrued-at
    move denominator-value to accrual-denominator.

administrator-policy.
    *> Empty bank, actor id, actor exists, Nana, disabled.
    if (admin-empty-bank = 1 and admin-actor = 1)
        or (admin-actor-exists = 1 and admin-actor-nana = 1 and admin-actor-disabled = 0)
        continue
    else
        move result-forbidden to result-code
    end-if.

protect-last-nana.
    *> Current Nana/disabled, demoting/disabling, active Nana count.
    if last-nana-current-nana = 1 and last-nana-current-disabled = 0 and last-nana-removing-nana = 1
        and last-nana-active-count = 1
        move result-forbidden to result-code
    end-if.

key-policy.
    *> Self, administrator, revoke, bot full-key, has login, disabled.
    if key-self = 0 and (key-administrator = 0 or
        (key-revoke = 0 and key-bot-full-key = 0))
        move result-forbidden to result-code
        exit paragraph
    end-if
    if key-revoke = 0 and (key-has-login = 0 or key-disabled = 1)
        move result-forbidden to result-code
    end-if.

member-grant.
    *> Issuance balance and initial grant; zero is allowed.
    if grant-amount < 0 or grant-amount > command-amount-limit
        move result-invalid to result-code
        exit paragraph
    end-if
    compute debit-value = grant-issuance-balance - grant-amount
    if debit-value < -9007199254740991
        move result-overflow to result-code
    end-if.

configuration-policy.
    *> Initial grant, optional settlement interval (-1 absent), now.
    if config-initial-grant < 0 or config-initial-grant > command-amount-limit
        move result-invalid to result-code
        exit paragraph
    end-if
    compute wide-value = function max(0, wallet-limit - config-now)
    if config-settlement-interval > wide-value move result-invalid to result-code end-if.

settle-loan.
    *> Status/manual/request/now/wallet/credit-block; loan numeric fields
    *> 7-13; amount/rate/rate days/payment days/installment in 15-19.
    *> Outputs 21-33 are the next loan and its principal/interest cash legs.
    *> Raw borrower/lender disabled facts in slots 20 and 34.
    if settle-now < minimum-clock or settle-now > wallet-limit
        move result-unavailable to result-code exit paragraph end-if
    if settle-borrower-disabled = 1 or settle-lender-disabled = 1
        move result-disabled to result-code exit paragraph end-if
    move settle-principal to loan-principal
    move settle-interest to loan-interest
    move settle-remainder to loan-remainder
    move settle-accrued-at to loan-accrued
    move settle-next-due to loan-next-due
    move settle-principal-due to loan-principal-due
    move settle-interest-due to loan-interest-due
    move 0 to loan-paid loan-interest-paid loan-principal-paid settle-draw-amount
    compute loan-interval = settle-payment-days * seconds-per-day
    if settle-status = 2 and settle-manual = 0
        if settle-wallet not = 0 or settle-credit-blocked not = 0
            move result-conflict to result-code
            exit paragraph
        end-if
        compute wide-value = settle-now + loan-interval
        if wide-value > wallet-limit
            move result-overflow to result-code
            exit paragraph
        end-if
        move wide-value to loan-next-due
        move settle-amount to loan-principal
        move settle-now to loan-accrued
        move 1 to settle-next-status settle-draw-amount
        perform publish-loan
        exit paragraph
    end-if
    if settle-status not = 1
        move result-conflict to result-code
        exit paragraph
    end-if
    if settle-now < loan-accrued
        move result-unavailable to result-code
        exit paragraph
    end-if
    compute denominator-value = basis-points-per-whole * settle-rate-days * seconds-per-day
    compute wide-value = loan-principal * settle-rate-basis-points
        * (settle-now - loan-accrued) + loan-remainder
        on size error move result-overflow to result-code
    end-compute
    if result-code not = result-success exit paragraph end-if
    divide wide-value by denominator-value giving quotient-value
        remainder remainder-value
    compute credit-value = loan-interest + quotient-value
    if credit-value + loan-principal > wallet-limit
        move result-overflow to result-code
        exit paragraph
    end-if
    move credit-value to loan-interest
    move remainder-value to loan-remainder
    move settle-now to loan-accrued
    if settle-now >= loan-next-due
        compute loan-periods = (settle-now - loan-next-due) / loan-interval
        add 1 to loan-periods
        compute wide-value = loan-principal-due
            + loan-periods * settle-installment
        compute loan-principal-due = function min(wide-value, loan-principal)
        move loan-interest to loan-interest-due
        compute wide-value = loan-next-due + loan-periods * loan-interval
        if wide-value > wallet-limit
            move result-overflow to result-code
            exit paragraph
        end-if
        move wide-value to loan-next-due
    end-if
    if settle-manual = 1
        if settle-requested-amount < 1 or settle-requested-amount > command-amount-limit
            move result-invalid to result-code
            exit paragraph
        end-if
        compute loan-requested = function min(settle-requested-amount,
            loan-principal + loan-interest)
        if settle-wallet < loan-requested
            move result-insufficient-funds to result-code
            exit paragraph
        end-if
    else
        compute loan-requested = function min(command-amount-limit,
            loan-principal-due + loan-interest-due)
    end-if
    compute loan-paid = function min(loan-requested, function max(0, settle-wallet))
    compute loan-interest-paid = function min(loan-paid, loan-interest)
    compute loan-principal-paid = loan-paid - loan-interest-paid
    subtract loan-interest-paid from loan-interest
    subtract loan-principal-paid from loan-principal
    compute loan-interest-due = function max(0,
        loan-interest-due - loan-interest-paid)
    compute loan-principal-due = function max(0,
        loan-principal-due - loan-principal-paid)
    move 1 to settle-next-status
    if loan-principal = 0 and loan-interest = 0
        move 3 to settle-next-status
        move 0 to loan-remainder loan-next-due
    end-if
    perform publish-loan.

publish-loan.
    move loan-principal to settle-next-principal
    move loan-interest to settle-next-interest
    move loan-remainder to settle-next-remainder
    move loan-accrued to settle-next-accrued-at
    move loan-next-due to settle-next-due-at
    move loan-principal-due to settle-next-principal-due
    move loan-interest-due to settle-next-interest-due
    compute settle-next-wallet = settle-wallet - loan-paid
    move settle-now to settle-updated-at
    move loan-principal-paid to settle-principal-paid
    move loan-interest-paid to settle-interest-paid.

reform-loan-fraction.
    *> Interest, remainder, denominator, exponent, converted principal.
    *> Keep the fractional claim exact when reforming the economy.
    if loan-reform-exponent < -24 or loan-reform-exponent > 24 or loan-reform-denominator <= 0
        move result-invalid to result-code
        exit paragraph
    end-if
    compute factor-value = 10 ** function abs(loan-reform-exponent)
    compute wide-value = loan-reform-interest * loan-reform-denominator + loan-reform-remainder
    if loan-reform-exponent >= 0
        compute wide-value = wide-value * factor-value
            on size error move result-overflow to result-code
        end-compute
        if result-code not = result-success exit paragraph end-if
    else
        divide wide-value by factor-value giving quotient-value
            remainder remainder-value
        if remainder-value not = 0
            move result-conflict to result-code
            exit paragraph
        end-if
        move quotient-value to wide-value
    end-if
    divide wide-value by loan-reform-denominator giving quotient-value
        remainder remainder-value
    if quotient-value + loan-reform-converted-principal > wallet-limit
        move result-overflow to result-code
        exit paragraph
    end-if
    move quotient-value to loan-reform-converted-interest
    move remainder-value to loan-reform-converted-remainder.

fulfillment-transition.
    *> Status/action/actor/provider/recipient/empty-reason/control-byte.
    *> Complete is allowed to either party; only recipient disputes.
    if fulfill-reason-control = 1
        move result-invalid to result-code
        exit paragraph
    end-if
    if fulfill-actor not = fulfill-recipient and
        (fulfill-action not = 0 or fulfill-actor not = fulfill-provider)
        move result-forbidden to result-code
        exit paragraph
    end-if
    evaluate fulfill-action
        when 0
            if fulfill-status not = 0 move result-conflict to result-code end-if
            move 1 to fulfill-next-status
        when 1
            if fulfill-status not = 0 and fulfill-status not = 1
                move result-conflict to result-code
            end-if
            move 2 to fulfill-next-status
        when 2
            if fulfill-status not = 2 move result-conflict to result-code end-if
            move 1 to fulfill-next-status
        when other move result-invalid to result-code
    end-evaluate
    if result-code not = result-success exit paragraph end-if
    if fulfill-action = 1 and fulfill-reason-empty = 1 move result-invalid to result-code end-if.

classify-flows.
    *> Amount/from/to/USD/quote/lotto/reversal/economic kind.
    *> Output slots 9-28: the twenty ledger flow deltas.
    initialize flow-deltas
    move 1 to flow-delta(20) flow-sign
    if flow-reversal = 1 move -1 to flow-sign end-if
    move 0 to nana-direction inbound-index outbound-index
    if flow-payee = 1 move 1 to nana-direction
    else if flow-payer = 1 move -1 to nana-direction end-if end-if
    if flow-usd = 1
        evaluate true
            when flow-payer = 0 move flow-amount to flow-delta(7)
            when flow-payee = 0 compute flow-delta(7) = 0 - flow-amount
            when flow-quote = 1
                move 11 to inbound-index
                move 9 to outbound-index
        end-evaluate
    else
        evaluate true
            when flow-payer = 0 or flow-payee = 0
                if flow-payer = 0 move flow-amount to flow-delta(6)
                else compute flow-delta(6) = 0 - flow-amount end-if
                evaluate true
                    when flow-reversal = 1 move flow-delta(6) to flow-delta(5)
                    when flow-payee = 0 move flow-amount to flow-delta(4)
                    when flow-lotto = 1 move flow-amount to flow-delta(3)
                    when flow-payee = 1 move flow-amount to flow-delta(1)
                    when other move flow-amount to flow-delta(2)
                end-evaluate
            when flow-quote = 1
                move 8 to inbound-index
                move 10 to outbound-index
            when flow-economic-kind = 4
                move 12 to inbound-index
                move 13 to outbound-index
            when flow-economic-kind = 3
                move 15 to inbound-index
                move 14 to outbound-index
            when flow-economic-kind = 1 or flow-economic-kind = 2
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
        compute flow-delta(flow-index) = flow-amount * flow-sign
    end-if
    perform varying flow-index from 1 by 1 until flow-index > 20
        move flow-delta(flow-index) to flow-output-delta(flow-index)
    end-perform.

accumulate-flows.
    *> Slots 1-20: deltas. Slots 21-60: signed high/low pairs in base 10^15.
    *> Update pairs in place, without exposing Rust's i128 representation.
    perform varying flow-index from 1 by 1 until flow-index > 20
        compute wide-value = flow-sum-high(flow-index) * flow-parts-base
            + flow-sum-low(flow-index) + flow-sum-delta(flow-index)
        if function abs(wide-value) >= 20282409603651670423947251286016
            move result-overflow to result-code
            exit paragraph
        end-if
        divide wide-value by flow-parts-base giving quotient-value
            remainder remainder-value
        move quotient-value to flow-sum-high(flow-index)
        move remainder-value to flow-sum-low(flow-index)
    end-perform.

correction-plan.
    *> Original units, previously refunded units, this correction's units,
    *> correction transaction id. Outputs refunded units and full-link id.
    if correction-original-amount <= 0 or correction-already-refunded < 0 or correction-amount <= 0
        move result-conflict to result-code
        exit paragraph
    end-if
    compute wide-value = correction-already-refunded + correction-amount
    if wide-value > correction-original-amount
        move result-conflict to result-code
        exit paragraph
    end-if
    move wide-value to correction-next-refunded
    move 0 to correction-full-correction-id
    if wide-value = correction-original-amount move correction-transaction-id to
        correction-full-correction-id end-if.

offer-projection.
    *> Phase/now/settles-at/viewer/Nana/disabled/owner/offerer.
    *> Outputs status/reversible/visible/recyclable/pinned in slots 9-13.
    move offer-view-phase to offer-view-status
    *> Slot 14 optionally supplies listing liveness (-1 means no joined view).
    if offer-view-phase = 0 and offer-view-listing-live = 0 move 6 to offer-view-status end-if
    move 0 to offer-view-reversible offer-view-visible offer-view-recyclable offer-view-pinned
    if offer-view-phase = 1
        if offer-view-now >= offer-view-settles-at
            move 5 to offer-view-status
            move 1 to offer-view-recyclable
        else
            move 1 to offer-view-pinned
            if offer-view-now >= minimum-clock move 1 to offer-view-reversible end-if
        end-if
    else
        if offer-view-phase not = 0 move 1 to offer-view-recyclable end-if
    end-if
    if offer-view-disabled = 0 and (offer-view-nana = 1 or offer-view-viewer = offer-view-owner
        or offer-view-viewer = offer-view-offerer) move 1 to offer-view-visible end-if.

offer-transition.
    *> Action 0 make, 1 accept, 2 undo, 3 decline, 4 withdraw.
    *> Input record is documented in Rust's offer_plan adapter.
    *> Outputs phase/payer/payee/deadline/overflow/listing-mutation 31-36.
    if offer-now < minimum-clock or offer-now > wallet-limit
        move result-unavailable to result-code
        exit paragraph
    end-if
    move 0 to offer-next-phase offer-payer offer-payee offer-deadline offer-overflow offer-mutate-listing
    if offer-action = 0
        if offer-amount < 1 or offer-amount > command-amount-limit or offer-memo-control = 1
            move result-invalid to result-code
            exit paragraph
        end-if
        if offer-listing-exists = 0 move result-not-found to result-code exit paragraph end-if
        if offer-listing-active = 0 move result-listing-closed to result-code exit paragraph end-if
        if offer-actor = offer-listing-owner move result-self-deal to result-code exit paragraph end-if
        if offer-good-deed = 1 and offer-bot = 1
            move result-bot-good-deed to result-code exit paragraph
        end-if
        if offer-book-full = 1 and offer-recyclable = 0 move result-capacity to result-code end-if
        exit paragraph
    end-if
    if offer-exists = 0 move result-not-found to result-code exit paragraph end-if
    evaluate offer-action
        when 1
            if offer-actor not = offer-owner
                move result-forbidden to result-code exit paragraph
            end-if
            if offer-phase not = 0 move result-offer-closed to result-code exit paragraph end-if
            if offer-listing-exists = 0 move result-not-found to result-code exit paragraph end-if
            if offer-listing-active = 0 move result-listing-closed to result-code exit paragraph end-if
            if offer-parties-disabled = -1 move result-not-found to result-code exit paragraph end-if
            if offer-parties-disabled = 1 move result-disabled to result-code exit paragraph end-if
            if offer-labor = 1 and
                (((offer-good-deed = 1 or offer-buy-side = 1) and offer-offerer-nana = 1) or
                 (offer-good-deed = 0 and offer-buy-side = 0 and offer-owner-nana = 1))
                move result-forbidden to result-code exit paragraph
            end-if
            evaluate true
                when offer-good-deed = 1
                    move 0 to offer-payer
                    move offer-offerer to offer-payee
                when offer-buy-side = 1
                    move offer-owner to offer-payer
                    move offer-offerer to offer-payee
                when other
                    move offer-offerer to offer-payer
                    move offer-owner to offer-payee
            end-evaluate
            move 1 to offer-next-phase
            compute wide-value = offer-now + offer-reversal-window
            *> Funds are checked before this deferred overflow result.
            if wide-value > wallet-limit move 1 to offer-overflow
            else move wide-value to offer-deadline end-if
            if offer-good-deed = 0 move 1 to offer-mutate-listing end-if
        when 2
            if offer-actor not = offer-owner and offer-actor not = offer-offerer
                and offer-nana = 0 move result-forbidden to result-code exit paragraph end-if
            if offer-phase not = 1 move result-offer-closed to result-code exit paragraph end-if
            if offer-now >= offer-settles-at move result-settled to result-code exit paragraph end-if
            if offer-memo-control = 1 move result-invalid to result-code exit paragraph end-if
            if offer-original-payer not = 0
                if offer-listing-exists = 0 move result-not-found to result-code exit paragraph end-if
                if offer-listing-sold = 0 move result-conflict to result-code exit paragraph end-if
                move 1 to offer-mutate-listing
            end-if
            if offer-correction-room = 0 move result-capacity to result-code exit paragraph end-if
            if offer-refunded not = 0 or offer-reversed = 1
                move result-conflict to result-code exit paragraph
            end-if
            move 4 to offer-next-phase
            move offer-original-payee to offer-payer
            move offer-original-payer to offer-payee
        when 3 when 4
            if offer-phase not = 0 move result-offer-closed to result-code exit paragraph end-if
            if offer-action = 3
                move offer-owner to offer-payer
                move 2 to offer-next-phase
            else
                move offer-offerer to offer-payer
                move 3 to offer-next-phase
            end-if
            if offer-actor not = offer-payer and offer-nana = 0
                move result-forbidden to result-code
            end-if
        when other move result-invalid to result-code
    end-evaluate.

fulfillment-capacity.
    *> Command category: 0 other, 1 purchase/accept, 2 classified transfer.
    *> amount, physical economic kind (1 work/2 goods), full, recyclable.
    if (fulfill-room-command-category = 1 or (fulfill-room-command-category = 2 and fulfill-room-amount > 0
        and
        (fulfill-room-economic-kind = 1 or fulfill-room-economic-kind = 2))) and
        fulfill-room-full = 1 and fulfill-room-recyclable = 0
        move result-capacity to result-code
    end-if.

fulfillment-creation.
    *> reversal, full correction, USD, amount, loan/lotto/quote/art,
    *> payer, listing present, physical kind, cash/service details.
    *> Outputs: action (skip/create/reverse), kind (work/goods/cash), status.
    move 0 to fulfill-create-action fulfill-create-kind fulfill-create-status
    if fulfill-create-reversal = 1
        if fulfill-create-full-correction = 1 move 2 to fulfill-create-action move 3 to
            fulfill-create-status end-if
        exit paragraph
    end-if
    if fulfill-create-usd = 1 or fulfill-create-amount = 0 or fulfill-create-loan = 1 or
        fulfill-create-lotto = 1 or fulfill-create-quote = 1 or fulfill-create-art = 1 or
            fulfill-create-payer = 0
        exit paragraph
    end-if
    if fulfill-create-listing-present = 0 and fulfill-create-economic-kind not = 1 and
        fulfill-create-economic-kind not = 2
        exit paragraph
    end-if
    move 1 to fulfill-create-action
    evaluate true
        when fulfill-create-cash-kind = 1 move 2 to fulfill-create-kind
        when fulfill-create-economic-kind = 1 or fulfill-create-service-kind = 1 move 0 to
            fulfill-create-kind
        when other move 1 to fulfill-create-kind
    end-evaluate.

fulfillment-recycling.
    *> Count <=31, then status/history-present pairs. First eligible index.
    if fulfill-recycle-row-count < 0 or fulfill-recycle-row-count > 31
        move result-invalid to result-code exit paragraph
    end-if
    move -1 to fulfill-recycle-selected-index
    perform varying selection-index from 1 by 1 until selection-index > fulfill-recycle-row-count
        if (fulfill-recycle-status(selection-index) = 1 or fulfill-recycle-status(selection-index) = 3) and
            fulfill-recycle-history-present(selection-index) = 0
            compute fulfill-recycle-selected-index = selection-index - 1
            exit perform
        end-if
    end-perform.

oldest-eligible-record.
    *> Thirty-two timestamp/id pairs; a timestamp of -1 is ineligible.
    *> Output slot 1 is the selected zero-based index or -1.
    move -1 to selection-best
    perform varying selection-index from 1 by 1 until selection-index > 32
        if oldest-time(selection-index) >= 0
            if selection-best = -1 or oldest-time(selection-index) < selection-time
                or (oldest-time(selection-index) = selection-time
                    and oldest-id(selection-index) < selection-key)
                compute selection-best = selection-index - 1
                move oldest-time(selection-index) to selection-time
                move oldest-id(selection-index) to selection-key
            end-if
        end-if
    end-perform
    move selection-best to oldest-selected-index.

reversal-offer-links.
    *> Original transaction/count, then at most sixteen phase/transaction pairs.
    *> Output slot 35 is the bit mask of accepted offers linked to a full refund.
    if offer-links-row-count < 0 or offer-links-row-count > 16
        move result-invalid to result-code exit paragraph
    end-if
    move 0 to offer-links-reversed-mask
    perform varying selection-index from 1 by 1 until selection-index > offer-links-row-count
        if offer-links-phase(selection-index) = 1 and offer-links-transaction(selection-index) =
            offer-links-original-transaction
            compute offer-links-reversed-mask = offer-links-reversed-mask + 2 ** (selection-index - 1)
        end-if
    end-perform.

recycling-policy.
    *> Kind (0 listing, 1 thing), active/standard, pinned/live dependency.
    move 0 to recycle-eligible
    if recycle-active-or-standard = 0 and recycle-dependency = 0 move 1 to recycle-eligible end-if.

first-eligible-record.
    *> Sixty-four eligibility flags; output slot 1 selects the first, or -1.
    move -1 to selection-best
    perform varying selection-index from 1 by 1 until selection-index > 64
        if first-eligible(selection-index) = 1
            compute selection-best = selection-index - 1
            exit perform
        end-if
    end-perform
    move selection-best to first-selected-index.

listing-transition.
    *> Action (0 list, 1 cancel, 2 edit, 3 buy, 4 classified list).
    *> Facts are documented in the Rust listing_plan adapter.
    *> Outputs status/payer/payee/thing action/standard in 31-35.
    move 0 to listing-next-status listing-payer listing-payee listing-thing-action listing-next-standard
    if listing-action = 0 or listing-action = 4
        if listing-action = 0 and listing-details-present = 1
            move listing-minor-units to wide-value
            if listing-details-kind = 4 and (listing-create-actor-nana = 0 or listing-buy-side = 0
                or listing-currency-empty = 0 or listing-minor-units not = 0)
                move result-forbidden to result-code exit paragraph
            end-if
            if listing-details-kind = -1 or listing-currency-control = 1
                or function abs(wide-value) > wallet-limit
                or (listing-details-kind = 3 and (listing-currency-empty = 1 or listing-minor-units <= 0))
                move result-invalid to result-code exit paragraph
            end-if
        end-if
        if listing-title-blank = 1 or listing-price < 1 or listing-price > command-amount-limit
            move result-invalid to result-code exit paragraph
        end-if
        if listing-action = 4
            if listing-quantity-milli < 1 or listing-quantity-milli > 1000000000
                move result-invalid to result-code exit paragraph
            end-if
            if listing-buy-side = 0 and listing-labor = 1 and listing-create-actor-nana = 1
                move result-forbidden to result-code exit paragraph
            end-if
            evaluate true
                when listing-requested-thing not = 0
                    if listing-thing-exists = 0 move result-not-found to result-code exit paragraph
                        end-if
                    if listing-thing-matches = 0 move result-invalid to result-code exit paragraph end-if
                    move 0 to listing-thing-action
                when listing-named-thing-exists = 1
                    if listing-named-thing-matches = 0 move result-conflict to result-code exit paragraph
                        end-if
                    move 1 to listing-thing-action
                when other
                    if listing-thing-book-full = 1 and listing-thing-recyclable = 0
                        move result-capacity to result-code exit paragraph
                    end-if
                    move 2 to listing-thing-action
            end-evaluate
            if listing-existing-standard = 1 or listing-requested-standard = 1 move 1 to
                listing-next-standard end-if
        end-if
        if listing-book-full = 1 and listing-recyclable = 0 move result-capacity to result-code end-if
        exit paragraph
    end-if
    if listing-exists = 0 move result-not-found to result-code exit paragraph end-if
    if listing-action = 1 or listing-action = 2
        if listing-actor not = listing-owner and listing-administrator = 0
            move result-forbidden to result-code exit paragraph
        end-if
    end-if
    if listing-active = 0 move result-conflict to result-code exit paragraph end-if
    evaluate listing-action
        when 1 move 2 to listing-next-status
        when 2
            if (listing-title-present = 1 and listing-title-blank = 1) or
                (listing-price-present = 1 and (listing-price < 1 or listing-price >
                    command-amount-limit))
                move result-invalid to result-code exit paragraph
            end-if
            move -1 to listing-next-status
        when 3
            if listing-good-deed = 1 move result-forbidden to result-code exit paragraph end-if
            if listing-buy-side = 1
                move listing-owner to listing-payer
                move listing-actor to listing-payee
            else
                move listing-actor to listing-payer
                move listing-owner to listing-payee
            end-if
            if listing-labor = 1 and
                ((listing-buy-side = 1 and listing-buyer-nana = 1) or
                 (listing-buy-side = 0 and listing-owner-nana = 1))
                move result-forbidden to result-code exit paragraph
            end-if
            move 1 to listing-next-status
        when other move result-invalid to result-code
    end-evaluate.

quote-projection.
    *> status, now, expiry, side (ASK=0), maker, taker; outputs 7-10.
    move 0 to quote-view-live
    move quote-view-status to quote-view-next-status
    if quote-view-status = 0
        if quote-view-now >= minimum-clock and quote-view-now <= wallet-limit
            and (quote-view-expires-at = 0 or quote-view-now < quote-view-expires-at)
            move 1 to quote-view-live
        end-if
        if quote-view-now not = 0 and quote-view-expires-at not = 0 and quote-view-now >=
            quote-view-expires-at
            move 3 to quote-view-next-status
        end-if
    end-if
    if quote-view-side = 0
        move quote-view-maker to quote-view-seller move quote-view-taker to quote-view-buyer
    else
        move quote-view-taker to quote-view-seller move quote-view-maker to quote-view-buyer
    end-if.

quote-transition.
    *> action post/take/cancel, actor, admin, now, exists, maker, side,
    *> stored status, live, rate, coins, scale, expiry.
    *> Book inputs 27-58 are sixteen maker/live pairs; empty maker is zero.
    *> Outputs 17 status, 18 seller, 19 buyer, 20 cash total.
    move 0 to quote-next-status quote-seller quote-buyer quote-cash-total
    if quote-action not = 2 and
        (quote-now < minimum-clock or quote-now > wallet-limit)
        move result-unavailable to result-code exit paragraph
    end-if
    if quote-action = 0
        if quote-cents-per-coin < 1 or quote-cents-per-coin > command-amount-limit or
            quote-coins < 1 or quote-coins > command-amount-limit or quote-nc-scale < 1 or
            (quote-expires-at not = 0 and quote-expires-at <= quote-now) or
            quote-expires-at > wallet-limit
            move result-invalid to result-code exit paragraph
        end-if
        compute wide-value = quote-cents-per-coin * quote-coins
        divide wide-value by quote-nc-scale giving quotient-value remainder remainder-value
        if remainder-value not = 0 or quotient-value < 1 or
            quotient-value > command-amount-limit
            move result-invalid to result-code exit paragraph
        end-if
        move quotient-value to quote-cash-total
        move 0 to quote-count quote-room
        perform varying selection-index from 1 by 1 until selection-index > 16
            if quote-book-maker(selection-index) = quote-actor and quote-book-live(selection-index) = 1
                add 1 to quote-count
            end-if
            if quote-book-maker(selection-index) = 0 or quote-book-live(selection-index) = 0
                move 1 to quote-room
            end-if
        end-perform
        if quote-count >= 2 move result-quote-limit to result-code exit paragraph end-if
        if quote-room = 0 move result-capacity to result-code end-if
        exit paragraph
    end-if
    if quote-exists = 0 move result-not-found to result-code exit paragraph end-if
    if quote-action = 2
        if quote-actor not = quote-maker and quote-administrator = 0
            move result-forbidden to result-code exit paragraph
        end-if
        if quote-status not = 0 move result-conflict to result-code exit paragraph end-if
        move 2 to quote-next-status exit paragraph
    end-if
    if quote-action not = 1 move result-invalid to result-code exit paragraph end-if
    if quote-actor = quote-maker move result-self-deal to result-code exit paragraph end-if
    if quote-live = 0 move result-conflict to result-code exit paragraph end-if
    move 1 to quote-next-status
    if quote-side = 0
        move quote-maker to quote-seller move quote-actor to quote-buyer
    else
        move quote-actor to quote-seller move quote-maker to quote-buyer
    end-if
    compute wide-value = quote-cents-per-coin * quote-coins
    divide wide-value by quote-nc-scale giving quotient-value
    move quotient-value to quote-cash-total.

quote-ordering.
    *> Sixteen side/price/id triples; side=-1 denotes an empty row.
    *> Sorted zero-based row indices in slots 49-64, empty outputs=-1.
    initialize quote-used
    perform varying quote-output from 1 by 1 until quote-output > 16
        move -1 to quote-order-output-index(quote-output)
    end-perform
    perform varying quote-output from 1 by 1 until quote-output > 16
        move -1 to quote-order-output-index(quote-output)
        move 0 to selection-best
        perform varying selection-index from 1 by 1 until selection-index > 16
            if quote-order-side(selection-index) >= 0 and quote-selected(selection-index) = 0
                move quote-order-price(selection-index) to wide-value
                if quote-order-side(selection-index) = 1 compute wide-value = 0 - wide-value end-if
                if selection-best = 0 or quote-order-side(selection-index) < selected-quote-side or
                    (quote-order-side(selection-index) = selected-quote-side and wide-value <
                        selected-quote-price) or
                    (quote-order-side(selection-index) = selected-quote-side and wide-value =
                        selected-quote-price and
                     quote-order-id(selection-index) < selection-key)
                    move selection-index to selection-best
                    move quote-order-side(selection-index) to selected-quote-side
                    move wide-value to selected-quote-price
                    move quote-order-id(selection-index) to selection-key
                end-if
            end-if
        end-perform
        if selection-best = 0 exit perform end-if
        move 1 to quote-selected(selection-best)
        compute quote-order-output-index(quote-output) = selection-best - 1
    end-perform.

gift-transition.
    *> action create/close/contribute, actor, disabled, now, capacity-full,
    *> exists, owner, closed, deadline-present/value, target-present/value,
    *> title-empty, received, amount. Outputs 21-24: received, overflow,
    *> closed, owner. Contribution overflow is deferred until funds validate.
    move 0 to gift-next-received gift-overflow gift-next-closed gift-next-owner
    if gift-disabled = 1 move result-disabled to result-code exit paragraph end-if
    if gift-action = 0
        if gift-full = 1 move result-capacity to result-code exit paragraph end-if
        if gift-title-empty = 1 or (gift-target-present = 1 and
            (gift-target < 1 or gift-target > command-amount-limit)) or
            (gift-deadline-present = 1 and (gift-deadline <= gift-now or
             gift-deadline > wallet-limit or gift-now < minimum-clock))
            move result-invalid to result-code exit paragraph
        end-if
        move gift-actor to gift-next-owner
        exit paragraph
    end-if
    if gift-exists = 0 move result-not-found to result-code exit paragraph end-if
    move gift-owner to gift-next-owner
    move gift-received to gift-next-received
    if gift-action = 1
        if gift-owner not = gift-actor move result-forbidden to result-code exit paragraph end-if
        if gift-closed = 1 move result-conflict to result-code exit paragraph end-if
        move 1 to gift-next-closed exit paragraph
    end-if
    if gift-action not = 2 move result-invalid to result-code exit paragraph end-if
    if gift-closed = 1 or (gift-deadline-present = 1 and
        (gift-now >= gift-deadline or gift-now < minimum-clock))
        move result-conflict to result-code exit paragraph
    end-if
    compute wide-value = gift-received + gift-amount
    if wide-value < min-i64 or wide-value > wallet-limit
        move 1 to gift-overflow
    else
        move wide-value to gift-next-received
    end-if.

gift-refund.
    *> received, current-epoch refund amount; output new received in slot 3.
    compute wide-value = gift-refund-received - gift-refund-amount
    if wide-value < min-i64 or wide-value > max-i64
        move result-overflow to result-code exit paragraph
    end-if
    move wide-value to gift-refund-next-received.

art-transition.
    *> action mint/list/buy/gift/equip, actor, disabled, full, exists,
    *> owner, price-present/value, equipped, revision, expected owner/revision/
    *> price, gift target/exists/disabled, equip requested, asset valid,
    *> event sequence, requested price-present/value.
    *> Outputs 31-37: owner, price-present/value, equipped, revision, seller, amount.
    move 0 to art-next-owner art-next-price-present art-next-price art-next-equipped art-next-revision
        art-seller art-payment-amount art-clear-equipment
    if art-disabled = 1 move result-disabled to result-code exit paragraph end-if
    if art-action = 0
        if art-full = 1 move result-capacity to result-code exit paragraph end-if
        if art-asset-valid = 0 move result-invalid to result-code exit paragraph end-if
        move art-actor to art-next-owner move art-event-sequence to art-next-revision
        exit paragraph
    end-if
    if art-exists = 0 move result-not-found to result-code exit paragraph end-if
    move art-owner to art-next-owner
    move art-price-present to art-next-price-present move art-price to art-next-price
    move art-equipped to art-next-equipped move art-revision to art-next-revision
    if art-action = 2
        if art-owner not = art-expected-owner or art-revision not = art-expected-revision or
            art-price-present = 0 or art-price not = art-expected-price
            move result-conflict to result-code exit paragraph
        end-if
        move art-owner to art-seller move art-expected-price to art-payment-amount
        move art-actor to art-next-owner
    else
        if art-owner not = art-actor move result-forbidden to result-code exit paragraph end-if
        evaluate art-action
            when 1
                if art-requested-price-present = 1 and (art-requested-price < 1 or art-requested-price >
                    command-amount-limit)
                    move result-invalid to result-code exit paragraph
                end-if
                move art-requested-price-present to art-next-price-present move art-requested-price to
                    art-next-price
                move art-event-sequence to art-next-revision exit paragraph
            when 3
                if art-gift-target = art-actor move result-invalid to result-code exit paragraph end-if
                if art-gift-target-exists = 0 move result-not-found to result-code exit paragraph end-if
                if art-gift-target-disabled = 1 move result-disabled to result-code exit paragraph end-if
                move art-gift-target to art-next-owner
            when 4
                move art-equip-requested to art-next-equipped art-clear-equipment exit paragraph
            when other move result-invalid to result-code exit paragraph
        end-evaluate
    end-if
    move 0 to art-next-price-present art-next-price art-next-equipped
    move art-event-sequence to art-next-revision.

art-metadata.
    *> title/permission text empty flags, hash/url byte lengths in slots 1-4.
    *> Raw hash bytes at offset 33 (64 bytes), URL at 97 (up to 192 bytes).
    if art-meta-title-empty = 1 or art-meta-permission-empty = 1 or art-meta-hash-length not = 64 or
        art-meta-url-length < 9 or art-meta-url-length > 192
        move result-invalid to result-code exit paragraph
    end-if
    perform varying selection-index from 1 by 1 until selection-index > 64
        compute flow-index = function ord(asset-hash(selection-index:1)) - 1
        if not (flow-index >= 48 and flow-index <= 57) and
            not (flow-index >= 65 and flow-index <= 70) and
            not (flow-index >= 97 and flow-index <= 102)
            move result-invalid to result-code exit paragraph
        end-if
    end-perform
    if asset-url(1:8) not = 'https://' or asset-url(9:1) = '/' or '?' or '#'
        move result-invalid to result-code exit paragraph
    end-if
    move art-meta-url-length to selection-best
    perform varying selection-index from 1 by 1 until selection-index > selection-best
        compute flow-index = function ord(asset-url(selection-index:1)) - 1
        if flow-index < 33 or flow-index > 126 or flow-index = 64
            move result-invalid to result-code exit paragraph
        end-if
    end-perform.

art-equipment.
    *> actor, count<=32, then owner IDs; output bit mask of rows to unequip.
    if equipment-row-count < 0 or equipment-row-count > 32
        move result-invalid to result-code exit paragraph
    end-if
    move 0 to equipment-clear-mask
    perform varying selection-index from 1 by 1 until selection-index > equipment-row-count
        if equipment-owner(selection-index) = equipment-actor
            compute equipment-clear-mask = equipment-clear-mask + 2 ** (selection-index - 1)
        end-if
    end-perform.

loan-projection.
    *> status, Nana, viewer, lender, borrower, borrower/lender exist/disabled,
    *> borrower/lender balances, amount, credit-blocked, next-due, now,
    *> principal/interest due, attempted balance. Outputs visible/terminal/ready 21-23.
    *> Extra inputs: accrual failure, optional profile subject (zero=normal book).
    *> Extra outputs: waiting reason, include, redact, overdue, accrue requested.
    move 0 to loan-view-visible loan-view-terminal loan-view-ready loan-view-waiting-reason
        loan-view-include loan-view-redact loan-view-accrue-requested
    compute loan-view-overdue = loan-view-principal-due + loan-view-interest-due
    if loan-view-status = 6 or loan-view-nana = 1 or loan-view-viewer = loan-view-lender or loan-view-viewer
        = loan-view-borrower
        move 1 to loan-view-visible
    end-if
    if loan-view-status = 3 or 4 or 5 move 1 to loan-view-terminal end-if
    if loan-view-profile-subject = 0
        move loan-view-visible to loan-view-include
    else
        if (loan-view-profile-subject = loan-view-lender or loan-view-profile-subject = loan-view-borrower)
            and (loan-view-status = 2 or 3)
            move 1 to loan-view-include
        end-if
    end-if
    if loan-view-visible = 0 move 1 to loan-view-redact end-if
    evaluate true
        when loan-view-borrower-disabled = 1 or loan-view-lender-disabled = 1 move 1 to
            loan-view-waiting-reason
        when loan-view-status = 2
            move 1 to loan-view-accrue-requested
            if loan-view-accrual-failed = 1 move 2 to loan-view-waiting-reason end-if
        when loan-view-status = 1
            evaluate true
                when loan-view-borrower-balance not = 0 move 3 to loan-view-waiting-reason
                when loan-view-credit-blocked = 1 move 4 to loan-view-waiting-reason
                when loan-view-lender-balance < loan-view-amount move 5 to loan-view-waiting-reason
                when other move 6 to loan-view-waiting-reason
            end-evaluate
    end-evaluate
    if loan-view-borrower-exists = 0 or loan-view-lender-exists = 0 or loan-view-borrower-disabled = 1 or
        loan-view-lender-disabled = 1
        exit paragraph
    end-if
    evaluate loan-view-status
        when 1
            if loan-view-borrower-balance = 0 and loan-view-lender-balance >= loan-view-amount and
                loan-view-credit-blocked = 0
                move 1 to loan-view-ready
            end-if
        when 2
            compute wide-value = loan-view-principal-due + loan-view-interest-due
            if loan-view-now >= loan-view-next-due or (wide-value > 0 and loan-view-borrower-balance > 0 and
                loan-view-borrower-balance not = loan-view-attempted-balance)
                move 1 to loan-view-ready
            end-if
    end-evaluate.

loan-book.
    *> Actor, count<=32, then lender*8+status words. Outputs count and terminal bitmap.
    if loan-book-row-count < 0 or loan-book-row-count > 32 move result-invalid to result-code exit paragraph
        end-if
    move 0 to loan-book-pending-count loan-book-terminal-mask
    perform varying selection-index from 1 by 1 until selection-index > loan-book-row-count
        divide loan-book-lender-status(selection-index) by 8 giving quotient-value remainder remainder-value
        if quotient-value = loan-book-actor and remainder-value = 0 add 1 to loan-book-pending-count end-if
        if remainder-value = 3 or 4 or 5
            compute loan-book-terminal-mask = loan-book-terminal-mask + 2 ** (selection-index - 1)
        end-if
    end-perform.

loan-policy.
    *> offer/request/respond/accept/close/repay/run, actor, now, borrower exists/
    *> disabled, terms borrower/amount/installment/rate days/payment days/memo control,
    *> loan exists/status/borrower/lender/lender exists/disabled/credit,
    *> mine/full/recyclable, expected updated/updated/ready, principal/accrued/next due.
    *> Outputs 31-39: status/lender/borrower/draw amount/next due/principal/accrued/updated/overflow.
    if loan-policy-now < minimum-clock or loan-policy-now > wallet-limit
        move result-unavailable to result-code exit paragraph
    end-if
    if loan-policy-action < 0 or loan-policy-action > 6 move result-invalid to result-code exit paragraph
        end-if
    move 0 to loan-policy-next-status loan-policy-next-lender loan-policy-next-borrower
        loan-policy-draw-amount loan-policy-next-due-at loan-policy-next-principal
        loan-policy-next-accrued-at loan-policy-next-updated loan-policy-overflow
    if loan-policy-action <= 2
        if loan-policy-action = 1
            if loan-policy-terms-borrower not = loan-policy-actor move result-forbidden to result-code exit
                paragraph end-if
        else
            if loan-policy-terms-borrower = loan-policy-actor move result-self-deal to result-code exit
                paragraph end-if
        end-if
        if loan-policy-borrower-exists = 0 move result-not-found to result-code exit paragraph end-if
        if loan-policy-borrower-disabled = 1 move result-disabled to result-code exit paragraph end-if
        if loan-policy-amount < 1 or loan-policy-amount > command-amount-limit or loan-policy-installment <
            1 or loan-policy-installment > loan-policy-amount or
            (loan-policy-rate-days not = 1 and 7 and 30 and 365) or
            (loan-policy-payment-days not = 1 and 7 and 30) or loan-policy-memo-control = 1
            move result-invalid to result-code exit paragraph
        end-if
        if loan-policy-action = 2
            if loan-policy-exists = 0 move result-not-found to result-code exit paragraph end-if
            if loan-policy-status not = 6 move result-conflict to result-code exit paragraph end-if
            if loan-policy-borrower not = loan-policy-terms-borrower move result-invalid to result-code exit
                paragraph end-if
        end-if
        if loan-policy-action not = 1 and loan-policy-mine-count >= 4 move result-loan-limit to result-code
            exit paragraph end-if
        if loan-policy-action not = 2 and loan-policy-book-full = 1 and loan-policy-recyclable = 0
            move result-capacity to result-code exit paragraph
        end-if
        compute wide-value = loan-policy-now + loan-policy-payment-days * seconds-per-day
        if wide-value > wallet-limit move result-overflow to result-code exit paragraph end-if
        move loan-policy-terms-borrower to loan-policy-next-borrower
        if loan-policy-action = 1 move 6 to loan-policy-next-status else move loan-policy-actor to
            loan-policy-next-lender end-if
        move loan-policy-now to loan-policy-next-updated exit paragraph
    end-if
    if loan-policy-action = 6 and loan-policy-actor not = 0 move result-forbidden to result-code exit
        paragraph end-if
    if loan-policy-exists = 0 move result-not-found to result-code exit paragraph end-if
    move loan-policy-status to loan-policy-next-status move loan-policy-lender to loan-policy-next-lender
        move loan-policy-borrower to loan-policy-next-borrower
    move loan-policy-next-due to loan-policy-next-due-at move loan-policy-principal to
        loan-policy-next-principal move loan-policy-accrued-at to loan-policy-next-accrued-at
    move loan-policy-now to loan-policy-next-updated
    evaluate loan-policy-action
        when 3
            if loan-policy-actor not = loan-policy-borrower move result-forbidden to result-code exit
                paragraph end-if
            if loan-policy-status not = 0 move result-conflict to result-code exit paragraph end-if
            if loan-policy-lender-exists = 0 move result-not-found to result-code exit paragraph end-if
            if loan-policy-lender-disabled = 1 move result-disabled to result-code exit paragraph end-if
            compute wide-value = loan-policy-now + loan-policy-payment-days * seconds-per-day
            if wide-value > wallet-limit move 1 to loan-policy-overflow end-if
            if loan-policy-credit-enabled = 1
                move 1 to loan-policy-next-status
            else
                move 2 to loan-policy-next-status move loan-policy-amount to loan-policy-draw-amount
                    loan-policy-next-principal
                move wide-value to loan-policy-next-due-at move loan-policy-now to
                    loan-policy-next-accrued-at
            end-if
        when 4
            if loan-policy-actor not = loan-policy-borrower and loan-policy-actor not = loan-policy-lender
                move result-forbidden to result-code exit paragraph
            end-if
            if loan-policy-status not = 0 and 1 and 6 move result-conflict to result-code exit paragraph
                end-if
            if loan-policy-actor = loan-policy-borrower and loan-policy-status = 0
                move 4 to loan-policy-next-status
            else move 5 to loan-policy-next-status end-if
        when 5
            if loan-policy-actor not = loan-policy-borrower move result-forbidden to result-code end-if
        when 6
            if loan-policy-actor not = 0 move result-forbidden to result-code exit paragraph end-if
            if loan-policy-expected-updated not = loan-policy-updated or loan-policy-ready = 0 move
                result-conflict to result-code end-if
        when other move result-invalid to result-code
    end-evaluate.

loan-summary.
    *> One row: status, principal, principal/interest due, rate bps/days.
    *> Accumulator: principal, overdue, active, weighted numerator high/low.
    *> Numerator uses common rate-days denominator 15330 and base 10^15 parts.
    *> Finalize flag in slot 12; output optional percentage flag/binary64 in 13/14.
    move 0 to loan-summary-rate-present loan-summary-rate-bits
    compute wide-value = loan-summary-weighted-high * flow-parts-base + loan-summary-weighted-low
    if loan-summary-status = 2
        if loan-summary-rate-days not = 1 and 7 and 30 and 365
            move result-invalid to result-code exit paragraph
        end-if
        add loan-summary-principal to loan-summary-total-principal
        compute loan-summary-total-overdue = loan-summary-total-overdue + loan-summary-principal-due +
            loan-summary-interest-due
        add 1 to loan-summary-active-count
        compute factor-value = 15330 / loan-summary-rate-days
        compute wide-value = wide-value + loan-summary-principal * loan-summary-rate-basis-points *
            factor-value
    end-if
    divide wide-value by flow-parts-base giving quotient-value remainder remainder-value
    move quotient-value to loan-summary-weighted-high move remainder-value to loan-summary-weighted-low
    if loan-summary-finalize = 1 and loan-summary-total-principal > 0
        compute denominator-value = loan-summary-total-principal * 15330 * 100
        compute summary-decimal = wide-value * 365 / denominator-value
        move summary-decimal to summary-percent
        move summary-bits to loan-summary-rate-bytes
        move 1 to loan-summary-rate-present
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
    move 0 to lotto-actor-ticket-count
    initialize lotto-output-word-table
    if lotto-now < minimum-clock or lotto-now > wallet-limit
        move result-unavailable to result-code exit paragraph
    end-if
    if lotto-action = 1
        if lotto-administrator = 0 move result-forbidden to result-code exit paragraph end-if
        if lotto-book-count = 16 and lotto-recyclable-row = 0
            move result-capacity to result-code exit paragraph
        end-if
        if lotto-kind < 0 or lotto-kind > 2 or lotto-title-valid = 0
            or lotto-ticket-price < 1 or lotto-ticket-price > command-amount-limit
            or lotto-closes-at <= lotto-now or lotto-closes-at > 9007199252148991
            or lotto-rate-basis-points < 0 or lotto-rate-basis-points > basis-points-per-whole
            or (lotto-kind = 0 and lotto-rate-basis-points not = 0)
            move result-invalid to result-code
        end-if
        exit paragraph
    end-if
    if lotto-action = 3 and lotto-actor not = 0
        move result-forbidden to result-code exit paragraph
    end-if
    if lotto-exists = 0 move result-not-found to result-code exit paragraph end-if
    move lotto-pool to lotto-next-pool move lotto-escrow to lotto-next-escrow
    move lotto-interest to lotto-next-interest move lotto-remaining to lotto-next-remaining
    move lotto-winner to lotto-next-winner move lotto-step to lotto-next-step
    evaluate lotto-action
        when 2
            if lotto-actor = lotto-house move result-forbidden to result-code exit paragraph end-if
            if lotto-now >= lotto-closes-at or lotto-step not = 0
                move result-conflict to result-code exit paragraph
            end-if
            if lotto-ticket-count-or-draw < 1 or lotto-ticket-count-or-draw > max-u32
                or lotto-actor < 1 or lotto-actor > 32
                move result-invalid to result-code exit paragraph
            end-if
            move lotto-actor to selection-index
            compute wide-value = lotto-tickets(selection-index) + lotto-ticket-count-or-draw
            if wide-value > max-u32
                move result-overflow to result-code exit paragraph
            end-if
            move wide-value to lotto-actor-ticket-count
            compute wide-value = lotto-ticket-price * lotto-ticket-count-or-draw
            if wide-value > command-amount-limit
                move result-overflow to result-code exit paragraph
            end-if
            move wide-value to lotto-payment-amount
            compute wide-value = lotto-pool + lotto-payment-amount
            if wide-value > command-amount-limit
                move result-overflow to result-code exit paragraph
            end-if
            move wide-value to lotto-next-pool
            compute wide-value = lotto-next-pool * lotto-rate-basis-points / basis-points-per-whole
            move wide-value to lotto-next-interest lotto-next-remaining
            compute wide-value = lotto-next-pool + lotto-next-interest
            compute credit-value = lotto-global-escrow + lotto-payment-amount
            if wide-value > command-amount-limit or credit-value > wallet-limit
                move result-overflow to result-code exit paragraph
            end-if
            if lotto-actor-or-house-cash < lotto-payment-amount move result-insufficient-funds to
                result-code exit paragraph end-if
            compute lotto-next-escrow = lotto-escrow + lotto-payment-amount
            compute lotto-payment-parties = lotto-actor * 256 + 255
        when 3
            compute wide-value = lotto-closes-at
            if lotto-kind not = 0 add 2592000 to wide-value end-if
            if lotto-expected-step not = lotto-step or lotto-step = 35 or lotto-now < wide-value
                move result-conflict to result-code exit paragraph
            end-if
            move 0 to quotient-value
            perform varying selection-index from 1 by 1 until selection-index > 32
                add lotto-tickets(selection-index) to quotient-value
            end-perform
            if (lotto-step = 0 and lotto-ticket-count-or-draw >= function max(1, quotient-value))
                or (lotto-step not = 0 and lotto-ticket-count-or-draw not = 0) or lotto-ticket-count-or-draw
                    < 0
                move result-invalid to result-code exit paragraph
            end-if
            if lotto-step = 0
                move lotto-ticket-count-or-draw to remainder-value
                move 35 to lotto-next-step
                perform varying selection-index from 1 by 1 until selection-index > 32
                    if remainder-value < lotto-tickets(selection-index)
                        compute lotto-next-winner = selection-index
                        move 1 to lotto-next-step
                        exit perform
                    end-if
                    subtract lotto-tickets(selection-index) from remainder-value
                end-perform
            else
                compute lotto-next-step = lotto-step + 1
                if lotto-winner = 0 exit paragraph end-if
                evaluate true
                    when lotto-step >= 1 and lotto-step <= 32
                        if lotto-kind = 2
                            move lotto-step to selection-index
                            compute lotto-payment-amount = lotto-tickets(selection-index) *
                                lotto-ticket-price
                        else
                            if lotto-step = lotto-winner move lotto-pool to lotto-payment-amount end-if
                        end-if
                        compute lotto-payment-parties = 255 * 256 + lotto-step
                        subtract lotto-payment-amount from lotto-next-escrow
                    when lotto-step = 33
                        compute lotto-payment-amount = function min(lotto-remaining, function max(0,
                            lotto-actor-or-house-cash))
                        compute lotto-payment-parties = 65536 + lotto-house * 256 + lotto-winner
                        subtract lotto-payment-amount from lotto-next-remaining
                    when lotto-step = 34
                        move lotto-remaining to lotto-payment-amount
                        compute lotto-payment-parties = 65536 + lotto-winner
                        subtract lotto-payment-amount from lotto-next-remaining
                end-evaluate
            end-if
        when other move result-invalid to result-code
    end-evaluate.

lotto-projection.
    *> Inputs: kind, closes, step, now, tickets for members 1..32 (5..36).
    *> Outputs 37..41: total tickets, due, status (0 open/1 waiting/
    *> 2 paying/3 settled), eligible for tick, terminal.
    move 0 to lotto-view-total-tickets lotto-view-status lotto-view-tick-eligible lotto-view-terminal
    perform varying selection-index from 1 by 1 until selection-index > 32
        add lotto-view-tickets(selection-index) to lotto-view-total-tickets
    end-perform
    move lotto-view-closes-at to lotto-view-due-at
    if lotto-view-kind not = 0 add 2592000 to lotto-view-due-at end-if
    evaluate true
        when lotto-view-step = 35 move 3 to lotto-view-status move 1 to lotto-view-terminal
        when lotto-view-step > 0 move 2 to lotto-view-status
        when lotto-view-now >= lotto-view-closes-at move 1 to lotto-view-status
    end-evaluate
    if lotto-view-step < 35 and lotto-view-now >= lotto-view-due-at move 1 to lotto-view-tick-eligible
        end-if
    *> Optional draw input 42; output 43 identifies its owner, zero out of range.
    move 0 to lotto-view-winner
    if lotto-view-draw-ticket >= 0 and lotto-view-draw-ticket < lotto-view-total-tickets
        move lotto-view-draw-ticket to remainder-value
        perform varying selection-index from 1 by 1 until selection-index > 32
            if remainder-value < lotto-view-tickets(selection-index)
                compute lotto-view-winner = selection-index exit perform
            end-if
            subtract lotto-view-tickets(selection-index) from remainder-value
        end-perform
    end-if.

lotto-book.
    *> Inputs: count then up to 16 settlement steps; output 18 is first
    *> completed row (one-based), zero if no recyclable row exists.
    move 0 to lotto-book-recyclable-row
    if lotto-book-row-count < 0 or lotto-book-row-count > 16 move result-invalid to result-code exit
        paragraph end-if
    perform varying selection-index from 1 by 1 until selection-index > lotto-book-row-count
        if lotto-book-step(selection-index) = 35
            compute lotto-book-recyclable-row = selection-index exit perform
        end-if
    end-perform.

lotto-invariants.
    *> Inputs 1..16: id, sequence, duplicate, house exists, kind, price,
    *> rate, closes, step, pool, escrow, interest, remaining, winner,
    *> member-presence bitmap, accumulated escrow; 17..48: member tickets.
    *> Output 49: accumulated escrow. Invalid facts return 1 (corrupt state).
    *> Final call: input 51 = 1 compares accumulated escrow (16) to global (50).
    if lotto-check-finalize = 1
        if lotto-check-escrow-sum < 0 or lotto-check-escrow-sum not = lotto-check-global-escrow move
            result-invalid to result-code end-if
        move lotto-check-escrow-sum to lotto-check-next-escrow-sum exit paragraph
    end-if
    if lotto-check-id < 1 or lotto-check-id > lotto-check-sequence or lotto-check-duplicate not = 0 or
        lotto-check-house-exists = 0
        or lotto-check-kind < 0 or lotto-check-kind > 2 or lotto-check-step < 0 or lotto-check-step > 35
        or lotto-check-ticket-price < 1 or lotto-check-ticket-price > command-amount-limit
        or lotto-check-rate-basis-points < 0 or lotto-check-rate-basis-points > basis-points-per-whole or
            lotto-check-closes-at < 0 or lotto-check-closes-at > 9007199252148991
        or (lotto-check-kind = 0 and lotto-check-rate-basis-points not = 0)
        or lotto-check-pool < 0 or lotto-check-pool > command-amount-limit
        or lotto-check-escrow < 0 or lotto-check-escrow > lotto-check-pool
        or lotto-check-remaining < 0 or lotto-check-remaining > lotto-check-interest
        or lotto-check-winner < 0 or lotto-check-winner > 32
        or (lotto-check-step = 35 and (lotto-check-escrow not = 0 or lotto-check-remaining not = 0))
        or (lotto-check-step = 0 and lotto-check-winner not = 0)
        or (lotto-check-step <= 33 and lotto-check-remaining not = lotto-check-interest)
        move result-invalid to result-code exit paragraph
    end-if
    move 0 to quotient-value debit-value
    move 1 to factor-value
    perform varying selection-index from 1 by 1 until selection-index > 32
        if lotto-check-tickets(selection-index) < 0 or lotto-check-tickets(selection-index) > max-u32
            move result-invalid to result-code exit paragraph
        end-if
        add lotto-check-tickets(selection-index) to quotient-value
        if lotto-check-tickets(selection-index) > 0
            compute remainder-value = function mod(function integer(lotto-check-member-mask / factor-value),
                2)
            if remainder-value = 0 move result-invalid to result-code exit paragraph end-if
        end-if
        if lotto-check-kind = 2 and lotto-check-step > selection-index
            compute debit-value = debit-value + lotto-check-tickets(selection-index) *
                lotto-check-ticket-price
        end-if
        multiply 2 by factor-value
    end-perform
    compute wide-value = quotient-value * lotto-check-ticket-price
    compute credit-value = lotto-check-pool * lotto-check-rate-basis-points / basis-points-per-whole
    if wide-value not = lotto-check-pool or credit-value not = lotto-check-interest
        or (lotto-check-step > 0 and quotient-value > 0 and lotto-check-winner = 0)
        move result-invalid to result-code exit paragraph
    end-if
    if lotto-check-winner > 0
        move lotto-check-winner to selection-index
        if lotto-check-tickets(selection-index) = 0 move result-invalid to result-code exit paragraph end-if
        if lotto-check-kind not = 2 and lotto-check-step > lotto-check-winner move lotto-check-pool to
            debit-value end-if
    end-if
    compute wide-value = lotto-check-pool - debit-value
    if wide-value not = lotto-check-escrow move result-invalid to result-code exit paragraph end-if
    compute wide-value = lotto-check-escrow-sum + lotto-check-escrow
    if wide-value < 0 or wide-value > max-i64
        move result-invalid to result-code exit paragraph
    end-if
    move wide-value to lotto-check-next-escrow-sum.

reform-policy.
    *> Inputs: requested decimals/power, current decimals, epoch count,
    *> current epoch/sequence, expected epoch/sequence, epoch capacity.
    *> Outputs 10..12: exponent, next epoch, new NC scale.
    if reform-current-epoch not = reform-expected-epoch or reform-current-sequence not =
        reform-expected-sequence
        move result-conflict to result-code exit paragraph
    end-if
    if reform-requested-decimals < 0 or reform-requested-decimals > 8 or reform-power < -12 or reform-power
        > 12
        or (reform-requested-decimals = reform-current-decimals and reform-power = 0)
        move result-invalid to result-code exit paragraph
    end-if
    if reform-epoch-count >= reform-epoch-capacity move result-overflow to result-code exit paragraph end-if
    compute reform-exponent = reform-requested-decimals - reform-current-decimals - reform-power
    compute reform-next-epoch = reform-current-epoch + 1
    compute reform-nc-scale = 10 ** reform-requested-decimals.

reform-lotto-consistency.
    *> Inputs: converted pool, converted promised interest, rate.
    *> Exact conversion must preserve the original rounded obligation.
    compute wide-value = reform-lotto-pool * reform-lotto-rate-basis-points / basis-points-per-whole
    if wide-value not = reform-lotto-promised-interest move result-conflict to result-code end-if.

reform-field.
    *> Inputs: value, old/new decimals, power, field kind; output 6.
    *> 0 gift target, 1 net gifts, 2 art price, 3 lotto money,
    *> 4 global escrow, 5 grant, 6 wallet/issuance, 7 listing/offer,
    *> 8 quote coins, 9 quote price, 10 loan terms, 11 loan balances.
    if reform-field-kind < 0 or reform-field-kind > 11 move result-invalid to result-code exit paragraph
        end-if
    compute exponent-value = reform-field-new-decimals - reform-field-old-decimals - reform-field-power
    if reform-field-kind = 9 move reform-field-power to exponent-value end-if
    if reform-field-kind = 1 or reform-field-kind = 4 or reform-field-kind = 6 or reform-field-kind = 11
        move wallet-limit to rescale-limit
    else
        move command-amount-limit to rescale-limit
    end-if
    move exponent-value to rescale-exponent
    perform exact-rescale
    if result-code = result-success move rescale-converted-units to reform-field-converted-value end-if.

account-posting.
    *> Inputs: from, to, amount, correction, USD, source exists/disabled,
    *> target exists/disabled, source/target balances. Outputs 7/8: balances.
    if account-payer = account-payee or account-amount < 1 or account-amount > command-amount-limit
        move result-invalid to result-code exit paragraph
    end-if
    if account-payer not = 0
        if account-source-exists = 0 move result-not-found to result-code exit paragraph end-if
        if account-correction = 0 and account-source-disabled = 1 move result-disabled to result-code exit
            paragraph end-if
    end-if
    if account-payee not = 0
        if account-target-exists = 0 move result-not-found to result-code exit paragraph end-if
        if account-correction = 0 and account-target-disabled = 1 move result-disabled to result-code exit
            paragraph end-if
    end-if
    move account-correction to account-saved-correction move account-usd to account-saved-usd
    if account-payer = 0 move 1 to post-issuance else move 0 to post-issuance end-if
    move account-source-balance to post-source-balance
    move account-target-balance to post-target-balance
    move account-saved-correction to post-correction
    move account-saved-usd to post-usd
    perform posting-plan.

correction-policy.
    *> Inputs: action (0 refund/1 reverse/2 room), original exists, actor,
    *> Nana, original from/to/amount, reverses, USD, loan, lotto, quote,
    *> art, refunded units, reversed, requested units, count/capacity, row exists.
    *> Outputs 20..23: payer, payee, original units, correction-overdraft flag.
    if correction-policy-action not = 2
        if correction-policy-original-exists = 0 move result-not-found to result-code exit paragraph end-if
        evaluate correction-policy-action
            when 0
                if correction-policy-original-reversal = 1 or correction-policy-original-amount = 0 or
                    correction-policy-usd = 1
                    or correction-policy-original-payer = 0 or correction-policy-original-payee = 0 or
                        correction-policy-loan = 1
                    or correction-policy-lotto = 1 or correction-policy-quote = 1 or correction-policy-art =
                        1
                    move result-forbidden to result-code exit paragraph
                end-if
                if correction-policy-actor not = correction-policy-original-payee and correction-policy-nana
                    = 0
                    move result-forbidden to result-code exit paragraph
                end-if
                compute wide-value = correction-policy-original-amount - correction-policy-refunded
                if correction-policy-requested-amount <= 0 or correction-policy-requested-amount >
                    wide-value
                    move result-conflict to result-code exit paragraph
                end-if
                move correction-policy-requested-amount to correction-policy-amount move 0 to
                    correction-policy-overdraft
            when 1
                if correction-policy-original-amount = 0 or correction-policy-loan = 1 or
                    correction-policy-lotto = 1
                    move result-forbidden to result-code exit paragraph
                end-if
                if correction-policy-nana = 0 and (correction-policy-original-payee not =
                    correction-policy-actor or correction-policy-original-payer = 0
                    or correction-policy-usd = 1 or correction-policy-quote = 1)
                    move result-forbidden to result-code exit paragraph
                end-if
                if correction-policy-refunded not = 0 or correction-policy-reversed = 1 or
                    correction-policy-original-reversal = 1 or correction-policy-art = 1
                    move result-conflict to result-code exit paragraph
                end-if
                move correction-policy-original-amount to correction-policy-amount move
                    correction-policy-nana to correction-policy-overdraft
            when other move result-invalid to result-code exit paragraph
        end-evaluate
        move correction-policy-original-payee to correction-policy-payer move
            correction-policy-original-payer to correction-policy-payee
    end-if
    if correction-policy-annotation-count = correction-policy-annotation-capacity and
        correction-policy-annotation-exists = 0 move result-capacity to result-code end-if.

historical-amount.
    *> Inputs 1..5: original units, USD, original epoch, epoch count, limit;
    *> 6..37: epoch exponents (including the original epoch). Outputs 38/39:
    *> converted units, sum of subsequent exponents. USD is never reformed.
    move 0 to historical-total-exponent
    if historical-usd = 1 move historical-original-units to historical-converted-amount exit paragraph
        end-if
    if historical-epoch-count < 1 or historical-epoch-count > 32 or historical-amount-epoch < 0 or
        historical-amount-epoch >= historical-epoch-count
        move result-invalid to result-code exit paragraph
    end-if
    compute selection-index = historical-amount-epoch + 2
    perform until selection-index > historical-epoch-count
        add historical-exponent(selection-index) to historical-total-exponent
        add 1 to selection-index
    end-perform
    move historical-total-exponent to rescale-exponent
    move historical-limit to rescale-limit
    perform exact-rescale
    if result-code = result-success move rescale-converted-units to historical-converted-amount end-if.

member-update-policy.
    *> Exists, bad bio, current verifier, new verifier present, self,
    *> role/status present, administrator, name blank/control; Mastodon
    *> empty/user empty/host empty/extra @/host dot/bad text; verifier valid,
    *> current Nana/disabled, demote or disable, active Nana count.
    if member-update-exists = 0 move result-not-found to result-code exit paragraph end-if
    if member-update-bio-invalid = 1 or (member-update-verifier-present = 0 and
        member-update-new-verifier-present = 1)
        move result-invalid to result-code exit paragraph
    end-if
    if (member-update-self = 0 or member-update-role-present = 1 or member-update-status-present = 1) and
        member-update-administrator = 0
        move result-forbidden to result-code exit paragraph
    end-if
    if member-update-name-blank = 1 or member-update-name-control = 1
        move result-invalid to result-code exit paragraph
    end-if
    if member-update-mastodon-empty = 0 and (member-update-mastodon-user-empty = 1 or
        member-update-mastodon-host-empty = 1 or member-update-mastodon-extra-at = 1
        or member-update-mastodon-host-dot = 0 or member-update-mastodon-invalid = 1)
        move result-invalid to result-code exit paragraph
    end-if
    if member-update-new-verifier-present = 1 and member-update-verifier-valid = 0 move result-invalid to
        result-code exit paragraph end-if
    if member-update-current-nana = 1 and member-update-current-disabled = 0 and member-update-removing-nana
        = 1 and member-update-active-nana-count = 1
        move result-forbidden to result-code
    end-if.

member-import-policy.
    *> Action 0=credential setup, 1=token member. Administrator, self,
    *> target exists/verifier present, name blank/control, new verifier valid,
    *> count/capacity, duplicate username/name/hash, zero token hash.
    evaluate import-member-action
        when 0
            if import-member-self = 0 and import-member-administrator = 0 move result-forbidden to
                result-code exit paragraph end-if
            if import-member-target-exists = 0 move result-not-found to result-code exit paragraph end-if
            if import-member-verifier-present = 1 move result-conflict to result-code exit paragraph end-if
            if import-member-name-blank = 1 or import-member-name-control = 1 or
                import-member-verifier-valid = 0
                move result-invalid to result-code exit paragraph
            end-if
            if import-member-duplicate = 1 move result-conflict to result-code end-if
        when 1
            if import-member-administrator = 0 move result-forbidden to result-code exit paragraph end-if
            if import-member-name-blank = 1 or import-member-zero-token-hash = 1 move result-invalid to
                result-code exit paragraph end-if
            if import-member-count >= import-member-capacity move result-capacity to result-code exit
                paragraph end-if
            if import-member-duplicate = 1 move result-conflict to result-code end-if
        when other move result-invalid to result-code
    end-evaluate.

identity-transition.
    *> Action: provision/human/bot/token member/update/full key/read key/setup/config.
    *> Count, requested/current role, current disabled, role/status present,
    *> requested disabled, verifier present, zero key, timestamp, household blank,
    *> current/new/default settlement, grant. Outputs 33..42:
    *> id, role, kind, disabled, revoke keys, key timestamp, full key,
    *> default household, settlement, grant cash leg present.
    if identity-action < 0 or identity-action > 8 move result-invalid to result-code exit paragraph end-if
    move 0 to identity-member-id identity-role identity-kind identity-disabled identity-revoke-key-plan
        identity-key-created-at
        identity-full-key identity-default-household identity-grant-cash-leg
    move identity-current-settlement to identity-settlement
    evaluate identity-action
        when 0 when 1 when 2 when 3
            if identity-member-count < 0 or identity-member-count >= 32 move result-capacity to result-code
                exit paragraph end-if
            compute identity-member-id = identity-member-count + 1
            move identity-requested-role to identity-role
            if identity-action = 0 or (identity-action = 3 and identity-member-id = 1)
                move 1 to identity-role
            end-if
            if identity-action = 2 move 0 to identity-role move 1 to identity-kind end-if
            if identity-action = 3 and identity-member-id not = 1 move 0 to identity-role end-if
            if (identity-action = 1 or identity-action = 2) and identity-grant > 0 move 1 to
                identity-grant-cash-leg end-if
        when 4
            move identity-current-role to identity-role
            if identity-role-present = 1 move identity-requested-role to identity-role end-if
            move identity-current-disabled to identity-disabled
            if identity-status-present = 1 move identity-requested-disabled to identity-disabled end-if
            move identity-revoke-keys to identity-revoke-key-plan
        when 5 when 6
            if identity-zero-key = 0 move identity-now to identity-key-created-at end-if
            if identity-action = 5 move 1 to identity-full-key end-if
        when 7
            move identity-household-blank to identity-default-household
        when 8
            if identity-requested-settlement >= 0
                move identity-requested-settlement to identity-settlement
                if identity-requested-settlement = 0 move identity-default-settlement to identity-settlement
                    end-if
            end-if
    end-evaluate.

actor-policy.
    *> member count, actor id, exists, disabled, bootstrap command.
    if actor-member-count = 0
        if actor-id not = 1 or actor-bootstrap-command = 0 move result-forbidden to result-code end-if
    else
        if actor-exists = 0 move result-not-found to result-code
        else if actor-disabled = 1 move result-disabled to result-code end-if end-if
    end-if.

new-member-policy.
    *> Username blank/control, display blank/control, verifier valid,
    *> member count, capacity, duplicate username. Error order is contractual.
    if new-member-username-blank = 1 or new-member-username-control = 1 or new-member-display-blank = 1 or
        new-member-display-control = 1 or new-member-verifier-valid = 0
        move result-invalid to result-code
    else
        if new-member-member-count >= new-member-capacity move result-capacity to result-code
        else if new-member-duplicate-username = 1 move result-conflict to result-code end-if end-if
    end-if.

household-statistics.
    *> Selectors and binary64 display aggregates; money remains exact elsewhere.
    move 0 to statistics-result statistics-first-count statistics-second-count statistics-second-result
    evaluate statistics-action
        when select-household-retained-year
            *> Usable retained-year row: USD/created/now/reverse/corrected.
            compute wide-value = function max(0, retained-year-now - 31536000)
            if retained-year-usd = 0 and retained-year-created-at >= wide-value and retained-year-reversal =
                0 and retained-year-corrected = 0
                move 1 to retained-year-include end-if
        when select-household-employment-member
            *> Eligible employment member: disabled/Nana.
            if employment-member-disabled = 0 and employment-member-nana = 0 move 1 to
                employment-member-eligible end-if
        when select-household-labor-row
            *> Labor row: payee/member/amount/kind (1 labor).
            if labor-row-payee = labor-row-member and labor-row-amount > 0 and labor-row-economic-kind = 1
                move 1 to labor-row-include end-if
        when select-household-sale-row
            *> Good sale: kind (2 good), thing/subject, quantity/amount.
            if sale-row-economic-kind = 2 and sale-row-thing = sale-row-subject and sale-row-quantity > 0
                and sale-row-amount > 0
                move 1 to sale-row-include end-if
        when select-household-inflation-fold
            *> Inflation fold: conversion flags, latest amount/quantity, prior
            *> amount/quantity, incoming binary64 total/count. Outputs total/count.
            move inflation-total-bytes to stats-total-bits
            move inflation-count to inflation-next-count
            if inflation-latest-converted = 1 and inflation-prior-converted = 1 and inflation-prior-amount >
                0
                if inflation-latest-quantity <= 0 or inflation-prior-quantity <= 0 move result-invalid to
                    result-code exit paragraph end-if
                move inflation-latest-amount to stats-first move inflation-latest-quantity to stats-second
                compute stats-first = stats-first / stats-second
                move inflation-prior-amount to stats-second
                compute stats-second = stats-second / inflation-prior-quantity
                compute stats-total = stats-total + (stats-first / stats-second - 1) * 100
                add 1 to inflation-next-count
            end-if
            move stats-total-bits to inflation-next-total-bytes
        when select-household-employment-fold
            *> Employment fold: eligible/labor-found, eligible/employed counts.
            move employment-eligible-count to employment-next-eligible-count move employment-employed-count
                to employment-next-employed-count
            if employment-eligible = 1
                add 1 to employment-next-eligible-count
                if employment-labor-found = 1 add 1 to employment-next-employed-count end-if
            end-if
        when select-household-finalize
            *> Finalize: inflation total/count, eligible/employed counts.
            if statistics-final-inflation-count > 0
                move statistics-total-bytes to stats-total-bits
                compute stats-total = stats-total / statistics-final-inflation-count
                move 1 to statistics-final-inflation-present move stats-total-bits to statistics-rate-bytes
            end-if
            if statistics-final-eligible-count > 0
                compute stats-total = statistics-final-employed-count * 100 /
                    statistics-final-eligible-count
                move 1 to statistics-final-employment-present move stats-total-bits to
                    statistics-employment-bytes
            end-if
        when select-household-filled-quote
            *> Filled quote selection: filled, coin/cash corrections, current
            *> present/time, candidate time/rate. Later rows win equal times.
            if filled-quote-filled = 1 and filled-quote-coins-corrected = 0 and filled-quote-cash-corrected
                = 0
                and (filled-quote-current-present = 0 or filled-quote-candidate-time >=
                    filled-quote-current-time) move 1 to filled-quote-include end-if
        when select-household-coin-row
            *> Coin candidate: USD/quote/reverse/amount/corrected,
            *> current quote present/time, row time. Output 18 stop selection.
            if exchange-coin-usd = 0 and exchange-coin-quote = 1 and exchange-coin-reversal = 0 and
                exchange-coin-amount > 0 and exchange-coin-corrected = 0
                move 1 to exchange-coin-include
                if exchange-coin-current-quote-present = 1 and exchange-coin-current-quote-time >
                    exchange-coin-row-time move 1 to exchange-coin-stop end-if
            end-if
        when select-household-cash-row
            *> Matching cash candidate: USD/quote match/reverse/amount/corrected.
            if exchange-cash-usd = 1 and exchange-cash-quote-matches = 1 and exchange-cash-reversal = 0 and
                exchange-cash-amount > 0 and exchange-cash-corrected = 0
                move 1 to exchange-cash-include end-if
        when select-household-exchange-rate
            *> Paired exchange: conversion valid, current NC/cash, decimals.
            if exchange-display-converted = 1 and exchange-display-nc > 0
                if exchange-display-decimals < 0 or exchange-display-decimals > 8 move result-invalid to
                    result-code exit paragraph end-if
                move exchange-display-cash to stats-first
                compute stats-total = stats-first * (10 ** exchange-display-decimals) / exchange-display-nc
                    / 100
                move 1 to exchange-display-present move stats-total-bits to statistics-rate-bytes
            end-if
        when select-household-quote-rate
            *> Quote cents/coin converted to display dollars/coin.
            compute stats-total = quote-display-cents-per-coin / 100
            move 1 to quote-display-present move stats-total-bits to statistics-rate-bytes
        when select-household-sales-fold
            *> Retained sale fold in descending ledger order: eligible, row
            *> index, latest/prior indices and count. Select exactly two rows.
            if sales-fold-count < 0 or sales-fold-count > 2 or sales-fold-row-index < 0
                move result-invalid to result-code exit paragraph end-if
            move sales-fold-latest-index to sales-fold-next-latest-index move sales-fold-prior-index to
                sales-fold-next-prior-index
            move sales-fold-count to sales-fold-next-count
            if sales-fold-eligible = 1 and sales-fold-count < 2
                if sales-fold-count = 0 move sales-fold-row-index to sales-fold-next-latest-index
                else move sales-fold-row-index to sales-fold-next-prior-index end-if
                add 1 to sales-fold-next-count
            end-if
        when other move result-invalid to result-code
    end-evaluate.

market-view.
    *> Action 0: wanted status (-1 all; 0 active, 1 sold, 2 cancelled), row status.
    *> Action 1: count (0..20), triples created time/id/eligible. Outputs 62..64
    *> local index/time/id. Action 2 merges index/time/id pairs; outputs 17..19.
    evaluate market-action
        when select-market-listing-filter
            if listing-filter-wanted-status < -1 or listing-filter-wanted-status > 2 or
                listing-filter-status < 0 or listing-filter-status > 2
                move result-invalid to result-code exit paragraph end-if
            move 0 to listing-filter-include
            if listing-filter-wanted-status = -1 or listing-filter-wanted-status = listing-filter-status
                move 1 to listing-filter-include end-if
        when select-market-listing-select
            if listing-select-row-count < 0 or listing-select-row-count > 20 move result-invalid to
                result-code exit paragraph end-if
            move -1 to selection-best move 0 to selection-time selection-key
            perform varying selection-index from 1 by 1 until selection-index > listing-select-row-count
                if listing-candidate-eligible(selection-index) = 1 and (selection-best < 0
                    or listing-candidate-created-at(selection-index) > selection-time
                    or (listing-candidate-created-at(selection-index) = selection-time and
                        listing-candidate-id(selection-index) < selection-key))
                    compute selection-best = selection-index - 1
                    move listing-candidate-created-at(selection-index) to selection-time
                    move listing-candidate-id(selection-index) to selection-key
                end-if
            end-perform
            move selection-best to listing-select-index move selection-time to listing-select-time move
                selection-key to listing-select-id
        when select-market-listing-merge
            move listing-merge-current-index to listing-merge-index move listing-merge-current-time to
                listing-merge-time move listing-merge-current-id to listing-merge-id
            if listing-merge-candidate-index >= 0 and (listing-merge-current-index < 0 or
                listing-merge-candidate-time > listing-merge-current-time
                or (listing-merge-candidate-time = listing-merge-current-time and listing-merge-candidate-id
                    < listing-merge-current-id))
                move listing-merge-candidate-index to listing-merge-index move listing-merge-candidate-time
                    to listing-merge-time move listing-merge-candidate-id to listing-merge-id
            end-if
        when select-market-profile-view
            *> Disabled profile viewer.
            if profile-view-disabled = 1 move result-forbidden to result-code end-if
        when select-market-profile-offer
            *> Offerer/subject/projected offer status; only OPEN belongs in profile.
            move 0 to profile-offer-include
            if profile-offer-offerer = profile-offer-subject and profile-offer-status = 0 move 1 to
                profile-offer-include end-if
        when select-market-listing-count
            *> Active listing count/status fold.
            move listing-count-previous to listing-count-next
            if listing-count-status = 0 add 1 to listing-count-next end-if
        when select-market-account-view
            *> Account viewer: Nana, actor, subject, disabled.
            if account-view-nana = 0 and account-view-actor not = account-view-subject and
                account-view-disabled = 1
                move result-forbidden to result-code end-if
        when select-market-account-balance
            *> Member id/found/USD, NC/USD issuance, NC/USD member, disabled.
            *> Outputs 17 balance, 18 issuance flag, 19 disabled.
            move 0 to account-balance-issuance account-balance-next-disabled
            if account-balance-member = 0
                move 1 to account-balance-issuance
                move account-balance-nc-issuance to account-balance-balance
                if account-balance-usd = 1 move account-balance-usd-issuance to account-balance-balance
                    end-if
            else
                if account-balance-found = 0 move result-not-found to result-code exit paragraph end-if
                move account-balance-member-nc to account-balance-balance move account-balance-disabled to
                    account-balance-next-disabled
                if account-balance-usd = 1 move account-balance-member-usd to account-balance-balance end-if
            end-if
        when select-market-fulfillment-view
            *> Fulfillment viewer: Nana, actor, provider, recipient.
            move 0 to fulfill-view-visible
            if fulfill-view-nana = 1 or fulfill-view-actor = fulfill-view-provider or fulfill-view-actor =
                fulfill-view-recipient
                move 1 to fulfill-view-visible end-if
        when select-market-transaction-view
            *> Private transaction visibility: amount, actor, payer, payee.
            move 0 to transaction-view-visible
            if transaction-view-amount not = 0 or transaction-view-actor = transaction-view-payer or
                transaction-view-actor = transaction-view-payee
                move 1 to transaction-view-visible end-if
        when select-market-account-row
            *> Account transaction matching: USD/wanted USD, payer/payee/subject.
            move 0 to account-row-include
            if account-row-usd = account-row-wanted-usd and (account-row-payer = account-row-subject or
                account-row-payee = account-row-subject)
                move 1 to account-row-include end-if
        when select-market-screen-mail
            *> Screen mail: amount/USD/blank/reverse/loan/lotto, read, actor/payer/payee.
            if screen-mail-amount not = 0 or screen-mail-usd = 1 or screen-mail-blank = 1 or
                screen-mail-reversal = 1
                or screen-mail-loan = 1 or screen-mail-lotto = 1 move result-invalid to result-code exit
                    paragraph end-if
            if (screen-mail-read = 1 and screen-mail-actor not = screen-mail-payee)
                or (screen-mail-read = 0 and screen-mail-actor not = screen-mail-payer) move
                    result-forbidden to result-code end-if
        when select-market-newest-fold
            *> Newest-first book fold: count<=30, best index/id, then index/id pairs.
            *> Zero id marks a consumed row. Outputs follow the complete input scan.
            if newest-row-count < 0 or newest-row-count > 30 move result-invalid to result-code exit
                paragraph end-if
            move newest-current-index to selection-best move newest-current-id to selection-key
            perform varying selection-index from 1 by 1 until selection-index > newest-row-count
                if newest-candidate-id(selection-index) > 0 and
                    (selection-best < 0 or newest-candidate-id(selection-index) > selection-key)
                    move newest-candidate-row-index(selection-index) to selection-best
                    move newest-candidate-id(selection-index) to selection-key
                end-if
            end-perform
            move selection-best to newest-index move selection-key to newest-id
        when other move result-invalid to result-code
    end-evaluate.

bank-invariants.
    *> Action 0 currency header; 1 loan; 2 quote; 3 initial NC/USD sums;
    *> 4 member sums; 5 zero sums; 6 ledger header; 7 epoch; 8 correction;
    *> 9 retained transaction metadata; 10 history tail; 11 audit sequence;
    *> 12 audit tail; 13 table capacity; 14 gift; 15 art; 16 fulfillment;
    *> 17 fulfillment update; 19 wide flow; 20 final epoch. Corrupt returns 19.
    evaluate check-action
        when select-bank-currency
            *> Decimals, epoch, epoch count, corrections/count limit.
            if currency-check-decimals > 8 or currency-check-epoch > wallet-limit
                or currency-check-epoch-count not = currency-check-epoch + 1 or
                    currency-check-correction-count > currency-check-correction-capacity
                move result-corrupt to result-code end-if
        when select-bank-loan
            *> Loan id, state sequence, duplicate, lender/borrower/status,
            *> lender/borrower exist, amount/installment/rate days/payment days,
            *> principal/interest/principal due/interest due/remainder,
            *> accrued/update/last time/next due.
            if loan-check-id < 1 or loan-check-id > loan-check-sequence or loan-check-duplicate = 1 or
                loan-check-lender = loan-check-borrower
                or (loan-check-lender = 0 and loan-check-status not = 6 and loan-check-status not = 5)
                or (loan-check-lender not = 0 and loan-check-lender-exists = 0) or
                    loan-check-borrower-exists = 0
                or (loan-check-status = 6 and (loan-check-lender not = 0 or loan-check-principal not = 0 or
                    loan-check-interest not = 0))
                or loan-check-amount < 1 or loan-check-amount > command-amount-limit
                or loan-check-installment < 1 or loan-check-installment > loan-check-amount
                or (loan-check-rate-days not = 1 and loan-check-rate-days not = 7 and loan-check-rate-days
                    not = 30 and loan-check-rate-days not = 365)
                or (loan-check-payment-days not = 1 and loan-check-payment-days not = 7 and
                    loan-check-payment-days not = 30)
                or loan-check-principal < 0 or loan-check-principal > loan-check-amount or
                    loan-check-interest < 0
                or loan-check-principal-due < 0 or loan-check-principal-due > loan-check-principal or
                    loan-check-interest-due < 0 or loan-check-interest-due > loan-check-interest
                or loan-check-accrued-at > loan-check-last-time or loan-check-updated-at >
                    loan-check-last-time
                or (loan-check-status = 2 and (loan-check-next-due < minimum-clock or loan-check-next-due >
                    wallet-limit))
                move result-corrupt to result-code exit paragraph end-if
            compute wide-value = loan-check-principal + loan-check-interest
            compute denominator-value = basis-points-per-whole * loan-check-rate-days * seconds-per-day
            if wide-value > wallet-limit or loan-check-remainder >= denominator-value
                move result-corrupt to result-code end-if
        when select-bank-quote
            *> Quote scale, coins, cents per coin, decimals.
            if quote-check-decimals < 0 or quote-check-decimals > 8 or quote-check-scale not = 10 **
                quote-check-decimals
                or quote-check-coins < 1 or quote-check-coins > command-amount-limit
                or quote-check-cents-per-coin < 1 or quote-check-cents-per-coin > command-amount-limit
                move result-corrupt to result-code exit paragraph end-if
            compute wide-value = quote-check-coins * quote-check-cents-per-coin
            divide wide-value by quote-check-scale giving quotient-value remainder remainder-value
            if remainder-value not = 0 or quotient-value < 1 or quotient-value > command-amount-limit
                move result-corrupt to result-code end-if
        when select-bank-initial-sums
            *> Issuance, lotto escrow, USD issuance. Output 33/34 NC/USD sum.
            compute wide-value = initial-sums-nc-issuance + initial-sums-lotto-escrow
            if wide-value < min-i64 or wide-value > max-i64
                move result-corrupt to result-code exit paragraph end-if
            move wide-value to initial-sums-nc move initial-sums-usd-issuance to initial-sums-usd
        when select-bank-member-sums
            *> Running NC/USD, member NC/USD. Output 33/34 new sums.
            if member-sums-member-nc < -9007199254740991 or member-sums-member-nc > wallet-limit
                or member-sums-member-usd < -9007199254740991 or member-sums-member-usd > wallet-limit
                move result-corrupt to result-code exit paragraph end-if
            compute wide-value = member-sums-nc + member-sums-member-nc
            compute debit-value = member-sums-usd + member-sums-member-usd
            if wide-value < min-i64 or wide-value > max-i64
                or debit-value < min-i64 or debit-value > max-i64
                move result-corrupt to result-code exit paragraph end-if
            move wide-value to member-sums-next-nc move debit-value to member-sums-next-usd
        when select-bank-conservation
            if conservation-nc not = 0 or conservation-usd not = 0 move result-corrupt to result-code end-if
        when select-bank-ledger
            *> Transactions, sequence, epoch, epoch count, correction count/cap,
            *> audit count/cap, history count/cap, archived tx/floor/audit seq/first page.
            if ledger-check-transactions > ledger-check-sequence * 2 or ledger-check-transactions >
                18014398509481982 or ledger-check-epoch >= 32
                or ledger-check-epoch-count not = ledger-check-epoch + 1 or ledger-check-correction-count >
                    ledger-check-correction-capacity
                or ledger-check-audit-count > ledger-check-audit-capacity or ledger-check-history-count >
                    ledger-check-history-capacity
                or ledger-check-archived-count > ledger-check-transactions or ledger-check-retained-floor >
                    ledger-check-archived-count or ledger-check-audit-head > ledger-check-sequence
                or (ledger-check-first-page = 0 and ledger-check-history-count not = function
                    min(ledger-check-transactions, ledger-check-history-capacity))
                move result-corrupt to result-code end-if
        when select-bank-epoch
            *> Epoch index, decimals, exponent, previous decimals.
            if epoch-check-decimals > 8 or (epoch-check-index = 0 and (epoch-check-decimals not = 4 or
                epoch-check-exponent not = 0))
                or (epoch-check-index > 0 and (epoch-check-decimals - epoch-check-previous-decimals -
                    epoch-check-exponent < -12
                    or epoch-check-decimals - epoch-check-previous-decimals - epoch-check-exponent > 12))
                        move result-corrupt to result-code end-if
        when select-bank-correction
            *> Original id/units/refunded, full flag/id, duplicate, retained original
            *> found/reverse flag/amount.
            if correction-check-original-id < 1 or correction-check-original-id > wallet-limit or
                correction-check-original-amount < 0 or correction-check-original-amount >
                command-amount-limit
                or correction-check-refunded < 0 or correction-check-refunded >
                    correction-check-original-amount or correction-check-duplicate = 1
                or (correction-check-full = 1 and correction-check-refunded not =
                    correction-check-original-amount) or (correction-check-full = 0 and
                    correction-check-refunded = correction-check-original-amount)
                or (correction-check-full = 1 and (correction-check-full-id < 1 or correction-check-full-id
                    > wallet-limit or correction-check-full-id = correction-check-original-id))
                or (correction-check-original-retained = 1 and (correction-check-original-reversal = 1 or
                    correction-check-original-amount not = correction-check-retained-amount or
                    correction-check-refunded > correction-check-retained-amount
                    or (correction-check-full = 1 and correction-check-refunded not =
                        correction-check-retained-amount) or (correction-check-full = 0 and
                        correction-check-refunded = correction-check-retained-amount)))
                move result-corrupt to result-code end-if
        when select-bank-history-row
            *> Ordinal,total,floor,previous flag/value,epoch/current epoch,group/sequence,
            *> decimals,USD,epoch decimals,refund/original units,reverse flag.
            if history-check-ordinal >= history-check-total or history-check-ordinal < history-check-floor
                or (history-check-previous-present = 1 and history-check-ordinal not =
                    history-check-previous-ordinal + 1)
                or history-check-epoch > history-check-current-epoch or history-check-group < 1 or
                    history-check-group > history-check-sequence
                or (history-check-usd = 1 and history-check-decimals not = 2) or (history-check-usd = 0 and
                    history-check-decimals not = history-check-epoch-decimals)
                or history-check-refunded < 0 or history-check-refunded > command-amount-limit or
                    history-check-original-amount < 0 or history-check-original-amount >
                    command-amount-limit
                or (history-check-reversal = 1 and (history-check-refunded <= 0 or
                    history-check-original-amount <= 0 or history-check-refunded >
                    history-check-original-amount))
                move result-corrupt to result-code end-if
        when select-bank-history-tail
            *> Last ordinal present/value, transactions, first page.
            if (history-tail-last-present = 1 and history-tail-last-ordinal + 1 not =
                history-tail-transactions)
                or (history-tail-transactions > 0 and history-tail-last-present = 0 and
                    history-tail-first-page = 0) move result-corrupt to result-code end-if
        when select-bank-audit-sequence
            *> Audit sequence/head, timestamp/last time, previous flag/sequence.
            if audit-sequence-sequence > audit-sequence-head or audit-sequence-time >
                audit-sequence-last-time move result-corrupt to result-code exit paragraph end-if
            if audit-sequence-previous-present = 1
                compute wide-value = audit-sequence-previous-sequence + 1
                if function mod(audit-sequence-previous-sequence, 8192) = 4096 compute wide-value =
                    audit-sequence-previous-sequence + 4097 end-if
                if wide-value > 9007199254736895 or audit-sequence-sequence not = wide-value move
                    result-corrupt to result-code end-if
            end-if
        when select-bank-audit-tail
            if audit-tail-head > 0 and audit-tail-last-sequence not = audit-tail-head move result-corrupt to
                result-code end-if
        when select-bank-capacity
            if table-check-count > table-check-capacity move result-corrupt to result-code end-if
        when select-bank-gift
            *> Owner exists, id/sequence, received, target flag/value, title blank, duplicate.
            if gift-check-owner-exists = 0 move result-not-found to result-code exit paragraph end-if
            if gift-check-id < 1 or gift-check-id > gift-check-sequence or gift-check-received < 0 or
                gift-check-received > wallet-limit
                or (gift-check-target-present = 1 and (gift-check-target < 1 or gift-check-target >
                    command-amount-limit))
                or gift-check-title-blank = 1 or gift-check-duplicate = 1 move result-corrupt to result-code
                    end-if
        when select-bank-art
            *> Owner/creator exist, id/sequence/revision, price flag/value,
            *> valid metadata, duplicate id, equipped, prior equipped owner count.
            if art-check-owner-exists = 0 or art-check-creator-exists = 0 move result-not-found to
                result-code exit paragraph end-if
            if art-check-id < 1 or art-check-id > art-check-sequence or art-check-revision < art-check-id or
                art-check-revision > art-check-sequence
                or (art-check-price-present = 1 and (art-check-price < 1 or art-check-price >
                    command-amount-limit))
                or art-check-metadata-valid = 0 or art-check-duplicate = 1 or (art-check-equipped = 1 and
                    art-check-prior-equipped-count > 0)
                move result-corrupt to result-code end-if
        when select-bank-fulfillment
            *> Payment id/transaction, to/provider/from/recipient, amount, USD/reverse,
            *> correction present, status reversed, sequence, member existence,
            *> duplicate transaction, last update present/status/current status.
            if fulfill-check-payment-id not = fulfill-check-transaction-id or fulfill-check-payee not =
                fulfill-check-provider or fulfill-check-payer not = fulfill-check-recipient
                or fulfill-check-amount < 1 or fulfill-check-amount > command-amount-limit or
                    fulfill-check-usd = 1 or fulfill-check-reversal = 1
                or fulfill-check-correction-present not = fulfill-check-reversed-status or
                    fulfill-check-transaction-id < 1 or fulfill-check-transaction-id >
                    fulfill-check-sequence
                or fulfill-check-provider = fulfill-check-recipient or fulfill-check-provider-exists = 0 or
                    fulfill-check-recipient-exists = 0 or fulfill-check-duplicate = 1
                or fulfill-check-update-present = 0 or fulfill-check-updated-status not =
                    fulfill-check-current-status move result-corrupt to result-code end-if
        when select-bank-fulfillment-update
            *> Update sequence, state sequence, actor exists.
            if fulfill-update-sequence > fulfill-update-state-sequence or fulfill-update-actor-exists = 0
                move result-corrupt to result-code end-if
        when select-bank-wide-flow
            *> Wide value encodable, signed high/low base 10^15, tx count, flow index.
            if wide-check-encodable = 0 move result-corrupt to result-code exit paragraph end-if
            compute wide-value = wide-check-high * flow-parts-base + wide-check-low
            if function abs(wide-value) > 288230376151711712000000000000000
                or (wide-check-flow-index = 19 and (wide-value < 0 or wide-value >
                    wide-check-transaction-count)) move result-corrupt to result-code end-if
        when select-bank-epoch-tail
            *> Last epoch present/decimals, state decimals.
            if epoch-tail-present = 0 or epoch-tail-decimals not = epoch-tail-state-decimals move
                result-corrupt to result-code end-if
        when other move result-invalid to result-code
    end-evaluate.

audit-policy.
    *> Action 0=prepare: command tag provision/human/bot/token/update/setup/
    *> full key/read key/business; target/new member ids, role/present,
    *> disabled present/value, password/display present, cache count/capacity.
    *> Outputs 17..26 identity/member/name source/role present/value,
    *> disabled present/value, credentials changed, created, evict.
    *> Name source: 0 empty, 1 display, 2 username, 3 token name, 4 optional display.
    evaluate audit-action
        when select-audit-prepare
            if audit-prepare-command < 0 or audit-prepare-command > 8 or audit-prepare-cache-count >
                audit-prepare-cache-capacity or audit-prepare-cache-capacity < 1
                move result-invalid to result-code exit paragraph end-if
            move 0 to audit-prepare-identity audit-prepare-member audit-prepare-name-source
                audit-prepare-next-role-present audit-prepare-next-role
                audit-prepare-next-disabled-present audit-prepare-next-disabled
                    audit-prepare-credentials-changed audit-prepare-created audit-prepare-evict
            if audit-prepare-cache-count = audit-prepare-cache-capacity move 1 to audit-prepare-evict end-if
            if audit-prepare-command = 8 exit paragraph end-if
            move 1 to audit-prepare-identity
            move audit-prepare-target to audit-prepare-member
            evaluate audit-prepare-command
                when 0 when 1 when 2
                    move audit-prepare-new-member to audit-prepare-member
                    move 1 to audit-prepare-name-source audit-prepare-next-role-present
                        audit-prepare-credentials-changed audit-prepare-created
                    move audit-prepare-role to audit-prepare-next-role
                    if audit-prepare-command = 0 move 1 to audit-prepare-member audit-prepare-next-role
                        end-if
                    if audit-prepare-command = 2 move 0 to audit-prepare-next-role end-if
                when 3
                    move audit-prepare-new-member to audit-prepare-member
                    move 3 to audit-prepare-name-source move 1 to audit-prepare-credentials-changed
                        audit-prepare-created
                when 4
                    if audit-prepare-display-present = 1 move 4 to audit-prepare-name-source end-if
                    move audit-prepare-role-present to audit-prepare-next-role-present move
                        audit-prepare-role to audit-prepare-next-role
                    move audit-prepare-disabled-present to audit-prepare-next-disabled-present move
                        audit-prepare-disabled to audit-prepare-next-disabled
                    move audit-prepare-password-present to audit-prepare-credentials-changed
                when 5 move 2 to audit-prepare-name-source move 1 to audit-prepare-credentials-changed
                when 6 when 7 move 1 to audit-prepare-credentials-changed
            end-evaluate
        when select-audit-validate
            *> Sequence, actor, identity, member, created, name/role/status present,
            *> credentials changed, credential-bearing business variant.
            if audit-check-sequence < 1 or audit-check-sequence > wallet-limit or audit-check-actor < 0 or
                audit-check-actor > 32
                move result-corrupt to result-code exit paragraph end-if
            if audit-check-identity = 1
                if audit-check-member < 1 or audit-check-member > 32 move result-corrupt to result-code exit
                    paragraph end-if
                if audit-check-created = 1 and (audit-check-name-present = 0 or audit-check-status-present =
                    1 or audit-check-credentials-changed = 0)
                    move result-corrupt to result-code end-if
            else if audit-check-credential-business = 1 move result-corrupt to result-code end-if end-if
        when other move result-invalid to result-code
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
    evaluate activity-action
        when select-activity-event
            move 0 to activity-event-kind activity-event-prefix activity-event-next-amount-present
                activity-event-rate-present activity-event-coins-present
                activity-event-apr-present activity-event-closes-present activity-event-next-side
                    activity-event-lotto-present
            move activity-event-actor to activity-event-next-actor
            move -1 to activity-event-next-other
            move activity-event-subject to activity-event-next-subject
            move activity-event-amount to activity-event-next-amount
            move activity-event-lotto-kind to activity-event-next-lotto-kind
            move activity-event-rate to activity-event-next-rate
            move activity-event-coins to activity-event-next-coins
            move activity-event-apr to activity-event-next-apr
            move activity-event-closes-at to activity-event-next-closes-at
            evaluate activity-event-tag
                when 0
                    if activity-event-exists = 1 and activity-event-name-present = 1 and
                        activity-event-role-present = 1
                        and activity-event-status-present = 0 and activity-event-created-at =
                            activity-event-audit-time and activity-event-creation-marker = 1
                        move 1 to activity-event-kind move activity-event-owner to activity-event-next-actor
                    end-if
                when 1 when 2
                    move 2 to activity-event-kind move 1 to activity-event-prefix
                        activity-event-next-amount-present
                    move activity-event-side to activity-event-next-side
                    if activity-event-tag = 1 and activity-event-good-deed = 1 move 3 to activity-event-kind
                        end-if
                when 3 when 4
                    if activity-event-exists = 1
                        move 4 to activity-event-kind move 1 to activity-event-prefix
                            activity-event-next-amount-present
                        move activity-event-other to activity-event-next-other
                        if activity-event-tag = 3 move activity-event-owner to activity-event-next-actor
                            end-if
                        if activity-event-tag = 4 and activity-event-good-deed = 1 move 5 to
                            activity-event-kind end-if
                    end-if
                when 5
                    move 6 to activity-event-kind move 2 to activity-event-prefix
                    move 1 to activity-event-next-amount-present activity-event-closes-present
                        activity-event-lotto-present
                when 6
                    if activity-event-exists = 1 and activity-event-step = 0 and activity-event-winner = 1
                        move 7 to activity-event-kind move 2 to activity-event-prefix move 1 to
                            activity-event-next-amount-present
                        move activity-event-owner to activity-event-next-actor move activity-event-other to
                            activity-event-next-other
                    end-if
                when 7
                    move 8 to activity-event-kind move 3 to activity-event-prefix move 1 to
                        activity-event-next-amount-present activity-event-apr-present
                when 8
                    if activity-event-exists = 1
                        move 9 to activity-event-kind move 3 to activity-event-prefix move 1 to
                            activity-event-next-amount-present activity-event-apr-present
                        move activity-event-owner to activity-event-next-actor move activity-event-other to
                            activity-event-next-other
                    end-if
                when 9
                    if activity-event-exists = 1 and activity-event-loan-status = 3 and
                        activity-event-loan-updated = activity-event-audit-time
                        and activity-event-latest-payment = activity-event-selected-payment
                        move 10 to activity-event-kind move 3 to activity-event-prefix move 1 to
                            activity-event-next-amount-present
                        move activity-event-owner to activity-event-next-actor move activity-event-other to
                            activity-event-next-other
                    end-if
                when 10 when 11
                    if activity-event-tag = 10 or activity-event-exists = 1
                        compute activity-event-kind = activity-event-tag + 1
                        move 4 to activity-event-prefix move 1 to activity-event-rate-present
                            activity-event-coins-present
                        move activity-event-side to activity-event-next-side
                        if activity-event-tag = 11 move activity-event-other to activity-event-next-other
                            end-if
                    end-if
                when 12
                    move 13 to activity-event-kind move 5 to activity-event-prefix move
                        activity-event-amount-present to activity-event-next-amount-present
                when 13
                    move 14 to activity-event-kind move 6 to activity-event-prefix
                when 14 continue
                when other move result-invalid to result-code
            end-evaluate
        when select-activity-page
            *> Disabled, limit, after, row count/capacity, first exists/sequence,
            *> limit present. Outputs 33 limit, 34 truncated.
            if activity-page-disabled = 1 move result-forbidden to result-code exit paragraph end-if
            move 50 to activity-page-next-limit
            if activity-page-limit-present = 1
                if activity-page-limit < 1 or activity-page-limit > 50 move result-invalid to result-code
                    exit paragraph end-if
                move activity-page-limit to activity-page-next-limit
            end-if
            move 0 to activity-page-truncated
            compute wide-value = activity-page-after + 1
            if function mod(activity-page-after, 8192) = 4096
                compute wide-value = activity-page-after + 4097 end-if
            if activity-page-row-count = activity-page-capacity and activity-page-first-present = 1 and
                wide-value <= 9007199254736895
                and activity-page-first-sequence > wide-value move 1 to activity-page-truncated end-if
        when select-activity-row
            *> Sequence, after. Output 33 visible.
            move 0 to activity-row-visible
            if activity-row-sequence > activity-row-after move 1 to activity-row-visible end-if
        when select-activity-account
            *> Self account, disabled actor.
            if activity-account-self = 0 and activity-account-disabled = 1 move result-forbidden to
                result-code end-if
        when select-activity-payment-selection
            *> Up to 60 joined payment sequences, select the latest.
            if activity-payment-row-count < 0 or activity-payment-row-count > 60 move result-invalid to
                result-code exit paragraph end-if
            move 0 to selection-best selection-time
            perform varying selection-index from 1 by 1 until selection-index > activity-payment-row-count
                if selection-best = 0 or activity-payment-candidate-sequence(selection-index) >
                    selection-time
                    move selection-index to selection-best
                    move activity-payment-candidate-sequence(selection-index) to selection-time
                end-if
            end-perform
            compute activity-payment-selected-index = selection-best - 1
            move selection-time to activity-payment-sequence
        when other move result-invalid to result-code
    end-evaluate.

ledger-projection.
    *> Action 0=transaction: amount, reverse/listing/loan/lotto/quote flags,
    *> from/to. Outputs 17 kind, 18 reference, 19 debit, 20 credit, 21 public.
    *> 1=cursor: incarnation matches, upper/before/page,total/next/first,limit.
    *> 2=row: ordinal,before,upper,amount,actor present/id,from/to,
    *> account present/id,USD/account USD. 3=continuation, 4=retention,
    *> 5=archive insertion, 6=descending ordinal key, 7=circulation.
    evaluate ledger-action
        when select-ledger-transaction
            move 5 to ledger-tx-kind
            evaluate true
                when ledger-tx-amount = 0 move 0 to ledger-tx-kind
                when ledger-tx-reversal = 1 move 1 to ledger-tx-kind
                when ledger-tx-payer = 0 move 2 to ledger-tx-kind
                when ledger-tx-listing = 1 move 3 to ledger-tx-kind
                when ledger-tx-payee = 0 move 4 to ledger-tx-kind
            end-evaluate
            move 0 to ledger-tx-reference ledger-tx-public
            evaluate true
                when ledger-tx-loan = 1 move 1 to ledger-tx-reference
                when ledger-tx-lotto = 1 move 2 to ledger-tx-reference
                when ledger-tx-quote = 1 move 3 to ledger-tx-reference
            end-evaluate
            if ledger-tx-amount = min-i64 move result-overflow to result-code exit paragraph end-if
            compute ledger-tx-debit = 0 - ledger-tx-amount
            move ledger-tx-amount to ledger-tx-credit
            if ledger-tx-amount not = 0 move 1 to ledger-tx-public end-if
        when select-ledger-cursor
            if ledger-cursor-incarnation-matches = 0 or ledger-cursor-upper > ledger-cursor-total or
                ledger-cursor-before > ledger-cursor-upper
                or ledger-cursor-page > ledger-cursor-next-page or ledger-cursor-page <
                    ledger-cursor-first-page
                move result-stale to result-code exit paragraph
            end-if
            compute ledger-cursor-effective-limit = function min(100, function max(1, ledger-cursor-limit))
        when select-ledger-row
            move 0 to ledger-row-include
            if ledger-row-ordinal < ledger-row-before and ledger-row-ordinal < ledger-row-upper
                and (ledger-row-amount not = 0 or (ledger-row-actor-present = 1
                    and (ledger-row-payer = ledger-row-actor or ledger-row-payee = ledger-row-actor)))
                and (ledger-row-account-present = 0 or (ledger-row-usd = ledger-row-account-usd
                    and (ledger-row-payer = ledger-row-account or ledger-row-payee = ledger-row-account)))
                move 1 to ledger-row-include
            end-if
        when select-ledger-continuation
            move 0 to ledger-more-present
            if ledger-more-count > 0 and (ledger-more-count-limit = ledger-more-retained-limit or
                ledger-more-page > ledger-more-first-page)
                move 1 to ledger-more-present
            end-if
        when select-ledger-retention
            move 0 to ledger-retention-truncated
            if ledger-retention-archive-present not = 0 or (ledger-retention-first-page = 0 and
                ledger-retention-total > ledger-retention-retained-capacity)
                move 1 to ledger-retention-truncated
            end-if
        when select-ledger-insertion
            move 0 to ledger-insert-position
            if ledger-insert-archive-enabled = 1
                if ledger-insert-ordinal < ledger-insert-floor move 1 to ledger-insert-position
                else if ledger-insert-last-page > ledger-insert-next-page move 2 to ledger-insert-position
                    end-if end-if
            end-if
        when select-ledger-descending-key when select-ledger-circulation
            if ledger-key-ordinal = min-i64 move result-overflow to result-code exit paragraph end-if
            compute ledger-key-descending = 0 - ledger-key-ordinal
        when select-ledger-ordinal-selection
            *> Count, up to 60 ordinals. Output zero-based minimum index/value.
            if ledger-select-row-count < 0 or ledger-select-row-count > 60 move result-invalid to
                result-code exit paragraph end-if
            move 0 to selection-best selection-time
            perform varying selection-index from 1 by 1 until selection-index > ledger-select-row-count
                if selection-best = 0 or ledger-candidate-ordinal(selection-index) < selection-time
                    move selection-index to selection-best
                    move ledger-candidate-ordinal(selection-index) to selection-time
                end-if
            end-perform
            compute ledger-select-selected-index = selection-best - 1
            move selection-time to ledger-select-ordinal
        when select-ledger-audit-cursor
            *> Audit cursor: incarnation matches, page/next/first.
            if audit-cursor-incarnation-matches = 0 or audit-cursor-page > audit-cursor-next-page or
                audit-cursor-page < audit-cursor-first-page
                move result-stale to result-code end-if
        when select-ledger-audit-row
            *> Audit sequence, before.
            move 0 to audit-row-include
            if audit-row-sequence < audit-row-before move 1 to audit-row-include end-if
        when select-ledger-audit-continuation
            *> Audit next sequence, count, page/first. Fixed 16-row page.
            move 0 to audit-more-present
            if audit-more-next-sequence > 1 and (audit-more-count = 16 or audit-more-page >
                audit-more-first-page)
                move 1 to audit-more-present end-if
        when select-ledger-audit-retention
            *> Audit retained floor.
            move 0 to audit-retention-truncated
            if audit-retention-floor > 0 move 1 to audit-retention-truncated end-if
        when select-ledger-audit-initial
            *> Initial audit before value is head sequence plus one.
            compute wide-value = audit-initial-head-sequence + 1
            if wide-value > max-i64 move result-overflow to result-code
            else move wide-value to audit-initial-before end-if
        when other move result-invalid to result-code
    end-evaluate.

identity-text-policy.
    *> kind 0=name, 1=Mastodon. Name: blank/control facts.
    *> Mastodon: empty, user empty, host empty, extra @, host dot, bad Unicode/slash.
    evaluate text-action
        when select-identity-name
            if name-blank = 1 or name-control = 1 move result-invalid to result-code end-if
        when select-identity-mastodon
            if mastodon-empty = 0 and (mastodon-user-empty = 1 or mastodon-host-empty = 1 or
                mastodon-extra-at = 1
                or mastodon-host-has-dot = 0 or mastodon-invalid-text = 1) move result-invalid to
                    result-code end-if
        when select-identity-creation
            *> Creation: display blank, bot, Nana, grant requested/configured.
            if creation-bot = 1 and creation-nana = 1 move result-invalid to result-code exit paragraph
                end-if
            move creation-display-blank to creation-default-display
            move 0 to creation-grant
            if creation-grant-requested = 1 move creation-configured-grant to creation-grant end-if
        when select-identity-wire-update
            *> Wire update guard: actor Nana, self, role/status present.
            if wire-update-actor-nana = 0 and (wire-update-self = 0 or wire-update-role-present = 1 or
                wire-update-status-present = 1)
                move result-forbidden to result-code
            end-if
        when select-identity-free-text
            *> Free-text control fact: blank correction reasons remain allowed.
            if free-text-control = 1 move result-invalid to result-code end-if
        when select-identity-provision
            *> Provisioning requires an empty current member table.
            if provision-member-count not = 0 move result-forbidden to result-code end-if
        when select-identity-quantity
            *> Parsed decimal quantity: whole/fraction/fraction digit count.
            if quantity-whole < 0 or quantity-whole > max-u32 or quantity-fraction < 0
                or quantity-fraction-digits < 0 or quantity-fraction-digits > 3
                move result-invalid to result-code exit paragraph end-if
            if quantity-fraction >= 10 ** quantity-fraction-digits
                move result-invalid to result-code exit paragraph end-if
            compute wide-value = quantity-whole * 1000 + quantity-fraction * (10 ** (3 -
                quantity-fraction-digits))
            if wide-value < 1 or wide-value > 1000000000
                move result-invalid to result-code
            else move wide-value to quantity-thousandths end-if
        when other move result-invalid to result-code
    end-evaluate.

transfer-policy.
    *> Inputs 1..11 match account-posting. Metadata 14..24: classified,
    *> recipient Nana, blank/control memo, quantity, kind (1 labor), unit,
    *> thing specified/exists/kind/unit. Outputs 7/8: posting balances.
    if transfer-target-exists = 0 move result-not-found to result-code exit paragraph end-if
    if transfer-classified = 0 and transfer-amount = 0
        if transfer-payer = transfer-payee move result-self-deal to result-code exit paragraph end-if
        if transfer-target-disabled = 1 move result-disabled to result-code exit paragraph end-if
        if transfer-memo-blank = 1 or transfer-memo-control = 1 move result-invalid to result-code exit
            paragraph end-if
        move transfer-source-balance to post-next-source-balance
        move transfer-target-balance to post-next-target-balance
        exit paragraph
    end-if
    perform account-posting
    if result-code not = result-success exit paragraph end-if
    if transfer-classified = 1
        if transfer-quantity < 1 or transfer-quantity > 1000000000
            move result-invalid to result-code exit paragraph
        end-if
        if transfer-economic-kind = 1 and transfer-recipient-nana = 1 move result-forbidden to result-code
            exit paragraph end-if
        if transfer-thing-present = 1
            if transfer-thing-exists = 0 move result-not-found to result-code exit paragraph end-if
            if transfer-economic-kind not = transfer-thing-kind or transfer-unit not = transfer-thing-unit
                move result-invalid to result-code
            end-if
        end-if
    end-if.

