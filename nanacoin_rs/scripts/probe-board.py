"""Strict post-deployment TLS, API and bundled-site probe for one NanaCoin bank.

The board is required. The probe verifies the certificate for that bank's
hostname and fails if the live board reports any other board or hostname, so
the S3 bank can never pass as the S2 bank or vice versa.
"""
import argparse
import gzip
import http.client
import json
from pathlib import Path
import socket
import ssl
import time
from asset_probe import bundled_assets, verify_board_assets
from boards import BOARDS, board

ROOT = Path(__file__).resolve().parents[1]


def bundled_index(web_dir):
    """Return the exact index bytes embedded by this board's most recent build."""
    for uri, identity, _, _ in bundled_assets(web_dir):
        if uri == "/index.html":
            return identity
    raise SystemExit("Could not identify the locally bundled index.html")


def get(address: str, hostname: str, context: ssl.SSLContext, path: str):
    raw = socket.create_connection((address, 443), timeout=5)
    tls = context.wrap_socket(raw, server_hostname=hostname)
    cipher = tls.cipher()
    # Browsers always accept gzip; the gzip-only S2 has no identity copy.
    tls.sendall(
        f"GET {path} HTTP/1.1\r\nHost: {hostname}\r\nAccept-Encoding: gzip\r\n"
        "Connection: close\r\n\r\n".encode()
    )
    response = http.client.HTTPResponse(tls)
    response.begin()
    body = response.read()
    if response.getheader("Content-Encoding") == "gzip":
        body = gzip.decompress(body)
    status = response.status
    tls.close()
    return status, body, cipher


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--board", required=True, choices=sorted(BOARDS), help="s3 (nanacoin.local) or s2 (nanacoin-s2.local)")
    parser.add_argument("--address", required=True, help="Board IP address or resolvable name")
    parser.add_argument("--attempts", type=int, default=15, help="Boot-time connection attempts")
    args = parser.parse_args()
    b = board(args.board)
    args.hostname = b.hostname

    ca_pem = ROOT / "certs/home-ca.crt"
    ca_der = ROOT / "certs/home-ca.der"
    context = ssl.create_default_context(cafile=str(ca_pem))
    context.minimum_version = ssl.TLSVersion.TLSv1_2

    last_error = None
    for _ in range(args.attempts):
        try:
            api_status, api_body, cipher = get(args.address, args.hostname, context, "/api/v1/status")
            break
        except (OSError, ssl.SSLError) as error:
            last_error = error
            time.sleep(2)
    else:
        raise SystemExit(f"Board did not pass strict TLS after {args.attempts} attempts: {last_error}")

    if api_status != 200:
        raise SystemExit(f"Status endpoint returned HTTP {api_status}")
    status = json.loads(api_body)
    if not status.get("ledger_balanced"):
        raise SystemExit("Board reports an unbalanced ledger")

    site_status, site, _ = get(args.address, args.hostname, context, "/")
    if site_status != 200 or b"<app-root" not in site:
        raise SystemExit("Bundled Angular shell is not being served")
    if b'name="nanacoin-api" content=""' not in site:
        raise SystemExit("Bundled Angular shell is not configured for the same-origin API")
    if site != bundled_index(b.web_dir):
        raise SystemExit("Board is serving an Angular bundle different from this deployment build")

    # These are intentionally anonymous. The public book and machine health
    # must work before login, not merely with a stale token on the build PC.
    ledger_status, ledger_body, _ = get(
        args.address, args.hostname, context, "/api/v1/transactions?limit=1"
    )
    if ledger_status != 200:
        raise SystemExit(f"Public ledger returned HTTP {ledger_status}")
    ledger = json.loads(ledger_body)
    if not isinstance(ledger.get("transactions"), list) or "circulation" not in ledger:
        raise SystemExit("Public ledger response has the wrong shape")

    # The board's own identity: a different bank answering this address fails.
    info_status, info_body, _ = get(args.address, args.hostname, context, "/api/v1/diag/static")
    if info_status != 200:
        raise SystemExit(f"Board identity endpoint returned HTTP {info_status}")
    info = json.loads(info_body)
    if info.get("board") != b.id or info.get("hostname") != b.hostname:
        raise SystemExit(
            f"Address {args.address} is board {info.get('board')!r} ({info.get('hostname')!r}), "
            f"not {b.id} ({b.hostname}). Stop: wrong bank."
        )

    diag_status, diag_body, _ = get(args.address, args.hostname, context, "/api/v1/diag")
    if diag_status != 200:
        raise SystemExit(f"Public board health returned HTTP {diag_status}")
    diag = json.loads(diag_body)
    if not all(field in diag for field in ("uptime_seconds", "free_heap", "samples")):
        raise SystemExit("Public board health response has the wrong shape")

    ca_status, served_ca, _ = get(args.address, args.hostname, context, "/ca")
    if ca_status != 200 or served_ca != ca_der.read_bytes():
        raise SystemExit("Board /ca does not match the CA used to verify its server certificate")

    asset_count = verify_board_assets(args.address, args.hostname, context, web_dir=b.web_dir)
    encodings = "gzip-only + 406 identity" if bundled_assets(b.web_dir)[0][3] else "identity + gzip"
    print(
        f"Board probe passed for {b.id} ({b.hostname}, {info.get('platform')}): strict CA/hostname verification, "
        f"{cipher[0]}, API, balanced ledger, {asset_count} exact assets "
        f"({encodings} + ETags, concurrent keep-alive and slow-reader checks), public notebook, "
        "public board health and matching /ca."
    )


if __name__ == "__main__":
    main()
