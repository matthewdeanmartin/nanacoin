#!/usr/bin/env python3
"""Evidence-first, bounded NanaCoin experiments. See EXPERIMENTS.md."""

import argparse
import asyncio
import base64
from collections import Counter
from contextlib import suppress
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import re
import time
import uuid

ORIGIN = "http://localhost:4200"
MAX_BODY = 128 * 1024
SCENARIOS = ("status", "me", "ledger", "listings", "preflight", "concurrency",
             "missing-fixed", "missing-unique", "issue-replay", "issue-new",
             "listing-churn", "auth-churn")
MUTATING = {"issue-replay", "issue-new", "listing-churn", "auth-churn",
            "missing-fixed", "missing-unique"}  # missing IDs currently intern!


def health(text):
    return {k: int(v) for k, v in re.findall(
        r"\b(free|inuse|obj|blk|gc|mallocs|frees)\s+(-?\d+)", text or "")}


class Evidence:
    def __init__(self, directory):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=False)
        self.file = (self.directory / "events.jsonl").open("w", encoding="utf-8")
        self.started = time.monotonic()

    def emit(self, kind, **fields):
        event = dict(kind=kind, utc=datetime.now(timezone.utc).isoformat(),
                     seconds=round(time.monotonic() - self.started, 6), **fields)
        self.file.write(json.dumps(event, ensure_ascii=True) + "\n")
        self.file.flush()  # last evidence survives Ctrl-C / board failure


class Wire:
    """No executor backlog: deadline cancels the actual socket operation.

    One connection per request, matching the board. Headers are persisted
    before reading the body, so a stalled/truncated body cannot hide health.
    Supports length, chunked and connection-close HTTP response framing.
    """
    def __init__(self, host, port, evidence, timeout=10):
        self.host, self.port, self.evidence = host, port, evidence
        self.timeout, self.sequence = timeout, 0

    async def request(self, method, path, *, body=None, token=None, headers=None,
                      phase="setup", role="load", expected=(200,), timeout=None):
        self.sequence += 1
        rid = self.sequence
        started = time.monotonic()
        result = dict(id=rid, phase=phase, role=role, method=method, path=path,
                      status=0, stage="connect", bytes=0, health={}, error=None)
        self.evidence.emit("request_start", **result)
        payload = b""
        writer = None

        async def exchange():
            nonlocal writer, payload
            reader, writer = await asyncio.open_connection(self.host, self.port)
            result["connect_ms"] = round((time.monotonic() - started) * 1000, 3)
            data = b"" if body is None else json.dumps(body).encode()
            hdr = {"Host": self.host, "Connection": "close", "Origin": ORIGIN}
            if token:
                hdr["Authorization"] = "Bearer " + token
            if body is not None:
                hdr.update({"Content-Type": "application/json", "Content-Length": str(len(data))})
            hdr.update(headers or {})
            request = f"{method} {path} HTTP/1.1\r\n" + "".join(
                f"{k}: {v}\r\n" for k, v in hdr.items()) + "\r\n"
            result["stage"] = "headers"
            writer.write(request.encode("ascii") + data)
            await writer.drain()
            raw = await reader.readuntil(b"\r\n\r\n")
            lines = raw.decode("iso-8859-1").split("\r\n")
            result["status"] = int(lines[0].split()[1])
            received = {}
            for line in lines[1:]:
                if ":" in line:
                    k, v = line.split(":", 1)
                    received[k.lower()] = v.strip()
            result["header_ms"] = round((time.monotonic() - started) * 1000, 3)
            result["health"] = health(received.get("x-nanacoin-health"))
            result["cors"] = received.get("access-control-allow-origin")
            result["stage"] = "body"
            self.evidence.emit("response_headers", **result)
            if method == "HEAD" or result["status"] in (204, 304):
                return
            if "chunked" in received.get("transfer-encoding", "").lower():
                while True:
                    size = int((await reader.readline()).split(b";", 1)[0], 16)
                    if not size:
                        while (await reader.readline()) not in (b"\r\n", b""):
                            pass
                        break
                    if size + len(payload) > MAX_BODY:
                        raise ValueError("response exceeds capture limit")
                    payload += await reader.readexactly(size)
                    result["bytes"] = len(payload)
                    if await reader.readexactly(2) != b"\r\n":
                        raise ValueError("bad chunk terminator")
            else:
                length = received.get("content-length")
                remaining = int(length) if length is not None else None
                if remaining is not None and (remaining < 0 or remaining > MAX_BODY):
                    raise ValueError("invalid/excessive response length")
                while remaining is None or remaining > 0:
                    chunk = await reader.read(min(4096, remaining) if remaining is not None else 4096)
                    if not chunk:
                        if remaining:
                            raise ValueError("truncated response")
                        break
                    payload += chunk
                    result["bytes"] = len(payload)
                    if len(payload) > MAX_BODY:
                        raise ValueError("response exceeds capture limit")
                    if remaining is not None:
                        remaining -= len(chunk)

        try:
            await asyncio.wait_for(exchange(), timeout or self.timeout)
            result["stage"] = "complete"
        except asyncio.CancelledError:
            result["error"] = "cancelled"
            raise
        except Exception as exc:
            result["error"] = type(exc).__name__ + ": " + str(exc)
        finally:
            if writer is not None:
                writer.close()
            result["elapsed_ms"] = round((time.monotonic() - started) * 1000, 3)
            result["bytes"] = len(payload)
            result["expected"] = list(expected)
            result["ok"] = result["error"] is None and result["status"] in expected
            if payload and result["error"] is None:
                try:
                    result["json"] = json.loads(payload)
                except (ValueError, UnicodeError):
                    result["error"] = "invalid JSON response"
                    result["ok"] = False
            elif not payload and result["error"] is None and method != "HEAD" and result["status"] not in (204, 304):
                result["error"] = "empty JSON response"
                result["ok"] = False
            # Never persist auth responses, tokens, passwords or arbitrary bodies.
            safe = {k: v for k, v in result.items() if k != "json"}
            if isinstance(result.get("json"), dict):
                safe["error_code"] = result["json"].get("error")
                if path == "/api/v1/admin/issue":
                    safe["transaction_id"] = result["json"].get("id")
            self.evidence.emit("request_end", **safe)
        return result


class Experiment:
    def __init__(self, args, evidence):
        self.args, self.evidence = args, evidence
        self.wire = Wire(args.ip, args.port, evidence, args.timeout)
        self.token = None
        self.account = None
        self.phase = "setup"
        self.stop = asyncio.Event()
        self.anomaly = None
        self.rows = []
        self.phases = []
        self.last_uptime = None
        self.tag = uuid.uuid4().hex[:12]

    def flag(self, reason, **details):
        if self.anomaly is None:
            self.anomaly = dict(reason=reason, phase=self.phase, **details)
            self.evidence.emit("anomaly", **self.anomaly)
            print(f"ANOMALY {self.phase}: {reason}", flush=True)
        self.stop.set()

    async def call(self, method, path, *, role="load", **kw):
        r = await self.wire.request(method, path, token=self.token,
                                    phase=self.phase, role=role, **kw)
        if role == "load":
            self.rows.append({k: v for k, v in r.items() if k != "json"})
        if not r["ok"]:
            self.flag("request_failed", request=r["id"], stage=r["stage"],
                      status=r["status"], error=r["error"])
        free = r["health"].get("free")
        if free is not None and free < self.args.min_free:
            self.flag("low_headroom", request=r["id"], free=free)
        return r

    async def snapshot(self, label, logs=False):
        snap = {}
        for name in (["diag", "status", "logs?limit=64"] if logs else ["diag", "status"]):
            r = await self.wire.request("GET", "/api/v1/" + name,
                                        phase=self.phase, role="telemetry")
            data = r.get("json")
            snap[name] = data if r["ok"] else {"unavailable": True, "request": r["id"]}
            if not r["ok"] and name in ("diag", "status"):
                self.flag("snapshot_unavailable", endpoint=name, request=r["id"])
            if isinstance(data, dict) and name == "diag":
                uptime = data.get("uptime_seconds")
                if uptime is not None:
                    if self.last_uptime is not None and uptime < self.last_uptime:
                        self.flag("uptime_decreased", before=self.last_uptime, after=uptime)
                    self.last_uptime = uptime
            if isinstance(data, dict) and name == "status" and data.get("ledger_balanced") is False:
                self.flag("ledger_unbalanced")
        self.evidence.emit("snapshot", phase=self.phase, label=label, data=snap)
        return snap

    async def monitor(self, finished):
        while not self.stop.is_set() and not finished.is_set():
            try:
                await asyncio.wait_for(finished.wait(), self.args.monitor_interval)
                break
            except asyncio.TimeoutError:
                pass
            if self.stop.is_set() or finished.is_set():
                break
            r = await self.call("GET", "/api/v1/diag", role="monitor")
            data = r.get("json")
            if isinstance(data, dict):
                self.evidence.emit("diagnostic_sample", phase=self.phase, data=data)
                uptime = data.get("uptime_seconds")
                if uptime is not None:
                    if self.last_uptime is not None and uptime < self.last_uptime:
                        self.flag("uptime_decreased", before=self.last_uptime, after=uptime)
                    self.last_uptime = uptime

    async def login(self, role="setup"):
        verifier = "boardprobe-" + "x" * 48
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
        r = await self.call("POST", "/api/v1/auth/authorize", role=role, timeout=45,
                            body=dict(username=self.args.username, password=self.args.password,
                                      code_challenge=challenge, code_challenge_method="S256",
                                      redirect_uri=ORIGIN + "/cb"))
        code = r.get("json", {}).get("code")
        if not r["ok"] or not code:
            raise RuntimeError("authorization failed; see request trace (check credentials)")
        r = await self.call("POST", "/api/v1/auth/token", role=role, timeout=45,
                            body=dict(code=code, code_verifier=verifier, redirect_uri=ORIGIN + "/cb"))
        self.token = r.get("json", {}).get("access_token")
        self.account = r.get("json", {}).get("user", {}).get("account")
        if not r["ok"] or not self.token or not self.account:
            raise RuntimeError("token exchange failed")

    async def operation(self, scenario, n):
        self.evidence.emit("operation_start", phase=self.phase, scenario=scenario, index=n)
        if scenario in ("status", "me", "listings", "concurrency", "ledger"):
            path = {"concurrency": "status", "ledger": f"transactions?limit={self.args.page_size}"}.get(scenario, scenario)
            r = await self.call("GET", "/api/v1/" + path)
            key = {"status": "transactions", "concurrency": "transactions",
                   "ledger": "transactions", "listings": "listings", "me": "id"}[scenario]
            if r["ok"] and (not isinstance(r.get("json"), dict) or key not in r["json"]):
                self.flag("response_shape_mismatch", request=r["id"], missing=key)
        elif scenario == "preflight":
            r = await self.call("OPTIONS", "/api/v1/me", expected=(204,), headers={
                "Access-Control-Request-Method": "GET",
                "Access-Control-Request-Headers": "authorization"})
            if r.get("cors") not in ("*", ORIGIN):
                self.flag("preflight_missing_cors", request=r["id"])
            if not self.stop.is_set():
                await self.call("GET", "/api/v1/me")
        elif scenario.startswith("missing-"):
            suffix = "fixed" if scenario == "missing-fixed" else str(n)
            await self.call("GET", f"/api/v1/accounts/probe-{self.tag}-{suffix}", expected=(404,))
        elif scenario.startswith("issue-"):
            suffix = "replay" if scenario == "issue-replay" else str(n)
            await self.call("POST", "/api/v1/admin/issue", expected=(201,), timeout=45,
                            headers={"Idempotency-Key": f"probe-{self.tag}-{suffix}"},
                            body={"to": self.account, "amount": 1, "reason": "bottleneck probe"})
        elif scenario == "listing-churn":
            description = "x" * self.args.description_bytes
            r = await self.call("POST", "/api/v1/listings", expected=(201,), timeout=45,
                                body={"title": f"probe {self.tag} {n}", "description": description, "price": 1})
            data = r.get("json", {})
            if r["ok"] and (not data.get("id") or data.get("description") != description):
                self.flag("listing_roundtrip_mismatch", request=r["id"],
                          expected_description_bytes=len(description),
                          actual_description_bytes=len(data.get("description", "")))
            if r["ok"] and data.get("id") and not self.stop.is_set():
                await self.call("POST", f"/api/v1/listings/{data['id']}/cancel")
        elif scenario == "auth-churn":
            await self.call("POST", "/api/v1/auth/logout", expected=(204,))
            self.token = None
            if not self.stop.is_set():
                await self.login(role="load")

    async def run_phase(self, scenario, concurrency):
        self.phase = f"{scenario}/c{concurrency}"
        print(f"START {self.phase}: {self.args.operations} operations", flush=True)
        before = await self.snapshot("before")
        start_index = len(self.rows)
        next_n = 0

        async def worker():
            nonlocal next_n
            while next_n < self.args.operations and not self.stop.is_set():
                n = next_n
                next_n += 1
                await self.operation(scenario, n)
                if self.args.pace:
                    await asyncio.sleep(self.args.pace)

        finished = asyncio.Event()
        monitor = asyncio.create_task(self.monitor(finished)) if self.args.monitor_interval else None
        workers = [asyncio.create_task(worker()) for _ in range(concurrency)]
        try:
            await asyncio.gather(*workers)
            finished.set()
            if monitor:
                # Drain an already-started diagnostic instead of disconnecting
                # it and manufacturing a failed response-write counter.
                await monitor
        finally:
            for task in workers + ([monitor] if monitor else []):
                task.cancel()
            await asyncio.gather(*workers, *([monitor] if monitor else []), return_exceptions=True)
        self.evidence.emit("load_stopped", phase=self.phase, operations_started=next_n)
        # Both ends of every comparison follow the same quiet interval.
        await asyncio.sleep(self.args.settle)
        after = await self.snapshot("after_quiet", logs=bool(self.anomaly))
        rows = self.rows[start_index:]
        summary = summarize(rows)
        summary.update(phase=self.phase, operations_started=next_n,
                       before=before, after=after)
        summary["quiet_deltas"] = {
            key: after["diag"][key] - before["diag"][key]
            for key in ("free_now", "objects_now", "gcs_now", "alloc_failures")
            if isinstance(before.get("diag", {}).get(key), (int, float))
            and isinstance(after.get("diag", {}).get(key), (int, float))}
        if not self.anomaly and scenario.startswith("issue-"):
            a, b = before.get("status", {}), after.get("status", {})
            if "transactions" in a and "transactions" in b:
                expected = 1 if scenario == "issue-replay" else self.args.operations
                delta = b["transactions"] - a["transactions"]
                if delta != expected:
                    self.flag("transaction_count_mismatch", expected=expected, actual=delta)
        self.phases.append(summary)
        self.evidence.emit("phase_summary", **summary)
        print(f"END {self.phase}: {summary['requests']} requests, "
              f"p95={summary['p95_ms']}ms, min_free={summary['min_free']}", flush=True)

    async def recovery(self):
        self.phase = "recovery"
        self.evidence.emit("recovery_start", quiet_seconds=self.args.settle)
        await asyncio.sleep(self.args.settle)
        replies = []
        for _ in range(3):
            r = await self.wire.request("GET", "/api/v1/status", phase=self.phase, role="recovery")
            replies.append(r["ok"])
            await asyncio.sleep(2)
        await self.snapshot("recovery", logs=True)
        verdict = "responding_after_load_removed" if all(replies) else "intermittent_or_unreachable"
        self.evidence.emit("recovery_end", replies=replies, verdict=verdict)
        return verdict


def summarize(rows):
    durations = sorted(r["elapsed_ms"] for r in rows)
    free = [r["health"]["free"] for r in rows if "free" in r["health"]]
    return dict(requests=len(rows), statuses=dict(Counter(str(r["status"]) for r in rows)),
                failures=sum(not r["ok"] for r in rows),
                p95_ms=durations[max(0, math.ceil(len(durations) * .95) - 1)] if durations else None,
                min_free=min(free) if free else None,
                health_samples=len(free))


async def serial_capture(port, evidence, experiment):
    """Nonblocking serial reads; reconnect after USB re-enumeration, never reset."""
    try:
        import serial
        from serial.tools import list_ports
    except ImportError:
        evidence.emit("serial_unavailable", reason="install pyserial")
        return
    sp = None
    buf = b""
    try:
        while True:
            if sp is None:
                device = port
                if port == "auto":
                    device = next((p.device for p in list_ports.comports() if p.vid == 0x303A), None)
                try:
                    if not device:
                        raise OSError("native USB not enumerated")
                    sp = serial.Serial(device, 115200, timeout=0, dsrdtr=True)
                    evidence.emit("serial_connected", port=device)
                except (OSError, serial.SerialException) as exc:
                    evidence.emit("serial_unavailable", reason=str(exc))
                    await asyncio.sleep(2)
                    continue
            try:
                chunk = sp.read(4096)
            except (OSError, serial.SerialException) as exc:
                evidence.emit("serial_disconnected", reason=str(exc), partial=buf.decode(errors="replace"))
                sp.close()
                sp, buf = None, b""
                continue
            buf += chunk
            while b"\n" in buf:
                raw, buf = buf.split(b"\n", 1)
                line = raw.decode("utf-8", "replace").rstrip("\r")
                evidence.emit("serial", line=line, phase=experiment.phase)
                if any(marker in line.lower() for marker in ("fatal error:", "panic:", "out of memory")):
                    experiment.flag("serial_fatal", line=line)
                elif any(marker in line for marker in ("NanaCoin starting", "ESP-ROM:", "rst:0x")):
                    experiment.flag("boot_banner_observed", line=line)
            if len(buf) > 16384:
                evidence.emit("serial_partial", line=buf.decode(errors="replace"))
                buf = b""
            await asyncio.sleep(.05)
    finally:
        if buf:
            evidence.emit("serial_partial", line=buf.decode(errors="replace"))
        if sp:
            sp.close()


async def run(args):
    evidence = Evidence(args.out)
    exp = Experiment(args, evidence)
    serial_task = asyncio.create_task(serial_capture(args.serial, evidence, exp)) if args.serial else None
    recovery = None
    error = None
    # Explicit whitelist: argparse also holds the password.
    evidence.emit("manifest", tool_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), settings={k: getattr(args, k) for k in (
        "ip", "port", "scenarios", "operations", "levels", "page_size", "description_bytes",
        "monitor_interval", "settle", "timeout", "min_free", "seconds", "pace", "firmware")})
    print(f"Evidence: {evidence.directory.resolve()}", flush=True)
    try:
        async with asyncio.timeout(args.seconds):
            baseline = await exp.snapshot("initial", logs=True)
            if baseline["status"].get("unavailable"):
                raise RuntimeError("board unavailable before workload; no stress sent")
            if exp.stop.is_set():
                raise RuntimeError("initial diagnostics failed; no setup or stress sent")
            if args.provision:
                await exp.call("POST", "/api/v1/provision", role="setup", timeout=45,
                               expected=(201, 403), body={"username": args.username,
                               "password": args.password, "display_name": "Nana",
                               "household_name": "Bottleneck test"})
            if any(s not in ("status", "concurrency") for s in args.scenarios):
                await exp.login()
            await asyncio.sleep(args.settle)
            for scenario in args.scenarios:
                for level in (args.levels if scenario == "concurrency" else [1]):
                    if exp.stop.is_set():
                        break
                    await exp.run_phase(scenario, level)
            if exp.anomaly:
                recovery = await exp.recovery()
    except (Exception, asyncio.CancelledError) as exc:
        error = type(exc).__name__ + ": " + str(exc)
        evidence.emit("run_interrupted", error=error)
        # Bounded extra evidence window, including on Ctrl-C / wall budget.
        try:
            async with asyncio.timeout(3 * args.timeout + args.settle + 10):
                recovery = await exp.recovery()
        except (Exception, asyncio.CancelledError):
            recovery = "recovery_capture_incomplete"
    finally:
        if serial_task:
            serial_task.cancel()
            with suppress(asyncio.CancelledError):
                await serial_task
        summary = dict(anomaly=exp.anomaly, error=error, recovery=recovery, phases=exp.phases,
                       interpretation="No-response is not proof of OOM. blk/frag_now is not largest free block; alloc_failures counts failed response writes.")
        (evidence.directory / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
        evidence.emit("run_end", anomaly=exp.anomaly, error=error, recovery=recovery)
        evidence.file.close()
        from evidence_report import report
        report(evidence.directory)
    return 2 if exp.anomaly or error else 0


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--ip", default="192.168.1.158")
    p.add_argument("--port", type=int, default=80)
    p.add_argument("--scenarios", nargs="+", choices=SCENARIOS, default=["status"])
    p.add_argument("--operations", type=int, default=500)
    p.add_argument("--levels", nargs="+", type=int, default=[1, 2, 4, 8])
    p.add_argument("--page-size", type=int, default=30)
    p.add_argument("--description-bytes", type=int, default=500)
    p.add_argument("--monitor-interval", type=float, default=5, help="0 disables active polling; passive headers still recorded")
    p.add_argument("--settle", type=float, default=12, help="quiet seconds before snapshots")
    p.add_argument("--timeout", type=float, default=10)
    p.add_argument("--seconds", type=float, default=600, help="run budget; bounded recovery may extend it")
    p.add_argument("--pace", type=float, default=0)
    p.add_argument("--min-free", type=int, default=8192, help="stop before exhaustion; 0 disables")
    p.add_argument("--allow-writes", action="store_true")
    p.add_argument("--provision", action="store_true")
    p.add_argument("--username", default="nana")
    p.add_argument("--password", default="nana-pin", help="test credential; never saved in trace")
    p.add_argument("--serial", help="native USB port or 'auto'; optional pyserial")
    p.add_argument("--firmware", default="unknown", help="flashed binary hash/build label, not assumed from checkout")
    p.add_argument("--out", default=str(Path(__file__).parent / "runs" / datetime.now().strftime("%Y%m%d-%H%M%S-%f")))
    return p


def main():
    p = parser()
    args = p.parse_args()
    if (MUTATING.intersection(args.scenarios) or args.provision) and not args.allow_writes:
        p.error("these scenarios change board state; use --allow-writes on a disposable household")
    if args.operations < 1 or any(n < 1 or n > 32 for n in args.levels):
        p.error("operations must be positive; concurrency must be 1..32")
    if not all(math.isfinite(v) for v in (args.timeout, args.seconds, args.monitor_interval, args.settle, args.pace)) or args.timeout <= 0 or args.seconds <= 0 or min(args.monitor_interval, args.settle, args.pace, args.min_free) < 0:
        p.error("invalid time/threshold bounds")
    if not 0 <= args.description_bytes <= 500 or args.page_size < 1:
        p.error("description bytes must be 0..500; page size must be positive")
    return asyncio.run(run(args))


if __name__ == "__main__":
    raise SystemExit(main())
