"""Strict post-deployment TLS, API and bundled-site probe for a NanaCoin board."""
import argparse
import http.client
import json
from pathlib import Path
import re
import socket
import ssl
import time

ROOT = Path(__file__).resolve().parents[1]


def bundled_index():
    """Return the exact index bytes embedded by the most recent local build."""
    generated = (ROOT / ".embuild" / "web" / "assets.rs").read_text(encoding="utf-8")
    match = re.search(r'path: "/index\.html".*?web/(\d+)\.raw', generated)
    if not match:
        raise SystemExit("Could not identify the locally bundled index.html")
    return (ROOT / ".embuild" / "web" / f"{match.group(1)}.raw").read_bytes()


def get(address: str, hostname: str, context: ssl.SSLContext, path: str):
    raw = socket.create_connection((address, 443), timeout=5)
    tls = context.wrap_socket(raw, server_hostname=hostname)
    cipher = tls.cipher()
    tls.sendall(
        f"GET {path} HTTP/1.1\r\nHost: {hostname}\r\nConnection: close\r\n\r\n".encode()
    )
    response = http.client.HTTPResponse(tls)
    response.begin()
    body = response.read()
    status = response.status
    tls.close()
    return status, body, cipher


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True, help="Board IP address or resolvable name")
    parser.add_argument("--hostname", default="nanacoin.local", help="TLS SNI and certificate hostname")
    parser.add_argument("--attempts", type=int, default=15, help="Boot-time connection attempts")
    args = parser.parse_args()

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
    if site != bundled_index():
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

    diag_status, diag_body, _ = get(args.address, args.hostname, context, "/api/v1/diag")
    if diag_status != 200:
        raise SystemExit(f"Public board health returned HTTP {diag_status}")
    diag = json.loads(diag_body)
    if not all(field in diag for field in ("uptime_seconds", "free_heap", "samples")):
        raise SystemExit("Public board health response has the wrong shape")

    ca_status, served_ca, _ = get(args.address, args.hostname, context, "/ca")
    if ca_status != 200 or served_ca != ca_der.read_bytes():
        raise SystemExit("Board /ca does not match the CA used to verify its server certificate")

    print(
        "Board probe passed: strict CA/hostname verification, "
        f"{cipher[0]}, API, balanced ledger, exact Angular build, public notebook, "
        "public board health and matching /ca."
    )


if __name__ == "__main__":
    main()
