"""Strict post-deployment TLS, API and bundled-site probe for one NanaCoin bank.

The board is required. miniframework's boardsafe probe verifies the
certificate for that bank's hostname, the app and host the board reports,
every bundled asset and the served CA; this script adds the ledger checks.
Either bank answering for the other fails.
"""
import argparse
import json

from boards import BOARDS, board
from boardsafe.probe import context_for, get, probe


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--board", required=True, choices=sorted(BOARDS), help="s3 (nanacoin.local) or s2 (nanacoin-s2.local)")
    parser.add_argument("--address", required=True, help="Board IP address or resolvable name")
    parser.add_argument("--attempts", type=int, default=15, help="Boot-time connection attempts")
    args = parser.parse_args()
    b = board(args.board)
    ca_pem = b.root / b.ca
    info = probe(b, args.address, ca_pem, args.attempts)
    context = context_for(ca_pem)

    def fetch(path):
        status, body, _ = get(args.address, b.hostname, context, path)
        if status != 200:
            raise SystemExit(f"{path} returned HTTP {status}")
        return body

    status = json.loads(fetch("/api/v1/status"))
    if not status.get("ledger_balanced"):
        raise SystemExit("Board reports an unbalanced ledger")
    site = fetch("/")
    if b"<app-root" not in site or b'name="nanacoin-api" content=""' not in site:
        raise SystemExit("Bundled Angular shell is not configured for the same-origin API")

    # Intentionally anonymous: the public book and machine health must work
    # before login, not merely with a stale token on the build PC.
    ledger = json.loads(fetch("/api/v1/transactions?limit=1"))
    if not isinstance(ledger.get("transactions"), list) or "circulation" not in ledger:
        raise SystemExit("Public ledger response has the wrong shape")

    # NanaCoin's own record of which bank this is.
    identity = json.loads(fetch("/api/v1/diag/static"))
    if identity.get("board") != b.id or identity.get("hostname") != b.hostname:
        raise SystemExit(
            f"Address {args.address} is board {identity.get('board')!r} ({identity.get('hostname')!r}), "
            f"not {b.id} ({b.hostname}). Stop: wrong bank."
        )
    health = json.loads(fetch("/api/v1/diag"))
    if not all(field in health for field in ("uptime_seconds", "free_heap", "samples")):
        raise SystemExit("Public board health response has the wrong shape")

    detail = info["_probe"]
    print(
        f"Board probe passed for {b.id} ({b.hostname}, {identity.get('platform')}): strict CA/hostname verification, "
        f"{detail['cipher']}, API, balanced ledger, {detail['assets']} exact assets "
        f"({detail['encodings']} + ETags, concurrent keep-alive and slow-reader checks), public notebook, "
        "public board health and matching /ca."
    )


if __name__ == "__main__":
    main()
