"""Offline HTML reports built from flushed evidence, including interrupted runs."""

import html
import json
import math
from pathlib import Path

from .common import REPORTS, write_json

CSS = """
:root{color-scheme:dark;font-family:system-ui,sans-serif;background:#0b1220;color:#e7edf7}
body{max-width:1180px;margin:40px auto;padding:0 24px}h1{font-size:36px;margin:8px 0}h2{font-size:20px}
.eyebrow{color:#65dec5;letter-spacing:.17em;font-size:12px;text-transform:uppercase}p{color:#aebdd2;line-height:1.6}
a{color:#69d8f5;text-decoration:none}a:hover{text-decoration:underline}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:14px;margin:28px 0}
.card,section{background:#142035;border:1px solid #27374e;border-radius:14px;padding:20px;margin-bottom:18px}.value{font-size:29px;font-weight:650;margin-top:8px}
.label{color:#aebdd2;font-size:13px}.warn{color:#ffcc80}.ok{color:#65dec5}table{border-collapse:collapse;width:100%;font-size:14px}td,th{text-align:left;padding:12px 8px;border-bottom:1px solid #2a3950}th{color:#aebdd2}code{color:#b0dbff}svg{width:100%;height:230px}small{color:#aebdd2}.links{display:flex;gap:22px;flex-wrap:wrap}.scroll{overflow:auto}footer{margin-top:30px;color:#91a1b6;font-size:13px}
"""


def read_events(path):
    result = []
    if not path.exists():
        return result
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            result.append(json.loads(line))
        except json.JSONDecodeError:
            pass  # Last line may have been interrupted; earlier evidence remains usable.
    return result


def percentile(values, p):
    if not values:
        return None
    values = sorted(values)
    return values[max(0, math.ceil(len(values) * p) - 1)]


def summarize(events):
    rows = [e for e in events if e["kind"] == "request" and e.get("role") == "load"]
    failed = [e for e in rows if e.get("error") or not 200 <= e.get("status", 0) < 400]
    free = [e["health"]["free"] for e in events if "free" in e.get("health", {})]
    free += [
        e["data"]["free_now"]
        for e in events
        if e["kind"] == "snapshot"
        and isinstance(e.get("data"), dict)
        and "free_now" in e["data"]
        and e["data"].get("health")
    ]
    checks = [e for e in events if e["kind"] == "check"]
    starts = [e["timestamp"] for e in events if e["kind"] == "start"]
    stops = [e["timestamp"] for e in events if e["kind"] == "stop"]
    duration = max(stops) - min(starts) if starts and stops else None
    return {
        "requests": len(rows),
        "failures": len(failed),
        "failure_percent": 100 * len(failed) / len(rows) if rows else 0,
        "p50_ms": percentile([e["ms"] for e in rows], 0.5),
        "p95_ms": percentile([e["ms"] for e in rows], 0.95),
        "p99_ms": percentile([e["ms"] for e in rows], 0.99),
        "min_free": min(free) if free else None,
        "rps": len(rows) / duration if duration and duration > 0 else None,
        "checks_passed": sum(e["passed"] for e in checks),
        "checks_total": len(checks),
        "anomalies": [e.get("reason", "unknown") for e in events if e["kind"] == "anomaly"],
    }


def chart(points, title, unit, color):
    if not points:
        return f"<section><h2>{title}</h2><p>No samples captured.</p></section>"
    # Preserve min/max information in summary; decimate only the drawn line.
    points = points[:: max(1, len(points) // 1600)]
    lo, hi = min(p[0] for p in points), max(p[0] for p in points)
    ymax = max(p[1] for p in points) or 1
    coords = " ".join(
        f"{50 + (x - lo) / max(hi - lo, 1) * 950:.1f},{190 - y / ymax * 155:.1f}" for x, y in points
    )
    return f'''<section><h2>{title}</h2><svg viewBox="0 0 1040 230" role="img" aria-label="{title}">
    <path d="M50 25V190H1000" fill="none" stroke="#405169"/>
    <polyline points="{coords}" fill="none" stroke="{color}" stroke-width="2"/>
    <g fill="#aebdd2" font-size="13"><text x="50" y="18">{ymax:,.0f} {unit}</text>
    <text x="50" y="217">0 s</text><text x="925" y="217">{hi - lo:.0f} s</text></g></svg></section>'''


def document(title, body):
    return f'<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>{html.escape(title)}</title><style>{CSS}</style><body>{body}</body></html>'


def build(run):
    run = Path(run)
    events = read_events(run / "events.jsonl")
    meta = json.loads((run / "meta.json").read_text()) if (run / "meta.json").exists() else {}
    summary = summarize(events)
    write_json(run / "summary.json", summary)
    fmt = lambda v: "—" if v is None else f"{v:,.1f}"
    cards = [
        ("Workload requests", str(summary["requests"])),
        ("Workload failure rate", fmt(summary["failure_percent"]) + "%" if summary["requests"] else "—"),
        ("p95 latency", fmt(summary["p95_ms"]) + " ms"),
        ("Throughput", fmt(summary["rps"]) + " req/s"),
        ("Lowest observed free heap", fmt(summary["min_free"]) + " B"),
    ]
    body = '<div class="eyebrow">NanaCoin · board load laboratory</div>'
    body += f"<h1>{html.escape(meta.get('scenario', run.name))}</h1><p>{html.escape(run.name)} · {html.escape(meta.get('host', ''))}</p>"
    body += '<div class="links"><a href="../index.html">All experiments</a><a href="events.jsonl">Raw evidence</a><a href="summary.json">Summary JSON</a>'
    if (run / "locust.html").exists():
        body += '<a href="locust.html">Locust endpoint report</a><a href="locust_stats.csv">Endpoint CSV</a>'
    body += (
        '</div><div class="cards">'
        + "".join(
            f'<div class="card"><div class="label">{k}</div><div class="value">{v}</div></div>'
            for k, v in cards
        )
        + "</div>"
    )
    reasons = summary["anomalies"] or [
        "No automatic stop condition recorded. This is not a crash-free certification."
    ]
    body += "<section><h2>Outcome and interpretation</h2>" + "".join(
        f'<p class="warn">{html.escape(x)}</p>' for x in reasons
    )
    body += f"<p>HTTP timeouts establish unresponsiveness, not its cause. A panic/OOM in serial output or an uptime reset provides stronger evidence. Diagnostic polling adds one request about every five seconds. E2E checks: {summary['checks_passed']}/{summary['checks_total']} passed.</p></section>"
    reqs = [e for e in events if e["kind"] == "request" and e.get("role") == "load"]
    latency_rows = reqs or [e for e in events if e["kind"] == "request"]
    body += chart(
        [(e["timestamp"], e["ms"]) for e in latency_rows],
        "Workload request latency" if reqs else "Setup / E2E request latency",
        "ms",
        "#72c9ff",
    )
    heap = [(e["timestamp"], e["health"]["free"]) for e in events if "free" in e.get("health", {})]
    body += chart(heap, "Free heap at responses", "bytes", "#65dec5")
    body += '<section><h2>Endpoints, including fixture setup</h2><div class="scroll"><table><tr><th>Role</th><th>Endpoint</th><th>Requests</th><th>Errors / 4xx / 5xx</th><th>p95 ms</th></tr>'
    groups = {}
    for e in events:
        if e["kind"] == "request":
            groups.setdefault((e.get("role", ""), e.get("name", "")), []).append(e)
    for (role, name), group in sorted(groups.items()):
        errors = sum(bool(e.get("error")) or not 200 <= e.get("status", 0) < 400 for e in group)
        body += f"<tr><td>{html.escape(role)}</td><td>{html.escape(name)}</td><td>{len(group)}</td><td>{errors}</td><td>{percentile([e['ms'] for e in group], 0.95):.1f}</td></tr>"
    body += "</table></div><small>E2E deliberately requests a forbidden route; its expected 403 appears here and is validated by the E2E checks.</small></section>"
    body += '<section><h2>Stage and incident timeline</h2><div class="scroll"><table><tr><th>Seconds</th><th>Event</th><th>Detail</th></tr>'
    start = events[0]["timestamp"] if events else 0
    for e in events:
        if e["kind"] not in ("stage", "anomaly", "monitor_failure", "recovery", "check", "serial"):
            continue
        detail = e.get("reason", e.get("name", e.get("text", "")))
        if e["kind"] == "stage":
            detail = f"{e['users']} virtual users"
        if e["kind"] in ("monitor_failure", "recovery"):
            detail = str({k: v for k, v in e.items() if k not in ("kind", "timestamp", "elapsed")})
        body += f"<tr><td>{e['timestamp'] - start:.1f}</td><td>{html.escape(e['kind'])}</td><td>{html.escape(str(detail))}</td></tr>"
    body += "</table></div></section><footer>Offline report · No CDN or network access required · Raw evidence is flushed after each event.</footer>"
    (run / "index.html").write_text(document("NanaCoin experiment", body), encoding="utf-8")
    index()
    return run / "index.html"


def index():
    REPORTS.mkdir(exist_ok=True)
    body = '<div class="eyebrow">NanaCoin · board load laboratory</div><h1>Experiment reports</h1><p>Compare workloads, locate the first degradation, and inspect the evidence before changing firmware.</p><section><div class="scroll"><table><tr><th>Experiment</th><th>Host</th><th>Requests</th><th>Failures</th><th>p95 ms</th><th>Min free bytes</th></tr>'
    for path in sorted(REPORTS.glob("*/summary.json"), reverse=True):
        s = json.loads(path.read_text())
        meta_path = path.parent / "meta.json"
        meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}
        latency = f"{s['p95_ms']:.1f}" if s["p95_ms"] is not None else "—"
        body += f'<tr><td><a href="{path.parent.name}/index.html">{html.escape(path.parent.name)}</a></td><td>{html.escape(meta.get("host", ""))}</td><td>{s["requests"]}</td><td>{s["failures"]}</td><td>{latency}</td><td>{s["min_free"] if s["min_free"] is not None else "—"}</td></tr>'
    body += "</table></div></section>"
    (REPORTS / "index.html").write_text(document("NanaCoin load laboratory", body), encoding="utf-8")
