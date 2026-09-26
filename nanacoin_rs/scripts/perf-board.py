"""Read-only board probe: separate connection, TLS, response and download time.

Uses the household CA and hostname verification even when connecting by IP.
No login, writes, resets, serial access or automatic retries.
"""
import argparse
import http.client
import json
import math
from pathlib import Path
import socket
import ssl
import statistics
import time

ROOT = Path(__file__).resolve().parents[1]
PATHS = ("/api/v1/diag", "/api/v1/status", "/api/v1/transactions?limit=1")
LIMIT = 512 * 1024


def measure(args, protocol, mode, context):
    connection = None
    rows = []
    try:
        for sample in range(args.samples):
            for path in PATHS:
                row = dict(protocol=protocol, mode=mode, sample=sample + 1, path=path)
                started = time.perf_counter()
                try:
                    reused = connection is not None
                    tcp_ms = tls_ms = 0.0
                    if connection is None:
                        connection = socket.create_connection(
                            (args.address, 443 if protocol == "https" else 80), args.timeout
                        )
                        connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                        connected = time.perf_counter()
                        tcp_ms = (connected - started) * 1000
                        if protocol == "https":
                            connection = context.wrap_socket(connection, server_hostname=args.hostname)
                            tls_ms = (time.perf_counter() - connected) * 1000
                    sent = time.perf_counter()
                    policy = "close" if mode == "fresh" else "keep-alive"
                    connection.sendall(
                        f"GET {path} HTTP/1.1\r\nHost: {args.hostname}\r\n"
                        f"Connection: {policy}\r\n\r\n".encode("ascii")
                    )
                    response = http.client.HTTPResponse(connection)
                    response.begin()
                    headers_at = time.perf_counter()
                    body = response.read(LIMIT + 1)
                    if len(body) > LIMIT or not response.isclosed():
                        raise ValueError("Response exceeds bounded read limit or is incomplete")
                    finished = time.perf_counter()
                    row.update(
                        status=response.status, reused=reused, bytes=len(body),
                        connect_ms=round(tcp_ms, 2), tls_ms=round(tls_ms, 2),
                        response_headers_ms=round((headers_at - sent) * 1000, 2),
                        download_ms=round((finished - headers_at) * 1000, 2),
                        total_ms=round((finished - started) * 1000, 2),
                        connection=response.getheader("Connection"),
                        server_timing=response.getheader("Server-Timing"),
                    )
                    if path == "/api/v1/diag" and response.status == 200:
                        row["diagnostics"] = json.loads(body)
                    close = response.will_close or mode == "fresh"
                    response.close()
                    if close:
                        connection.close()
                        connection = None
                except (OSError, ValueError, http.client.HTTPException) as error:
                    row.update(error=str(error), total_ms=round((time.perf_counter() - started) * 1000, 2))
                    if connection is not None:
                        connection.close()
                        connection = None
                rows.append(row)
                print(json.dumps(row), flush=True)
    finally:
        if connection is not None:
            connection.close()
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", default="nanacoin.local")
    parser.add_argument("--hostname", default="nanacoin.local")
    parser.add_argument("--samples", type=int, default=3, help="Rounds per connection mode (1–20)")
    parser.add_argument("--timeout", type=float, default=15)
    parser.add_argument("--http", action="store_true", help="Also compare anonymous HTTP reads")
    parser.add_argument("--output", type=Path, help="Save complete measurements and summaries as JSON")
    args = parser.parse_args()
    if not 1 <= args.samples <= 20 or not 0 < args.timeout <= 60:
        parser.error("samples must be 1–20 and timeout must be >0 and <=60 seconds")
    context = ssl.create_default_context(cafile=str(ROOT / "certs/home-ca.crt"))
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    rows = []
    for protocol in (("https", "http") if args.http else ("https",)):
        for mode in ("fresh", "reuse"):
            rows.extend(measure(args, protocol, mode, context))
    summaries = []
    for protocol, mode, path in sorted({(r["protocol"], r["mode"], r["path"]) for r in rows}):
        group = [r for r in rows if (r["protocol"], r["mode"], r["path"]) == (protocol, mode, path)]
        good = [r for r in group if r.get("status") == 200 and "error" not in r]
        times = sorted(r["total_ms"] for r in good)
        summaries.append(dict(
            protocol=protocol, mode=mode, path=path, successful=len(good),
            failed=len(group) - len(good), reused=sum(r["reused"] for r in good),
            mean_ms=round(statistics.mean(times), 2) if times else None,
            median_ms=statistics.median(times) if times else None,
            p95_ms=times[math.ceil(len(times) * .95) - 1] if times else None,
        ))
    report = dict(address=args.address, measurements=rows, summaries=summaries)
    print(json.dumps({"summaries": summaries}, indent=2))
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if any(s["failed"] for s in summaries) else 0


if __name__ == "__main__":
    raise SystemExit(main())
