#!/usr/bin/env python3
"""Probe and stress the NanaCoin board without ever hanging.

# Why this exists

Every earlier attempt to test the board from PowerShell hung. A request to a
board that has stopped answering costs the full timeout, and a loop of those
costs half an hour to learn one fact - so the tool meant to detect a hang
became the hang. Worse, a fixed-timeout loop cannot tell "dead" from "slow",
which is exactly the distinction that matters here.

So: every request has a short hard deadline, the whole run has a wall-clock
budget, and each sample prints as it happens. A board that stops answering is
visible on the next line, not after the script finishes.

# What it measures

The board's real ceiling, honestly. It writes ledger records continuously and
reads the growing history back at the largest page allowed, because both
growth curves matter and only the combination has ever broken it. Failures do
not stop the run - "dies at round 40 and recovers" and "dies at round 40 and
stays dead" are different answers and the second is worse.

# Usage

    python probe.py watch [--seconds 120] [--interval 2]
        Poll /status and report uptime and every up/down transition.

    python probe.py stress [--rounds 40] [--tx-per-round 5]
        Write and read until it breaks or the rounds run out.

    python probe.py provision
        Create the household and a few users, then save the token.

Every mode exits non-zero if the board finished unreachable, so this can gate
a build.
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
from pathlib import Path

DEFAULT_IP = "192.168.1.158"
ORIGIN = "http://localhost:4200"

# Short on purpose. A healthy board answers on the LAN in well under a second;
# past a couple of seconds it is in trouble, and waiting longer only delays the
# next sample. Writes get more room because they do real work (a PBKDF2 hash on
# user creation, a journal append) before replying.
# Generous on purpose.
#
# An earlier version used 3s and counted anything slower as "dead", which
# meant a board answering in 3.5s under load was reported as crashed. That is
# a desktop's expectation applied to hardware with a 1980s memory budget and
# a weak radio link: this board answers a simple read in tens of milliseconds
# when idle and takes seconds when four clients are queued behind one another,
# and both are healthy.
#
# 10s separates "slow" from "gone" far better than 3s did.
READ_TIMEOUT = 10.0

# Generous, because the first write after a boot is genuinely slow: the board
# has just associated, the TCP pool is cold, and a provision or user creation
# does a PBKDF2 hash before replying. A 15s ceiling timed out on the very
# first provision and made a working board look unreachable.
WRITE_TIMEOUT = 45.0

TOKEN_FILE = Path(__file__).with_name(".token")

_pool = ThreadPoolExecutor(max_workers=8)


class Board:
    def __init__(self, ip: str):
        self.base = f"http://{ip}"
        self.token = None
        if TOKEN_FILE.exists():
            self.token = TOKEN_FILE.read_text().strip() or None

    def _headers(self, auth: bool):
        h = {"Origin": ORIGIN}
        if auth and self.token:
            h["Authorization"] = f"Bearer {self.token}"
        return h

    def _blocking(self, method, path, body, auth, timeout):
        data = None
        headers = self._headers(auth)
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(
            self.base + path, data=data, headers=headers, method=method
        )
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, dict(r.headers), r.read()

    async def request(self, method, path, body=None, auth=True, timeout=None):
        """Returns (status, headers, body). status 0 means no answer.

        Never raises and never outlives its deadline, which is the whole point
        of the module.
        """
        if timeout is None:
            timeout = WRITE_TIMEOUT if method != "GET" else READ_TIMEOUT
        loop = asyncio.get_running_loop()
        try:
            return await asyncio.wait_for(
                loop.run_in_executor(
                    _pool, self._blocking, method, path, body, auth, timeout
                ),
                timeout=timeout + 0.5,
            )
        except asyncio.TimeoutError:
            return 0, {}, b"timeout"
        except urllib.error.HTTPError as e:
            try:
                return e.code, dict(e.headers), e.read()
            except Exception:
                return e.code, {}, b""
        except Exception as e:
            return 0, {}, str(e).encode()[:80]

    async def get(self, path, **kw):
        return await self.request("GET", path, **kw)

    async def post(self, path, body, **kw):
        return await self.request("POST", path, body, **kw)


def health_of(headers):
    return headers.get("X-Nanacoin-Health", "")


async def cmd_watch(board: Board, args):
    """Poll /status, reporting uptime and every transition."""
    start = time.monotonic()
    up = down = 0
    transitions = []
    last = None

    print(
        f"polling {board.base}/api/v1/status every {args.interval}s "
        f"for {args.seconds}s (timeout {READ_TIMEOUT}s)",
        flush=True,
    )

    while time.monotonic() - start < args.seconds:
        t = time.monotonic() - start
        status, headers, body = await board.get("/api/v1/status", auth=False)
        alive = status == 200
        up, down = (up + 1, down) if alive else (up, down + 1)

        if last is not None and alive != last:
            transitions.append((round(t, 1), "UP" if alive else "DOWN"))
            print(f"  >>> {t:5.1f}s -> {'UP' if alive else 'DOWN'}", flush=True)
        last = alive

        if alive:
            txns = "?"
            try:
                txns = json.loads(body).get("transactions", "?")
            except Exception:
                pass
            print(f"{t:6.1f}s  200  tx={txns}  {health_of(headers)}", flush=True)
        else:
            print(f"{t:6.1f}s  ---  {body.decode('utf-8','replace')[:50]}", flush=True)

        await asyncio.sleep(args.interval)

    total = max(up + down, 1)
    print(f"\n=== {up}/{total} up ({100*up//total}%), "
          f"{len(transitions)} transitions ===")
    for t, s in transitions:
        print(f"    {t}s -> {s}")
    return 0 if last else 1


async def cmd_provision(board: Board, args):
    """Create the household, log in, add users. Saves the token."""
    status, _, body = await board.post(
        "/api/v1/provision",
        {
            "username": "nana",
            "display_name": "Nana",
            "password": "nana-pin",
            "household_name": "Stress House",
        },
        auth=False,
    )
    print(f"provision: {status}")
    if status not in (201, 403):  # 403 = already provisioned, which is fine
        print("cannot provision; is the board up?")
        return 1

    verifier = "a" * 43
    challenge = (
        base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest())
        .decode()
        .rstrip("=")
    )
    status, _, body = await board.post(
        "/api/v1/auth/authorize",
        {
            "username": "nana",
            "password": "nana-pin",
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "redirect_uri": f"{ORIGIN}/cb",
        },
        auth=False,
    )
    if status != 200:
        print(f"authorize failed: {status} {body[:120]}")
        return 1
    code = json.loads(body)["code"]

    status, _, body = await board.post(
        "/api/v1/auth/token",
        {"code": code, "code_verifier": verifier, "redirect_uri": f"{ORIGIN}/cb"},
        auth=False,
    )
    if status != 200:
        print(f"token failed: {status} {body[:120]}")
        return 1
    board.token = json.loads(body)["access_token"]
    TOKEN_FILE.write_text(board.token)
    print(f"login: ok (token saved to {TOKEN_FILE.name})")

    for name in ("alice", "bob", "carol"):
        status, _, _ = await board.post(
            "/api/v1/users",
            {"username": name, "display_name": name, "password": f"{name}-pin"},
        )
        print(f"user {name}: {status}")
    return 0


async def cmd_stress(board: Board, args):
    """Write and read until it breaks or the rounds run out."""
    status, _, body = await board.get("/api/v1/users")
    if status != 200:
        print(f"cannot list users ({status}); run `provision` first, "
              f"and check the board is up")
        return 1
    accts = [u["account"] for u in json.loads(body)["users"]]
    if len(accts) < 2:
        print("need at least two accounts; run `provision`")
        return 1
    nana = accts[0]
    print(f"accounts: {len(accts)}", flush=True)

    total_ok = total_fail = 0
    first_fail = None
    consecutive_dead = 0

    for rnd in range(1, args.rounds + 1):
        ok = fail = 0
        errs = []
        health = ""

        for i in range(args.tx_per_round):
            target = accts[(i % (len(accts) - 1)) + 1]
            st, _, _ = await board.post(
                "/api/v1/admin/issue",
                {"to": target, "amount": 5, "reason": f"stress {rnd}"},
            )
            ok, fail = (ok + 1, fail) if st == 201 else (ok, fail + 1)
            if st != 201:
                errs.append(f"issue:{st}")

            # Transfer between two *different* member accounts. An earlier
            # version sent to Nana's own account, which the API correctly
            # refuses as a self-deal - so the run reported a 16% failure rate
            # that was entirely the test's fault.
            other = accts[((i + 1) % (len(accts) - 1)) + 1]
            st, _, _ = await board.post(
                "/api/v1/transfers",
                {"to": other, "amount": 1, "memo": f"r{rnd}-{i}"},
            )
            ok, fail = (ok + 1, fail) if st == 201 else (ok, fail + 1)
            if st != 201:
                errs.append(f"xfer:{st}")

        if rnd % 3 == 0:
            st, _, _ = await board.post(
                "/api/v1/listings",
                {"title": f"Item r{rnd}", "description": "x" * 200, "price": 3},
            )
            ok, fail = (ok + 1, fail) if st == 201 else (ok, fail + 1)
            if st != 201:
                errs.append(f"listing:{st}")

        for p in (
            "/api/v1/transactions?limit=100",
            "/api/v1/users",
            "/api/v1/listings",
            f"/api/v1/accounts/{nana}/transactions?limit=100",
            "/api/v1/logs?limit=100",
        ):
            st, h, _ = await board.get(p)
            if st == 200:
                ok += 1
                if health_of(h):
                    health = health_of(h)
            else:
                fail += 1
                errs.append(f"read:{st}")

        total_ok += ok
        total_fail += fail
        if fail and first_fail is None:
            first_fail = rnd

        st, _, body = await board.get("/api/v1/status", auth=False)
        txns = "?"
        if st == 200:
            try:
                txns = json.loads(body).get("transactions", "?")
            except Exception:
                pass

        print(
            f"r{rnd:<3} tx={txns:<5} ok={ok:<3} fail={fail:<3} {health} "
            f"{','.join(errs[:4])}",
            flush=True,
        )

        # Give up when the board has stopped answering entirely for a few
        # rounds. Without this the run burns its whole budget against a dead
        # board, which is the exact hang this tool exists to avoid.
        if ok == 0:
            consecutive_dead += 1
            if consecutive_dead >= args.give_up_after:
                print(
                    f"\nboard answered nothing for {consecutive_dead} rounds "
                    f"- stopping at round {rnd}",
                    flush=True,
                )
                break
        else:
            consecutive_dead = 0

    print(f"\n=== ok={total_ok} fail={total_fail} ===")
    if first_fail:
        print(f"first failure at round {first_fail}")
    else:
        print("no failures")

    st, _, body = await board.get("/api/v1/diag", auth=False)
    print(f"final diag: {body.decode('utf-8','replace') if st == 200 else 'UNREACHABLE'}")
    return 0 if st == 200 else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ip", default=DEFAULT_IP)
    sub = ap.add_subparsers(dest="cmd", required=True)

    w = sub.add_parser("watch", help="poll /status and report transitions")
    w.add_argument("--seconds", type=float, default=120.0)
    w.add_argument("--interval", type=float, default=2.0)

    sub.add_parser("provision", help="create the household and users")

    s = sub.add_parser("stress", help="write and read until it breaks")
    s.add_argument("--rounds", type=int, default=40)
    s.add_argument("--tx-per-round", type=int, default=5)
    s.add_argument(
        "--give-up-after",
        type=int,
        default=3,
        help="stop after this many rounds with zero successful requests",
    )

    args = ap.parse_args()
    board = Board(args.ip)
    fn = {"watch": cmd_watch, "provision": cmd_provision, "stress": cmd_stress}[args.cmd]
    sys.exit(asyncio.run(fn(board, args)))


if __name__ == "__main__":
    main()
