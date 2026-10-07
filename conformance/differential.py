"""Compare public banking JSON from deterministic scenarios on two executables.

Uses only the same launcher and HTTP interface as the baseline contracts.
Authentication responses are excluded; incarnation identifiers are normalized.
All financial values, business identifiers, times and object states stay exact.
"""
import argparse
import json
from pathlib import Path
import unittest

import run as contracts


SCENARIOS = (
    "issue_retire_transfer_and_reversal",
    "invalid_money_does_not_mutate_balances",
    "account_currency_views_disabled_targets_and_cross_build_restart",
    "offer_accept_undo_is_atomic_and_retry_stable",
    "marketplace_labor_role_subject_and_prepared_fulfillment_recovery",
    "newest_first_offer_loan_lotto_books_and_cross_build_recovery",
    "forex_exchanges_both_legs_once",
    "forex_cash_leg_failure_then_trade_retry_across_build_restart",
    "gift_contribution_close_refund_and_restart",
    "art_sale_is_atomic_and_checks_revision",
    "fulfillment_dispute_completion_and_reversal",
    "exact_currency_reform_preview_and_restart",
    "configuration_and_member_grants",
    "loan_proposal_acceptance_and_repayment",
    "disabled_lender_pauses_settlement_and_failed_key_recovers_across_build",
    "clock_loan_interest_payment_and_cross_build_restart",
    "lotto_ticket_escrow_receipts_and_restart",
    "simple_lotto_settles_once_and_cannot_sell_late_tickets",
)


def normalize(value):
    if isinstance(value, dict):
        return {key: "<bank-incarnation>" if key == "incarnation" else normalize(item)
                for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [normalize(item) for item in value]
    return value


class RecordedContract(contracts.BankContract):
    def request(self, path, *args, **kwargs):
        status, body = super().request(path, *args, **kwargs)
        # Startup polling and ephemeral authentication are framework operations.
        if (not getattr(self, "polling", False)
                and path != "/status" and not path.startswith("/auth/")):
            self.trace.append(dict(path=path, status=status, body=normalize(body)))
        return status, body

    def wait_for(self, read, predicate, timeout=10):
        # Scheduler thread timing changes the number of polling reads. Compare
        # the complete condition-satisfying observation, never intermediate
        # reads or the number of attempts. The contract still checks readiness.
        self.polling = True
        try:
            value = super().wait_for(read, predicate, timeout)
        finally:
            self.polling = False
        self.trace.append(dict(observation="wait_for", body=normalize(value)))
        return value


def scenario(server, restart, name):
    contracts.SERVER = str(server.resolve())
    contracts.RESTART_SERVER = str(restart.resolve())
    contracts.CLOCK = True
    contracts.CLOCK_START = 1_800_000_000
    case = RecordedContract("test_" + name)
    case.trace = []
    result = unittest.TestResult()
    case.run(result)
    if not result.wasSuccessful():
        raise AssertionError("\n".join(detail for _, detail in result.errors + result.failures))
    return case.trace


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--cobol", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = dict(profile="nanacoin-json-v1-differential", scenarios=[])
    for name in SCENARIOS:
        rust = scenario(args.rust, args.cobol, name)
        cobol = scenario(args.cobol, args.rust, name)
        entry = dict(name=name, equal=rust == cobol, rust=rust, cobol=cobol)
        report["scenarios"].append(entry)
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        if not entry["equal"]:
            raise AssertionError(f"Public JSON differs for {name}; inspect {args.report}")
        print(f"{name}: {len(rust)} responses match", flush=True)
    print(f"{len(SCENARIOS)} differential scenarios passed; {args.report}", flush=True)


if __name__ == "__main__":
    main()
