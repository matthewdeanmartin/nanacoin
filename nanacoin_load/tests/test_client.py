import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest

from nanacoin_load.client import API
from nanacoin_load.common import Evidence, pkce


def test_pkce_has_required_lengths():
    verifier, challenge = pkce()
    assert 43 <= len(verifier) <= 128
    assert len(challenge) == 43
    assert verifier != pkce()[0]


def test_http_failure_is_recorded_without_secret_body(tmp_path):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(403)
            self.send_header("X-Nanacoin-Health", "free 001024 gc 00007")
            self.end_headers()
            self.wfile.write(b'{"access_token":"secret-not-in-reports"}')

        def log_message(self, *args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    trace = Evidence(tmp_path)
    try:
        with pytest.raises(RuntimeError, match="HTTP 403"):
            API(f"http://127.0.0.1:{server.server_port}", trace).call("GET", "/me", token="secret")
    finally:
        trace.close()
        server.shutdown()
        server.server_close()
        thread.join()
    content = (tmp_path / "events.jsonl").read_text()
    assert "secret" not in content
    result = json.loads(content.splitlines()[-1])
    assert result["status"] == 403
    assert result["health"]["free"] == 1024
