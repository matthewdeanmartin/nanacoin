"""Read-only TLS handshake/state soak. Run beside the board's serial heap log."""
import argparse
import base64
import getpass
import hashlib
from collections import deque
from concurrent.futures import ThreadPoolExecutor
import json
import os
import secrets
import ssl
import statistics
import time
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="https://nanacoin.local")
    parser.add_argument("--ca", default="certs/home-ca.crt")
    parser.add_argument("--seconds", type=int, default=60)
    args = parser.parse_args()
    if not 1 <= args.seconds <= 86400:
        parser.error("seconds must be between 1 and 86400")
    if not args.url.startswith("https://"):
        parser.error("board soak requires HTTPS")
    context = ssl.create_default_context(cafile=args.ca)

    def post(path, payload):
        request = urllib.request.Request(args.url.rstrip('/') + '/api/v1' + path,
            data=json.dumps(payload).encode(), headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, context=context, timeout=10) as response:
            return json.loads(response.read(131073))

    username = os.environ.get('NANACOIN_USERNAME') or input('Username: ')
    password = os.environ.get('NANACOIN_PASSWORD') or getpass.getpass('PIN/password: ')
    verifier = secrets.token_urlsafe(32)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip('=')
    redirect = 'http://localhost:4200/'
    code = post('/auth/authorize', dict(username=username, password=password,
        code_challenge=challenge, code_challenge_method='S256', redirect_uri=redirect))['code']
    token = post('/auth/token', dict(code=code, code_verifier=verifier, redirect_uri=redirect))['access_token']
    deadline = time.monotonic() + args.seconds

    def worker():
        success = failures = 0
        timings = deque(maxlen=4096)
        last_error = None
        while time.monotonic() < deadline:
            start = time.monotonic()
            request = urllib.request.Request(args.url.rstrip("/") + "/api/v1/state", headers={"Authorization": "Bearer " + token, "Connection": "close"})
            try:
                with urllib.request.urlopen(request, context=context, timeout=10) as response:
                    data = response.read(131073)
                    if len(data) > 131072:
                        raise ValueError("response exceeded bound")
                    json.loads(data)["state"]["sequence"]
                success += 1
                timings.append((time.monotonic() - start) * 1000)
            except Exception as error:
                failures += 1
                last_error = str(error)
            time.sleep(0.05)
        return success, failures, list(timings), last_error

    with ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(lambda _: worker(), range(2)))
    timings = sorted(t for result in results for t in result[2])
    print(json.dumps({"success": sum(r[0] for r in results), "failures": sum(r[1] for r in results), "median_ms": statistics.median(timings) if timings else None, "p95_ms": timings[int((len(timings) - 1) * .95)] if timings else None, "last_errors": [r[3] for r in results if r[3]], "timing_window": "up to last 4096 successes per client"}, indent=2))
    if any(r[1] for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
