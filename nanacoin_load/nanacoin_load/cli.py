import argparse
import http.server
import os
import subprocess
import sys
import threading
import time
import webbrowser
from functools import partial

import requests

from . import report
from .client import API
from .common import MONITOR_PATH, REPORTS, ROOT, SCENARIOS, VERIFY, Evidence, load_fixture, new_run, write_json
from .journeys import e2e, prepare


def serial_capture(port, directory, done):
    import serial

    # Optional independent file avoids cross-process JSONL writes. Never toggles reset pins.
    with (directory / "serial.log").open("w", encoding="utf-8", buffering=1) as log:
        try:
            with serial.Serial(port, 115200, timeout=0.3) as device:
                while not done.is_set():
                    line = device.readline().decode("utf-8", "replace")
                    if line:
                        log.write(f"{time.time():.3f} {line}")
        except (serial.SerialException, OSError) as exc:
            log.write(f"Capture unavailable: {type(exc).__name__}\n")


def snapshot(host, evidence, label):
    # Fresh session avoids ambient proxy configuration and retains no auth material.
    session = requests.Session()
    session.trust_env = False
    session.verify = VERIFY
    try:
        r = session.get(host + MONITOR_PATH, timeout=(3, 5))
        r.raise_for_status()
        data = r.json()
        evidence.emit("snapshot", role=label, data=data)
        return True
    except (requests.RequestException, ValueError) as exc:
        evidence.emit(
            "recovery" if label == "after" else "monitor_failure", available=False, error=type(exc).__name__
        )
        return False
    finally:
        session.close()


def run_load(args, ui=False):
    directory = new_run(args.scenario)
    write_json(
        directory / "meta.json",
        {
            "scenario": args.scenario,
            "host": args.host,
            "users": args.users,
            "steps": args.steps,
            "stage": args.stage,
            "duration": args.duration,
            "python": sys.version,
            "wait_min": args.wait_min,
            "wait_max": args.wait_max,
            "monitor_interval": args.monitor_interval,
        },
    )
    trace = Evidence(directory)
    if not snapshot(args.host, trace, "before"):
        trace.emit("anomaly", reason="Board unavailable before workload; no load was started")
        trace.close()
        report.build(directory)
        print(f"Board unavailable. Report: {directory / 'index.html'}", flush=True)
        return 2
    if args.scenario != "status":
        try:
            fixture = load_fixture(args.host)
            API(args.host, trace).call("GET", "/api/v1/me", token=fixture["nana"]["token"])
        except OSError, ValueError, KeyError, RuntimeError:
            trace.emit(
                "anomaly", reason="Fixture unavailable or stale; run make prepare before this workload"
            )
            trace.close()
            report.build(directory)
            return 2
    trace.close()
    env = os.environ.copy()
    env.update(
        NANA_RUN=str(directory),
        NANA_SCENARIO=args.scenario,
        NANA_USERS=str(args.users),
        NANA_DURATION=str(args.duration),
        NANA_STEPS=args.steps,
        NANA_STAGE=str(args.stage),
        NANA_WAIT_MIN=str(args.wait_min),
        NANA_WAIT_MAX=str(args.wait_max),
        NANA_MONITOR_INTERVAL=str(args.monitor_interval),
    )
    command = [
        sys.executable,
        "-m",
        "locust",
        "-f",
        str(ROOT / "locustfile.py"),
        "--host",
        args.host,
        "--html",
        str(directory / "locust.html"),
        "--csv",
        str(directory / "locust"),
        "--csv-full-history",
        "--stop-timeout",
        "12",
    ]
    command += ["--web-host", "127.0.0.1"] if ui else ["--headless", "--only-summary"]
    done = threading.Event()
    serial_thread = None
    if args.serial:
        serial_thread = threading.Thread(
            target=serial_capture, args=(args.serial, directory, done), daemon=True
        )
        serial_thread.start()
    print(f"Running {args.scenario}; evidence: {directory}", flush=True)
    if ui:
        print("Locust UI: http://127.0.0.1:8089 (Ctrl+C to finish and build reports)", flush=True)
    budget = len(args.steps.split(",")) * args.stage if args.steps else args.duration
    process = subprocess.Popen(command, cwd=ROOT, env=env)
    code = 2
    try:
        code = process.wait(timeout=None if ui else budget + 90)
    except subprocess.TimeoutExpired, KeyboardInterrupt:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
    finally:
        done.set()
        if serial_thread:
            serial_thread.join(timeout=1)
        trace = Evidence(directory)
        if code and not any(e["kind"] == "anomaly" for e in report.read_events(directory / "events.jsonl")):
            trace.emit(
                "anomaly",
                reason=f"Locust exited with code {code}; inspect endpoint failures or terminal errors",
            )
        # With load removed, probe whether backpressure recovers; never reset/flash the board.
        for delay in (1, 3, 5):
            time.sleep(delay)
            if snapshot(args.host, trace, "after"):
                trace.emit("recovery", available=True, delay_seconds=delay)
                break
        if (directory / "serial.log").exists():
            for line in (directory / "serial.log").read_text(encoding="utf-8").splitlines():
                if any(word in line.lower() for word in ("panic", "out of memory", "abort", "starting")):
                    trace.emit("serial", text=line)
        trace.close()
        print(f"Report: {report.build(directory)}", flush=True)
    return code


def main():
    parser = argparse.ArgumentParser(description="NanaCoin board load laboratory")
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "e2e", "run", "suite", "ui"):
        cmd = commands.add_parser(name)
        cmd.add_argument("--host", default=os.getenv("NANA_HOST", "http://192.168.1.158"))
        if name in ("run", "suite", "ui"):
            cmd.add_argument("--scenario", choices=SCENARIOS, default="browse")
            cmd.add_argument("--users", type=int, default=4)
            cmd.add_argument("--duration", type=int, default=60, help="seconds at a fixed user count")
            cmd.add_argument("--steps", default="", help="comma-separated user counts, e.g. 1,2,4,8")
            cmd.add_argument("--stage", type=int, default=30)
            cmd.add_argument("--wait-min", type=float, default=0.3)
            cmd.add_argument("--wait-max", type=float, default=1)
            cmd.add_argument("--monitor-interval", type=float, default=5)
            cmd.add_argument(
                "--serial", help="optional native USB serial port; never use the flashing/reset bridge"
            )
    cmd = commands.add_parser("reports")
    cmd.add_argument("--port", type=int, default=8090)
    commands.add_parser("open")
    args = parser.parse_args()
    if args.command in ("reports", "open"):
        for path in REPORTS.glob("*/events.jsonl"):
            report.build(path.parent)
        report.index()
        if args.command == "open":
            webbrowser.open((REPORTS / "index.html").as_uri())
            return 0
        print(f"Reports: http://127.0.0.1:{args.port}", flush=True)
        handler = partial(http.server.SimpleHTTPRequestHandler, directory=str(REPORTS))
        with http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler) as server:
            try:
                server.serve_forever()
            except KeyboardInterrupt:
                pass
        return 0
    args.host = args.host.rstrip("/")
    if args.command in ("run", "suite", "ui"):
        try:
            steps = [int(x) for x in args.steps.split(",")] if args.steps else []
        except ValueError:
            parser.error("steps must be comma-separated positive integers")
        if (
            min([args.users, args.duration, args.stage, *steps]) <= 0
            or args.wait_min < 0
            or args.wait_max < args.wait_min
            or args.monitor_interval <= 0
        ):
            parser.error("counts/times must be positive; waits must satisfy 0 <= min <= max")
        if args.command == "suite":
            for scenario in SCENARIOS:
                args.scenario = scenario
                if code := run_load(args):
                    print("Suite stopped at the first failed workload. Inspect evidence before restarting.")
                    return code
            return 0
        return run_load(args, ui=args.command == "ui")
    directory = new_run(args.command)
    write_json(directory / "meta.json", {"scenario": args.command, "host": args.host})
    trace = Evidence(directory)
    try:
        if args.command == "prepare":
            prepare(args.host, trace)
        else:
            e2e(args.host, load_fixture(args.host), trace)
        return 0
    except (RuntimeError, AssertionError, OSError, ValueError, KeyError) as exc:
        # Errors contain endpoint/status information, never request bodies or credentials.
        trace.emit("anomaly", reason=str(exc))
        print(f"FAILED: {exc}", flush=True)
        return 2
    finally:
        trace.close()
        print(f"Report: {report.build(directory)}", flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
