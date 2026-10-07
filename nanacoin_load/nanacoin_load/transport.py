"""Read-only transport benchmark for a live NanaCoin bank: HTTP/1.1 vs HTTP/2.

    uv run ncbench --label s3-before            # settings from .env
    uv run ncbench --label s3-after --modes h1 h2

Safe on a household's production bank:

- only GETs, plus one login per member at the start and its logout at the
  end (sessions are RAM-only; the ledger is never written);
- each member logs in exactly once and the token is reused, because the
  board holds 64 sessions for eight hours and refuses (never evicts) when
  full;
- concurrency stays within the board's connection slots.

Phases, for each transport in --modes:

  cold   one page view per member from nothing: the site's files (index,
         scripts, stylesheets) and the API reads the UI makes, on fresh
         connections with no TLS session to resume. HTTP/1.1 opens up to
         --page-connections connections (browsers: 6); HTTP/2 uses one.
  warm   --levels concurrent virtual members (cycling through the real
         ones) repeat the UI's API reads on open connections for
         --duration seconds each, pausing --think seconds between rounds.

HTTP/2 runs only if the bank offers it by ALPN (new firmware); the old
firmware is HTTP/1.1 only, which is today's baseline. Results go to
reports/transport-<label>-<time>.json and a summary table.
"""
import argparse
import base64
import gzip
import hashlib
import http.client
import json
import os
import re
import secrets
import socket
import ssl
import statistics
import threading
import time
from pathlib import Path

import h2.config
import h2.connection
import h2.events

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CA = ROOT.parent / "nanacoin_rs" / "certs" / "home-ca.crt"
API = [
    "/api/v1/status",
    "/api/v1/me",
    "/api/v1/users",
    "/api/v1/transactions?limit=20",
    "/api/v1/listings",
    "/api/v1/offers",
    "/api/v1/loans",
    "/api/v1/lottos",
    "/api/v1/things",
    "/api/v1/quotes",
    "/api/v1/commerce",
]
NANA_ONLY = ["/api/v1/admin/config", "/api/v1/admin/storage"]


def load_env():
    """KEY=value lines from .env, without overriding the real environment."""
    path = ROOT / ".env"
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            os.environ.setdefault(key.strip(), value.strip().strip("'\""))


class Target:
    def __init__(self, args):
        self.host = args.host
        self.address = args.address
        self.scheme = args.scheme
        self.port = 443 if args.scheme == "https" else 80
        self.ca = args.ca
        self.timeout = args.timeout

    def connect(self, alpn):
        """A fresh connection (no TLS session resumption). Returns
        (socket, setup seconds, protocol chosen)."""
        started = time.perf_counter()
        raw = socket.create_connection((self.address, self.port), timeout=self.timeout)
        raw.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        if self.scheme != "https":
            return raw, time.perf_counter() - started, None
        context = ssl.create_default_context(cafile=str(self.ca))
        context.set_alpn_protocols(alpn)
        sock = context.wrap_socket(raw, server_hostname=self.host)
        return sock, time.perf_counter() - started, sock.selected_alpn_protocol()


def h1_request(sock, host, method, path, headers=None, body=None):
    """One keep-alive request; returns (status, body bytes, seconds)."""
    started = time.perf_counter()
    lines = [f"{method} {path} HTTP/1.1", f"Host: {host}", "Accept-Encoding: gzip"]
    for name, value in (headers or {}).items():
        lines.append(f"{name}: {value}")
    if body is not None:
        lines += ["Content-Type: application/json", f"Content-Length: {len(body)}"]
    sock.sendall(("\r\n".join(lines) + "\r\n\r\n").encode() + (body or b""))
    response = http.client.HTTPResponse(sock)
    response.begin()
    data = response.read()
    if response.getheader("Content-Encoding") == "gzip":
        data = gzip.decompress(data)
    if response.will_close:
        raise ConnectionResetError("server closed the connection")
    return response.status, data, time.perf_counter() - started


def login(target, username, password):
    verifier = secrets.token_urlsafe(36)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
    redirect = "http://localhost/"
    sock, _, _ = target.connect(["http/1.1"])
    try:
        status, data, _ = h1_request(sock, target.host, "POST", "/api/v1/auth/authorize", body=json.dumps({
            "username": username, "password": password, "code_challenge": challenge,
            "code_challenge_method": "S256", "redirect_uri": redirect}).encode())
        if status != 200:
            raise SystemExit(f"login {username}: authorize HTTP {status}; stopping before any lockout")
        code = json.loads(data)["code"]
        status, data, _ = h1_request(sock, target.host, "POST", "/api/v1/auth/token", body=json.dumps({
            "code": code, "code_verifier": verifier, "redirect_uri": redirect}).encode())
        if status != 200:
            raise SystemExit(f"login {username}: token HTTP {status}")
        return json.loads(data)["access_token"]
    finally:
        sock.close()


def logout(target, token):
    try:
        sock, _, _ = target.connect(["http/1.1"])
        h1_request(sock, target.host, "POST", "/api/v1/auth/logout",
                   headers={"Authorization": "Bearer " + token}, body=b"{}")
        sock.close()
    except OSError:
        pass


def site_files(target):
    """The index and the scripts/stylesheets it references."""
    sock, _, _ = target.connect(["http/1.1"])
    try:
        status, data, _ = h1_request(sock, target.host, "GET", "/")
    finally:
        sock.close()
    if status != 200:
        return []
    found = re.findall(r'(?:src|href)="(/?[^"]+\.(?:js|css))"', data.decode("utf-8", "replace"))
    return ["/"] + ["/" + p.lstrip("/") for p in dict.fromkeys(found)]


class H2Client:
    """One HTTP/2 connection; a batch of requests is multiplexed."""

    def __init__(self, target):
        self.target = target
        self.sock, self.setup, chosen = target.connect(["h2", "http/1.1"])
        if target.scheme == "https" and chosen != "h2":
            self.sock.close()
            raise RuntimeError(f"the bank chose {chosen!r}, not h2")
        self.conn = h2.connection.H2Connection(h2.config.H2Configuration(client_side=True, header_encoding="utf-8"))
        self.conn.initiate_connection()
        self.sock.sendall(self.conn.data_to_send())
        # The server's SETTINGS (its stream limit) follow the handshake at
        # once; wait for them so the first batch respects the limit.
        while True:
            data = self.sock.recv(65536)
            if not data:
                raise ConnectionResetError("server closed the connection")
            events = self.conn.receive_data(data)
            outgoing = self.conn.data_to_send()
            if outgoing:
                self.sock.sendall(outgoing)
            if any(isinstance(e, h2.events.RemoteSettingsChanged) for e in events):
                break

    def batch(self, requests):
        """requests: [(path, headers)]. Returns [(path, status, bytes, seconds)].

        Opens as many streams as the server's SETTINGS_MAX_CONCURRENT_STREAMS
        allows and starts the rest as earlier ones finish, as browsers do.
        """
        streams = {}
        pending = list(requests)

        def start_more():
            while pending and self.conn.open_outbound_streams < self.conn.remote_settings.max_concurrent_streams:
                path, headers = pending.pop(0)
                stream = self.conn.get_next_available_stream_id()
                fields = [(":method", "GET"), (":scheme", self.target.scheme), (":authority", self.target.host),
                          (":path", path), ("accept-encoding", "gzip")]
                fields += [(k.lower(), v) for k, v in headers.items()]
                self.conn.send_headers(stream, fields, end_stream=True)
                streams[stream] = {"path": path, "status": 0, "body": b"", "sent": time.perf_counter(), "done": False}

        start_more()
        self.sock.sendall(self.conn.data_to_send())
        results = []
        while pending or not all(s["done"] for s in streams.values()):
            start_more()
            outgoing = self.conn.data_to_send()
            if outgoing:
                self.sock.sendall(outgoing)
            data = self.sock.recv(65536)
            if not data:
                raise ConnectionResetError("server closed the connection")
            for event in self.conn.receive_data(data):
                s = streams.get(getattr(event, "stream_id", None))
                if isinstance(event, h2.events.ResponseReceived):
                    s["status"] = int(dict(event.headers)[":status"])
                elif isinstance(event, h2.events.DataReceived):
                    s["body"] += event.data
                    self.conn.acknowledge_received_data(event.flow_controlled_length, event.stream_id)
                elif isinstance(event, h2.events.StreamEnded):
                    s["done"] = True
                    results.append((s["path"], s["status"], len(s["body"]), time.perf_counter() - s["sent"]))
                elif isinstance(event, h2.events.StreamReset):
                    s["done"] = True
                    results.append((s["path"], 0, 0, time.perf_counter() - s["sent"]))
                elif isinstance(event, h2.events.ConnectionTerminated):
                    raise ConnectionResetError(f"GOAWAY {event.error_code}")
            outgoing = self.conn.data_to_send()
            if outgoing:
                self.sock.sendall(outgoing)
        return results

    def close(self):
        try:
            self.conn.close_connection()
            self.sock.sendall(self.conn.data_to_send())
        except OSError:
            pass
        self.sock.close()


class H1Client:
    """Up to `width` keep-alive HTTP/1.1 connections, opened as needed; a
    batch is spread across them like a browser does."""

    def __init__(self, target, width):
        self.target = target
        self.width = width
        self.socks = []
        self.setups = []

    def _sock(self, i):
        while len(self.socks) <= i:
            sock, setup, _ = self.target.connect(["http/1.1"])
            self.socks.append(sock)
            self.setups.append(setup)
        return self.socks[i]

    def batch(self, requests):
        results, errors = [], []
        lanes = [requests[i::self.width] for i in range(min(self.width, len(requests)))]

        def run(i, lane):
            try:
                sock = self._sock_locked(i)
                for path, headers in lane:
                    status, data, seconds = h1_request(sock, self.target.host, "GET", path, headers)
                    results.append((path, status, len(data), seconds))
            except (OSError, http.client.HTTPException) as error:
                errors.append(f"{type(error).__name__}: {error}")
                self._drop(i)

        threads = [threading.Thread(target=run, args=(i, lane)) for i, lane in enumerate(lanes)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        if errors:
            raise ConnectionResetError("; ".join(errors[:3]))
        return results

    _lock = threading.Lock()

    def _sock_locked(self, i):
        with self._lock:
            return self._sock(i)

    def _drop(self, i):
        with self._lock:
            if i < len(self.socks):
                try:
                    self.socks[i].close()
                except OSError:
                    pass
                sock, setup, _ = self.target.connect(["http/1.1"])
                self.socks[i] = sock
                self.setups.append(setup)

    def close(self):
        for sock in self.socks:
            sock.close()


def board_health(target):
    """Public machine health (no login), on its own connection."""
    try:
        sock, _, _ = target.connect(["http/1.1"])
        status, data, _ = h1_request(sock, target.host, "GET", "/api/v1/diag")
        sock.close()
        d = json.loads(data) if status == 200 else {}
        internal = d.get("internal", {})
        return {k: d.get(k) for k in ("uptime_seconds", "free_heap", "minimum_free_heap", "largest_free_block",
                                       "psram_free", "requests", "errors")} | {
            "internal_min": internal.get("minimum")}
    except (OSError, ValueError) as error:
        return {"error": str(error)}


def percentile(values, p):
    if not values:
        return None
    values = sorted(values)
    return values[min(len(values) - 1, round(p / 100 * (len(values) - 1)))]


def summarize(samples):
    ms = [s[3] * 1000 for s in samples]
    return {
        "requests": len(samples),
        "errors": sum(1 for s in samples if s[1] == 0 or s[1] >= 500),
        "non_200": sum(1 for s in samples if s[1] != 200),
        "p50_ms": round(percentile(ms, 50), 1) if ms else None,
        "p95_ms": round(percentile(ms, 95), 1) if ms else None,
        "p99_ms": round(percentile(ms, 99), 1) if ms else None,
    }


def cold(target, mode, members, files, args):
    views = []
    for _ in range(args.repeats):
        for member in members:
            requests = [(p, {}) for p in files] + [(p, member["auth"]) for p in member["api"]]
            started = time.perf_counter()
            try:
                client = H2Client(target) if mode == "h2" else H1Client(target, args.page_connections)
                results = client.batch(requests)
                setups = [client.setup] if mode == "h2" else client.setups
                client.close()
                views.append({"member": member["name"], "ms": (time.perf_counter() - started) * 1000,
                              "connections": len(setups), "setup_ms": [round(s * 1000) for s in setups],
                              "bytes": sum(r[2] for r in results), "statuses": sorted({r[1] for r in results})})
            except (OSError, RuntimeError) as error:
                views.append({"member": member["name"], "error": f"{type(error).__name__}: {error}"})
            time.sleep(args.think)
    good = [v for v in views if "ms" in v]
    return {
        "views": views,
        "page_ms_median": round(statistics.median(v["ms"] for v in good)) if good else None,
        "page_ms_p95": round(percentile([v["ms"] for v in good], 95)) if good else None,
        "setup_ms_median": round(statistics.median(s for v in good for s in v["setup_ms"])) if good else None,
        "failed_views": len(views) - len(good),
    }


def warm(target, mode, members, level, args):
    samples, failures, lock = [], [], threading.Lock()
    deadline = time.perf_counter() + args.duration

    def vuser(i):
        member = members[i % len(members)]
        requests = [(p, member["auth"]) for p in member["api"]]
        client = None
        while time.perf_counter() < deadline:
            try:
                if client is None:
                    client = H2Client(target) if mode == "h2" else H1Client(target, args.warm_connections)
                results = client.batch(requests)
                with lock:
                    samples.extend(results)
            except (OSError, RuntimeError) as error:
                with lock:
                    failures.append(f"{type(error).__name__}: {error}")
                if client:
                    client.close()
                client = None
                time.sleep(1)
            time.sleep(args.think)
        if client:
            client.close()

    started = time.perf_counter()
    threads = [threading.Thread(target=vuser, args=(i,)) for i in range(level)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    seconds = time.perf_counter() - started
    return summarize(samples) | {
        "level": level,
        "requests_per_s": round(len(samples) / seconds, 1),
        "connection_failures": len(failures),
        "failure_examples": sorted(set(failures))[:3],
    }


def main():
    load_env()
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--label", required=True, help="names the report, e.g. s3-before")
    parser.add_argument("--host", default=os.getenv("NANACOIN_HOST", "nanacoin.local"))
    parser.add_argument("--address", default=os.getenv("NANACOIN_ADDRESS"))
    parser.add_argument("--scheme", default="https", choices=["https", "http"])
    parser.add_argument("--ca", type=Path, default=Path(os.getenv("NANACOIN_CA", DEFAULT_CA)))
    parser.add_argument("--modes", nargs="+", default=["h1", "h2"], choices=["h1", "h2"])
    parser.add_argument("--repeats", type=int, default=2, help="cold page views per member")
    parser.add_argument("--levels", type=int, nargs="+", default=[1, 3, 5])
    parser.add_argument("--duration", type=float, default=30.0, help="seconds per warm level")
    parser.add_argument("--think", type=float, default=0.5, help="seconds between a member's rounds")
    parser.add_argument("--page-connections", type=int, default=6)
    parser.add_argument("--warm-connections", type=int, default=2)
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args()
    args.address = args.address or args.host
    pairs = [p.split(":", 1) for p in os.getenv("NANACOIN_USERS", "").split(",") if ":" in p]
    if not pairs:
        raise SystemExit("set NANACOIN_USERS=name:pin,... (in .env)")
    target = Target(args)

    report = {"label": args.label, "started": time.strftime("%Y-%m-%dT%H:%M:%S"), "target": args.host,
              "address": args.address, "scheme": args.scheme, "settings": {
                  k: getattr(args, k) for k in ("repeats", "levels", "duration", "think",
                                                "page_connections", "warm_connections")}}
    report["health_before"] = board_health(target)
    files = site_files(target)
    report["site_files"] = files
    members, tokens = [], []
    try:
        for name, password in pairs:
            token = login(target, name, password)
            tokens.append(token)
            auth = {"Authorization": "Bearer " + token}
            api = API + (NANA_ONLY if name == "nana" else [])
            # Keep only reads this firmware has (a 404 is not a transport result).
            sock, _, _ = target.connect(["http/1.1"])
            kept = []
            for path in api:
                status, _, _ = h1_request(sock, target.host, "GET", path, auth)
                if status != 404:
                    kept.append(path)
            sock.close()
            members.append({"name": name, "auth": auth, "api": kept})
        report["reads_per_member"] = {m["name"]: len(m["api"]) for m in members}
        print(f"{len(members)} members, {len(files)} site files, reads: {report['reads_per_member']}")

        report["modes"] = {}
        for mode in args.modes:
            if mode == "h2":
                try:
                    H2Client(target).close()
                except RuntimeError as error:
                    report["modes"]["h2"] = {"skipped": str(error)}
                    print(f"h2: skipped ({error})")
                    continue
            result = {"cold": cold(target, mode, members, files, args)}
            c = result["cold"]
            print(f"{mode} cold page view: median {c['page_ms_median']} ms, p95 {c['page_ms_p95']} ms, "
                  f"setup median {c['setup_ms_median']} ms, failed {c['failed_views']}")
            result["warm"] = []
            for level in args.levels:
                w = warm(target, mode, members, level, args)
                result["warm"].append(w)
                print(f"{mode} warm x{level}: {w['requests_per_s']} req/s, p50 {w['p50_ms']} ms, "
                      f"p95 {w['p95_ms']} ms, p99 {w['p99_ms']} ms, errors {w['errors']}, "
                      f"connection failures {w['connection_failures']}")
            result["health_after"] = board_health(target)
            report["modes"][mode] = result
    finally:
        for token in tokens:
            logout(target, token)
        report["logged_out"] = len(tokens)
        report["health_end"] = board_health(target)
        out = ROOT / "reports" / f"transport-{args.label}-{time.strftime('%Y%m%d-%H%M%S')}.json"
        out.parent.mkdir(exist_ok=True)
        out.write_text(json.dumps(report, indent=2))
        print(f"health before {report['health_before']}")
        print(f"health end    {report['health_end']}")
        print(f"report: {out}")


if __name__ == "__main__":
    main()
