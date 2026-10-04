#!/usr/bin/env python3
"""Summarize a bottleneck run offline, including incomplete/interrupted runs."""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import re
import statistics


def growth_rows(lines):
    rows, pending = [], {}
    for line in lines:
        match = re.search(r"ledger-growth (\w+) (before|after) len (\d+) cap (\d+) elem (\S+) buffer-bytes (\S+) free (\d+)", line)
        if not match:
            continue
        name, phase, length, capacity, element, size, free = match.groups()
        if phase == "before":
            row = dict(slice=name, length=int(length), capacity=int(capacity),
                       element=int(element, 16) if element.startswith("0x") else int(element),
                       bytes=int(size, 16) if size.startswith("0x") else int(size),
                       free=int(free), after="no after marker")
            rows.append(row)
            pending[name] = row
        elif name in pending:
            pending.pop(name)["after"] = int(capacity)
    return rows


def report(directory):
    directory = Path(directory)
    phases = defaultdict(list)
    serial = defaultdict(list)
    active = {}
    anomalies, tail, snapshots, growth = [], [], [], []
    malformed = 0
    last_radio = None
    for raw in (directory / "events.jsonl").read_text(encoding="utf-8").splitlines():
        try:
            e = json.loads(raw)
        except ValueError:
            malformed += 1
            continue  # a host interruption can leave a partial final line
        kind = e["kind"]
        if kind == "request_start":
            active[e["id"]] = e
        elif kind == "response_headers":
            active[e["id"]] = e
        elif kind == "request_end":
            active.pop(e["id"], None)
            phases[e["phase"]].append(e)
        elif kind in ("anomaly", "run_interrupted", "recovery_end", "serial_unavailable", "serial_disconnected"):
            anomalies.append(e)
        elif kind == "snapshot":
            snapshots.append(e)
        elif kind == "serial":
            line = e["line"]
            tail.append(f"{e['seconds']:.3f}s {line}")
            tail = tail[-20:]
            if line.startswith("ledger-growth "):
                growth.append(f"{e['seconds']:.3f}s {line}")
            if line.startswith("heap conn "):
                fields = {k: int(v) for k, v in re.findall(
                    r"\b(conn|refused|free|inuse|obj|mallocs|frees|gc)\s+(-?\d+)", line)}
                serial[e["phase"]].append(fields)
            elif "radio" in line.lower():
                last_radio = line

    lines = ["# Board experiment evidence", "", str(directory.resolve()), "",
             "All results describe the flashed firmware and the board's existing state at run start.", "",
             "| Phase | Load requests | Status counts (load) | Monitor requests | First/last serial free* | Mallocs/connection* |",
             "|---|---:|---|---:|---|---:|"]
    for name, rows in phases.items():
        load = [r for r in rows if r["role"] == "load"]
        counts = dict(Counter(r["status"] for r in load))
        samples = serial[name]
        memory, allocs = "unavailable", "unavailable"
        if samples:
            free = [s["free"] for s in samples if "free" in s]
            if free:
                memory = f"{statistics.median(free[:10]):g} / {statistics.median(free[-10:]):g}"
            first, last = samples[0], samples[-1]
            connections = last.get("conn", 0) - first.get("conn", 0)
            delta = last.get("mallocs", 0) - first.get("mallocs", 0)
            if connections > 0 and delta >= 0:
                allocs = f"{delta / connections:.1f}"
        lines.append(f"| {name} | {len(load)} | {counts} | {sum(r['role'] == 'monitor' for r in rows)} | {memory} | {allocs} |")
    lines += ["", "*Serial free uses medians of the first/last ten samples. Allocation deltas include monitoring and other board activity; they are not per-handler allocation counts. Counters spanning a reboot must not be compared.", "",
              "## Events requiring attention", ""]
    lines += ["- " + json.dumps(e, ensure_ascii=True) for e in anomalies] or ["No recorded anomaly."]
    lines += ["", "## Requests without a completion record", ""]
    lines += ["- " + json.dumps(e, ensure_ascii=True) for e in active.values()] or ["None."]
    lines += ["", "## Ledger growth boundaries", "",
              "| Slice | Length before | Capacity before | Element bytes | Requested buffer bytes | Free before | Capacity after |",
              "|---|---:|---:|---:|---:|---:|---|"]
    for row in growth_rows(growth):
        lines.append("| " + " | ".join(str(row[k]) for k in ("slice", "length", "capacity", "element", "bytes", "free", "after")) + " |")
    lines += ["", "Requested sizes use the diagnostic's TinyGo growth estimate. No after marker can also mean incomplete serial capture; correlate with fatal output.", "",
              "## Ledger growth markers", "", "```text", *(growth or ["No growth markers captured (requires ledgertrace firmware)."]), "```", "",
              "## Last board snapshots", "", "```json", json.dumps(snapshots[-2:], indent=2), "```", "",
              "## Last serial lines", "", "```text", *tail, "```", "",
              "Last radio line: " + (last_radio or "unavailable"), "",
              "## Interpretation limits", "",
              "- A timeout identifies the failed stage, not the cause. Recovery after removing load supports transient overload/link trouble; it does not prove it.",
              "- Missing serial output or a missing fatal message does not exclude OOM, stack overflow, watchdog reset, or a disconnected console.",
              "- `frag_now` / `blk` is average blocks per object, not a fragmentation measurement. `alloc_failures` counts unsuccessful response writes, including peers disconnecting.",
              "- `/diag` shares the HTTP workers and locks being stressed. Its timeout is not independent evidence that the CPU stopped. Serial provides a separate observation channel.",
              "- Heap samples occur in the accept loop after GC, potentially while workers are active. Neither these nor the health header measure the handler's peak or largest free block.",
              f"- Ignored {malformed} malformed event lines (usually an interrupted final write).", ""]
    target = directory / "report.md"
    target.write_text("\n".join(lines), encoding="utf-8")
    return target


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory")
    print(report(parser.parse_args().directory))
