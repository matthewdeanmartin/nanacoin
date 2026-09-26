"""Read-only regression checks for the board's persistent, isolated TLS transport."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import gzip
import json
from pathlib import Path
import re
import socket
import ssl
import statistics
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


class Connection:
    def __init__(self, address, context, session=None):
        raw = socket.create_connection((address, 443), timeout=8)
        raw.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        try:
            self.sock = context.wrap_socket(raw, server_hostname="nanacoin.local", session=session)
        except BaseException:
            raw.close()
            raise
        self.file = self.sock.makefile("rb")

    def close(self):
        self.file.close()
        self.sock.close()

    def read(self):
        status_line = self.file.readline(4096).decode("ascii")
        assert status_line.startswith("HTTP/1.1 "), status_line
        status = int(status_line.split()[1])
        headers = {}
        for _ in range(32):
            line = self.file.readline(4096)
            if line == b"\r\n":
                break
            assert line and line.endswith(b"\r\n")
            name, value = line.decode("ascii").split(":", 1)
            headers[name.lower()] = value.strip()
        else:
            raise AssertionError("Too many response headers")
        assert "transfer-encoding" not in headers
        size = int(headers.get("content-length", "0"))
        assert 0 <= size <= 1024 * 1024
        body = self.file.read(size)
        assert len(body) == size
        return status, headers, body

    def get(self, path="/api/v1/status", extra=""):
        self.sock.sendall(f"GET {path} HTTP/1.1\r\nHost: nanacoin.local\r\n{extra}\r\n".encode("ascii"))
        status, headers, body = self.read()
        assert status == 200, status
        assert headers.get("connection", "").lower() != "close"
        return headers, body


def summary(samples):
    return dict(samples=len(samples), mean_ms=statistics.mean(samples),
                median_ms=statistics.median(samples), max_ms=max(samples))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    context = ssl.create_default_context(cafile=str(ROOT / "certs/home-ca.crt"))
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    # Explicit TLS 1.2 to exercise IDF's RFC5077 tickets.
    context.maximum_version = ssl.TLSVersion.TLSv1_2
    report = dict(address=args.address, tests={})
    hot = Connection(args.address, context)
    try:
        _, data = hot.get("/api/v1/diag")
        report["before"] = json.loads(data)
        timings = []
        for _ in range(20):
            start = time.perf_counter()
            headers, data = hot.get()
            assert json.loads(data)["ledger_balanced"] is True
            assert "server-timing" in headers
            timings.append((time.perf_counter() - start) * 1000)
        report["tests"]["persistent"] = summary(timings)
        session = hot.sock.session
        start = time.perf_counter()
        resumed = Connection(args.address, context, session)
        try:
            assert resumed.sock.session_reused, "Server did not resume TLS session"
            resumed.get()
            report["tests"]["resumed_tls"] = dict(total_ms=(time.perf_counter()-start)*1000)
        finally:
            resumed.close()

        # One silent handshake plus a real new client must not stall hot reads.
        stalled = socket.create_connection((args.address, 443), timeout=8)
        stalled_at = time.perf_counter()
        def cold_request():
            start = time.perf_counter()
            cold = Connection(args.address, context)
            try:
                cold.get()
                return (time.perf_counter()-start)*1000
            finally:
                cold.close()
        try:
            with ThreadPoolExecutor(max_workers=1) as pool:
                future = pool.submit(cold_request)
                timings = []
                for _ in range(50):
                    start = time.perf_counter()
                    hot.get()
                    timings.append((time.perf_counter()-start)*1000)
                    time.sleep(.02)
                cold_ms = future.result(timeout=8)
            assert stalled.recv(1) == b"", "Silent handshake did not expire"
            report["tests"]["hot_during_handshakes"] = dict(**summary(timings), cold_ms=cold_ms,
                silent_connection_closed_by_ms=(time.perf_counter()-stalled_at)*1000)
        finally:
            stalled.close()

        slow = Connection(args.address, context)
        try:
            slow.sock.sendall(b"GET /api/v1/status HTTP/1.1\r\nHost: ")
            timings = []
            for _ in range(10):
                start = time.perf_counter()
                hot.get()
                timings.append((time.perf_counter()-start)*1000)
            report["tests"]["hot_during_partial_request"] = summary(timings)
        finally:
            slow.close()

        # GET bodies must be consumed before the following pipelined request.
        first = b"GET /api/v1/status HTTP/1.1\r\nHost: nanacoin.local\r\nContent-Length: 2\r\n\r\n{}"
        second = b"GET /api/v1/diag HTTP/1.1\r\nHost: nanacoin.local\r\n\r\n"
        hot.sock.sendall(first + second)
        assert hot.read()[0] == 200
        assert hot.read()[0] == 200
        report["tests"]["pipelined_framing"] = "passed"

        bad = Connection(args.address, context)
        try:
            bad.sock.sendall(b"GET /api/v1/status HTTP/1.1\r\nHost: nanacoin.local\r\nContent-Length: 0\r\nContent-Length: 1\r\n\r\n")
            status, headers, _ = bad.read()
            assert status == 400 and headers["connection"] == "close"
            assert bad.file.read(1) == b""
            report["tests"]["ambiguous_framing_rejected"] = "passed"
        finally:
            bad.close()

        _, index = hot.get("/")
        asset = re.search(rb'src="([A-Za-z0-9_-]+\.js)"', index).group(1).decode()
        headers, body = hot.get("/" + asset, "Accept-Encoding: gzip\r\n")
        assert headers["content-encoding"] == "gzip"
        local = ROOT.parent / "nanacoin_ui/dist/nanacoin-web/browser" / asset
        assert gzip.decompress(body) == local.read_bytes()
        report["tests"]["large_gzip_asset_matches_build"] = dict(bytes=len(body))

        # Establish all eight sessions before timing concurrent API work, so
        # handshake cost cannot be mistaken for established-request latency.
        peers = [hot]
        try:
            for _ in range(7):
                peers.append(Connection(args.address, context))
                peers[-1].get()
            barrier = threading.Barrier(len(peers), timeout=10)
            def established_reads(connection):
                barrier.wait()
                times = []
                for _ in range(25):
                    start = time.perf_counter()
                    connection.get()
                    times.append((time.perf_counter() - start) * 1000)
                return times
            with ThreadPoolExecutor(max_workers=len(peers)) as pool:
                timings = [ms for batch in pool.map(established_reads, peers) for ms in batch]
            report["tests"]["eight_established_clients"] = summary(timings)
        finally:
            for peer in peers[1:]:
                peer.close()
        _, data = hot.get("/api/v1/diag")
        report["after"] = json.loads(data)
        assert report["after"]["uptime_seconds"] >= report["before"]["uptime_seconds"]
        assert report["after"]["samples"] > report["before"]["samples"]
    finally:
        hot.close()
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
