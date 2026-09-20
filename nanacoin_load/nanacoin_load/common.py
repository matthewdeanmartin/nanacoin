import base64
import hashlib
import json
import os
import re
import secrets
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STATE = ROOT / ".state" / "fixture.json"
REPORTS = ROOT / "reports"
ORIGIN = os.getenv("NANA_ORIGIN", "http://localhost:4200")
# The Rust board serves HTTPS only, with the gitignored development
# certificate. NANA_CA points at that certificate; "0" disables
# verification entirely for a board whose cert does not match its name.
CA = os.getenv("NANA_CA", "")
VERIFY = False if CA == "0" else (CA or True)
# No /api/v1/diag on the Rust firmware; status is public and carries
# the ledger invariant. Heap comes from serial, not response headers.
MONITOR_PATH = os.getenv("NANA_MONITOR_PATH", "/api/v1/diag")
SCENARIOS = ("status", "browse", "ledger", "write", "replay", "market", "auth", "seed", "forex")


def health(text):
    return {k: int(v) for k, v in re.findall(r"\b(free|inuse|obj|blk|gc)\s+(\d+)", text or "")}


def pkce():
    verifier = secrets.token_urlsafe(36)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
    return verifier, challenge


def credentials():
    return os.getenv("NANA_USERNAME", "nana"), os.getenv("NANA_PASSWORD", "nana-pin")


def load_fixture(host):
    data = json.loads(STATE.read_text(encoding="utf-8"))
    if data["host"] != host.rstrip("/"):
        raise ValueError("Fixture belongs to another host; run prepare for this board")
    return data


class Evidence:
    def __init__(self, directory):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.file = (self.directory / "events.jsonl").open("a", encoding="utf-8", buffering=1)
        self.started = time.monotonic()
        self.lock = threading.Lock()

    def emit(self, kind, **fields):
        row = dict(
            kind=kind, timestamp=time.time(), elapsed=round(time.monotonic() - self.started, 3), **fields
        )
        with self.lock:
            self.file.write(json.dumps(row, ensure_ascii=True) + "\n")

    def close(self):
        self.file.close()


def new_run(label):
    path = REPORTS / (time.strftime("%Y%m%d-%H%M%S") + "-" + label + "-" + secrets.token_hex(2))
    path.mkdir(parents=True)
    return path


def write_json(path, data):
    Path(path).write_text(json.dumps(data, indent=2), encoding="utf-8")
