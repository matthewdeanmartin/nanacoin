import json

from nanacoin_load.common import Evidence, health
from nanacoin_load.report import document, read_events, summarize


def test_health_parses_zero_padded_board_header():
    assert health("free 004656 inuse 277488 obj 00454 blk 1909 gc 00010") == {
        "free": 4656,
        "inuse": 277488,
        "obj": 454,
        "blk": 1909,
        "gc": 10,
    }


def test_summary_excludes_setup_and_preserves_failure_latency():
    rows = [
        {"kind": "start", "timestamp": 0},
        {"kind": "stop", "timestamp": 10},
        {"kind": "request", "role": "setup", "status": 200, "ms": 9000},
        {"kind": "request", "role": "load", "status": 200, "ms": 100, "health": {"free": 2000}},
        {"kind": "request", "role": "load", "status": 0, "ms": 10000, "error": "timeout"},
    ]
    result = summarize(rows)
    assert result["requests"] == 2
    assert result["failures"] == 1
    assert result["failure_percent"] == 50
    assert result["p95_ms"] == 10000
    assert result["rps"] == 0.2
    assert result["min_free"] == 2000


def test_interrupted_last_line_does_not_destroy_evidence(tmp_path):
    trace = Evidence(tmp_path)
    trace.emit("anomaly", reason="board timed out")
    trace.close()
    with (tmp_path / "events.jsonl").open("a") as f:
        f.write('{"kind":')
    assert len(read_events(tmp_path / "events.jsonl")) == 1


def test_report_escapes_title():
    assert "<title>&lt;script&gt;" in document("<script>", "")


def test_desktop_placeholder_diagnostics_do_not_claim_zero_heap():
    result = summarize([{"kind": "snapshot", "data": {"free_now": 0, "health": ""}}])
    assert result["min_free"] is None


def test_jsonl_flushes_each_record(tmp_path):
    trace = Evidence(tmp_path)
    trace.emit("check", name="visible before close", passed=True)
    assert json.loads((tmp_path / "events.jsonl").read_text())["passed"] is True
    trace.close()
