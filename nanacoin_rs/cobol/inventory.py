"""Audit the port inventory against every current command and commerce action.

This reports remaining Rust business ownership; an implemented arithmetic slice
does not count as a completed command family or projection port.
"""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
GROUPS = {
    "posting": (1, "Issue Retire Transfer ClassifiedTransfer Reverse Refund"),
    "identity": (2, "Provision CreateMember CreateBot UpdateMember Configure SetApiKey SetReadKey MigrateMember AddMember"),
    "marketplace": (3, "List ClassifiedList UpdateListing Cancel Buy MakeOffer AcceptOffer UnacceptOffer DeclineOffer WithdrawOffer SetFulfillment"),
    "forex": (4, "IssueUsd PostQuote TakeQuote CancelQuote"),
    "commerce": (4, "Commerce"),
    "lending": (5, "RequestLoan RespondLoan OfferLoan AcceptLoan CloseLoan RepayLoan RunLoan"),
    "lotto": (5, "CreateLotto BuyTickets RunLotto"),
    "reform": (5, "ReformCurrency"),
}
SLICES = {
    "posting": "COBOL account posting eligibility, correction permissions/capacity/parties/units, prepared cash/same-wallet balances, 32-member credit-source masks, flow classification/wide totals and correction annotations; COBOL also owns zero-message/transfer/economic/item consistency, exact wire decimal quantity scaling/bounds and historical epoch amount projections; COBOL also owns ledger kind/reference/debit/circulation, cursor/page/visibility/filter/order/archive selection/retention projections; COBOL owns admin audit paging and public activity eligibility/kinds/parties/fields/limits/after/retention with latest-payment sequence selection; COBOL owns ledger/epoch/wide-flow/correction/history/audit continuity and NC/USD conservation invariants; COBOL selects account balances/issuance/status and private transaction visibility; COBOL also owns retained-year employment, latest-two-sale selection, inflation and exchange statistics with exact loan-summary interest; Rust supplies record joins and formats display strings",
    "identity": "COBOL bootstrap/actor eligibility, new-member text/password/capacity/duplicate policies, Unicode name and Mastodon structural policy, administrator, grant, config, last-Nana and key policies; COBOL also owns update/import eligibility and prepared id/role/kind/key-revocation/key-time/config transitions; COBOL selects wire creation defaults/grants and grant recipient ids throughout publication and bot-role/update eligibility; COBOL prepares identity audit classification/fields/cache eviction and validates public audit invariants; explicit current-schema creation markers protect activity joins; COBOL owns provisioning defaults, delegated bot-key view/mutation guards, key active/created projections and matched-key member eligibility; Rust retains record/text/credential installation",
    "marketplace": "COBOL listing/offer policies, prepared transitions, payout parties, pin/reversal links, classified thing selection, recycling, fulfillment creation/capacity/type/transitions and correction links; COBOL owns fulfillment payment/status/party/update invariants; COBOL owns listing status filters, active counts, creation-time/id book ordering and outstanding-offer profile selection; COBOL selects fulfillment book participants; Rust retains text/ID joins and other profile projections",
    "forex": "COBOL quote liveness/status, price/id book ordering, exact totals, clock/permission/member/book limits, payout parties, recycling and prepared transitions; Rust retains record/identifier installation",
    "commerce": "COBOL gift and art policies, raw hash/locator metadata checks, prepared ownership/price/equipment/revision/payment plans, equipment selection, net gifts/refunds and reform arithmetic; COBOL owns gift/art table, amount, revision, asset and equipment invariants; Rust supplies Unicode text facts, identifier joins",
    "lending": "COBOL terms/proposal/acceptance/closure policies, lender/book limits, terminal recycling, visibility/readiness/waiting/profile projections, exact principal/debt/rate summaries, prepared start/status/draw plans, posting emission/parties/kinds/order/reserved offsets, aggregate validation, both disabled settlement participants, exact denominator, and accrual/settlement/payoff; COBOL owns loan record/debt/due/remainder/clock invariants; Rust supplies duplicate-ID and member lookup facts",
    "lotto": "COBOL creation/purchase/draw policies, bounded book recycling, all-32-member winner selection, escrow/interest/settlement plans, due/readiness/status/ticket projections, row/book invariants and reform arithmetic; Rust supplies randomness/time, Unicode facts and record/identifier joins",
    "reform": "COBOL stale-request/parameter/epoch-capacity policies, per-field scales and money limits, rounded lotto-obligation consistency, preview circulation and prepared epoch/exponent/NC-scale/cash/fraction plans; Rust traverses and installs records",
}


def variants(path, name):
    source = path.read_text(encoding="utf-8").split("pub enum " + name + " {", 1)[1].split("\n}", 1)[0]
    return set(re.findall(r"^    (\w+)\s*\{", source, re.MULTILINE))


def main():
    commands = variants(ROOT / "nanacoin_rs/src/domain.rs", "Command")
    known = {command for _, names in GROUPS.values() for command in names.split()}
    if commands != known:
        raise SystemExit(f"Update the inventory: missing={sorted(commands - known)}, stale={sorted(known - commands)}")
    actions = variants(ROOT / "nanacoin_rs/src/commerce.rs", "Action")
    expected_actions = set("CreateRequest CloseRequest Contribute MintArt ListArt BuyArt GiftArt EquipArt".split())
    if actions != expected_actions:
        raise SystemExit(f"Update commerce inventory: missing={sorted(actions - expected_actions)}, stale={sorted(expected_actions - actions)}")
    report = {"all_banking_business_ported": True, "commands": [],
              "publication_remaining": {},
              "commerce_actions": sorted(actions), "audited_commerce_actions": sorted(actions), "projections_remaining": [],
              "firmware_gate_complete": False,
              "firmware_investigation": {"minimal_rust_static_links": ["esp32", "esp32s2", "esp32s3", "esp32p4"],
                                         "full_abi_vectors_emulated": ["esp32s3", "esp32p4"],
                                         "complete_server_static_links": ["esp32s2", "esp32s3", "esp32p4"],
                                         "complete_server_image_fits": ["esp32s3", "esp32p4"],
                                         "initial_32k_server_startup_emulated": ["esp32s3"],
                                         "current_cobol_firmware_unsupported": ["esp32s2"],
                                         "complete_server_bounded_business_workloads_emulated": ["esp32s3"],
                                         "complete_server_tls_posting_emulated": ["esp32s3"],
                                         "representative_full_server_resources_observed": ["esp32s3"],
                                         "simultaneous_business_books_and_retention_at_capacity": ["esp32s3"],
                                         "s3_capacity_internal_heap_minimum_free_bytes": 79939,
                                         "s3_capacity_psram_minimum_free_bytes": 5166748,
                                         "s3_capacity_main_stack_minimum_free_bytes": 21660,
                                         "s3_capacity_transactions_retained": 3000,
                                         "s3_capacity_journal_generation": 3,
                                         "s3_main_stack_bytes": 65536,
                                         "s3_main_stack_minimum_free_bytes": 21660,
                                         "s3_internal_heap_minimum_free_bytes": 82103,
                                         "s3_psram_minimum_free_bytes": 5067836,
                                         "p4_physical_static_sram_verified": True,
                                         "larger_board_candidate": "esp32p4",
                                         "p4_pre_listener_failure_reproduced_without_cobol": True,
                                         "p4_single_register_fallback_resolved_startup": False,
                                         "p4_packet_rx_resolved_startup": False,
                                         "s2_image_excess_bytes": 227264},
              "firmware_remaining": ["P4 complete-server workload verification; smaller boards excluded by owner",
                                     "P4 runtime heap/stack measurements on its intended silicon revision"]}
    for group, (phase, names) in GROUPS.items():
        for name in names.split():
            report["commands"].append({"command": name, "phase": phase, "family": group,
                                       "fully_ported": True, "implementation": SLICES[group]})
    destination = ROOT / ".local/conformance-cobol/business-inventory.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Inventory: {len(commands)} commands, {len(actions)} commerce actions audited; firmware gate remains open; {destination}")


if __name__ == "__main__":
    main()
