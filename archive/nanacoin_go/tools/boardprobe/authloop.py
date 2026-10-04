#!/usr/bin/env python3
"""Hammer the login/logout cycle, which nothing else tests.

# Why this exists

Sequential reads: 45 transactions, hundreds of requests, fine.
Ten parallel connections: dropped some, survived, heap flat.
A person clicking around in a browser: dead inside a minute.

The difference is what a person does that a read probe never does - log in,
log out, log in again. That path is the heaviest thing this API has:

  - two PBKDF2 hashes per login attempt (authorize, then the verify inside it)
  - an authorization code minted, stored, swept and deleted
  - a session minted into a map, and RevokeUser walking that map on logout
  - all of it under the single global service lock

and it is the one path with *no test coverage on hardware at all*.

    python authloop.py [--cycles 15]
"""

import argparse
import asyncio
import base64
import hashlib
import json
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

DEFAULT_IP = "192.168.1.158"
ORIGIN = "http://localhost:4200"
READ_TIMEOUT = 5.0
WRITE_TIMEOUT = 45.0

_pool = ThreadPoolExecutor(max_workers=8)


def _blocking(base, method, path, body, token, timeout):
    headers = {"Origin": ORIGIN}
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(base + path, data=data, headers=headers,
                                 method=method)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, dict(r.headers), r.read()


async def call(base, method, path, body=None, token=None, timeout=None):
    if timeout is None:
        timeout = WRITE_TIMEOUT if method != "GET" else READ_TIMEOUT
    loop = asyncio.get_running_loop()
    try:
        return await asyncio.wait_for(
            loop.run_in_executor(_pool, _blocking, base, method, path, body,
                                 token, timeout),
            timeout=timeout + 0.5,
        )
    except asyncio.TimeoutError:
        return 0, {}, b"timeout"
    except urllib.error.HTTPError as e:
        return e.code, {}, b""
    except Exception as e:
        return 0, {}, str(e).encode()[:60]


async def login(base, user, pw):
    """Full PKCE flow. Returns (token, note)."""
    verifier = "a" * 43
    challenge = (
        base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest())
        .decode().rstrip("=")
    )
    st, _, body = await call(
        base, "POST", "/api/v1/auth/authorize",
        {"username": user, "password": pw, "code_challenge": challenge,
         "code_challenge_method": "S256", "redirect_uri": f"{ORIGIN}/cb"},
    )
    if st != 200:
        return None, f"authorize:{st}"
    code = json.loads(body)["code"]

    st, _, body = await call(
        base, "POST", "/api/v1/auth/token",
        {"code": code, "code_verifier": verifier,
         "redirect_uri": f"{ORIGIN}/cb"},
    )
    if st != 200:
        return None, f"token:{st}"
    return json.loads(body)["access_token"], "ok"


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ip", default=DEFAULT_IP)
    ap.add_argument("--cycles", type=int, default=15)
    args = ap.parse_args()
    base = f"http://{args.ip}"

    print(f"login/logout x{args.cycles} against {base}\n", flush=True)

    for i in range(1, args.cycles + 1):
        t0 = time.monotonic()
        token, note = await login(base, "nana", "nana-pin")
        login_s = time.monotonic() - t0
        if not token:
            print(f"cycle {i:<3} LOGIN FAILED ({note}) after {login_s:.1f}s",
                  flush=True)
            st, _, _ = await call(base, "GET", "/api/v1/status")
            if st != 200:
                print("*** board is not answering - this is the failure ***")
                return 2
            continue

        # What a person does after logging in.
        health = ""
        for p in ("/api/v1/me", "/api/v1/users", "/api/v1/listings",
                  "/api/v1/transactions?limit=30"):
            st, h, _ = await call(base, "GET", p, token=token)
            if h.get("X-Nanacoin-Health"):
                health = h["X-Nanacoin-Health"]

        st, _, _ = await call(base, "POST", "/api/v1/auth/logout", {},
                              token=token)
        total = time.monotonic() - t0
        print(f"cycle {i:<3} login {login_s:4.1f}s  logout:{st}  "
              f"total {total:4.1f}s  {health}", flush=True)

        st, _, _ = await call(base, "GET", "/api/v1/status", timeout=5.0)
        if st != 200:
            print(f"\n*** board stopped answering after {i} login cycles ***")
            return 2

    print("\nsurvived every cycle")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
