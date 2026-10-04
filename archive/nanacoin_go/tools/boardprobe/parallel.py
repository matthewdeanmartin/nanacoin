#!/usr/bin/env python3
"""Find out how many simultaneous connections the board survives.

# Why this exists

The sequential probe ran 45 transactions and hundreds of requests without
failing. A person clicking around in a browser killed the board in under a
minute. Same network, same endpoints - so the difference is not load, it is
*shape*: a browser opens several connections at once and keeps them alive,
while the probe opened one at a time and closed it.

That is a plausible failure on this board by construction. httphi runs
FixedNumGoroutines workers (2), the adapter hands out that many buffer sets,
and the TCP pool holds 8 slots. A third simultaneous request has to wait for a
buffer while holding a pool slot and a goroutine stack, which is exactly the
pile-up the worker/buffer mismatch was supposed to have fixed.

This walks concurrency up one level at a time and reports where it breaks, so
the answer is a number rather than a theory.

    python parallel.py [--max 6] [--requests 12] [--ip ...]
"""

import argparse
import asyncio
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

DEFAULT_IP = "192.168.1.158"
ORIGIN = "http://localhost:4200"
TIMEOUT = 8.0

_pool = ThreadPoolExecutor(max_workers=32)


def _blocking_get(url, timeout):
    req = urllib.request.Request(url, headers={"Origin": ORIGIN})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, dict(r.headers)


async def get(url, timeout=TIMEOUT):
    loop = asyncio.get_running_loop()
    try:
        return await asyncio.wait_for(
            loop.run_in_executor(_pool, _blocking_get, url, timeout),
            timeout=timeout + 0.5,
        )
    except asyncio.TimeoutError:
        return 0, {}
    except urllib.error.HTTPError as e:
        return e.code, {}
    except Exception:
        return 0, {}


async def alive(base, tries=3):
    """Is the board answering at all? Sequential, so it cannot itself be the
    thing that knocks it over."""
    for _ in range(tries):
        st, _ = await get(f"{base}/api/v1/status", timeout=4.0)
        if st == 200:
            return True
        await asyncio.sleep(2)
    return False


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ip", default=DEFAULT_IP)
    ap.add_argument("--max", type=int, default=6,
                    help="highest concurrency level to try")
    ap.add_argument("--requests", type=int, default=12,
                    help="requests per level")
    args = ap.parse_args()
    base = f"http://{args.ip}"

    if not await alive(base):
        print("board is not answering before we start - reset it first")
        return 1
    print(f"board is up; walking concurrency 1..{args.max}\n", flush=True)

    # A browser's real pattern: a mix of endpoints, not one hot path.
    paths = [
        "/api/v1/status",
        "/api/v1/logs?limit=20",
        "/api/v1/listings",
        "/api/v1/diag",
    ]

    for level in range(1, args.max + 1):
        t0 = time.monotonic()
        results = []
        sent = 0
        while sent < args.requests:
            batch = min(level, args.requests - sent)
            # The whole point: `batch` requests genuinely in flight together.
            tasks = [
                get(f"{base}{paths[(sent + i) % len(paths)]}")
                for i in range(batch)
            ]
            results.extend(await asyncio.gather(*tasks))
            sent += batch

        ok = sum(1 for st, _ in results if st == 200)
        dead = sum(1 for st, _ in results if st == 0)
        elapsed = time.monotonic() - t0
        health = ""
        for st, h in results:
            if h.get("X-Nanacoin-Health"):
                health = h["X-Nanacoin-Health"]

        print(f"concurrency {level}: {ok}/{len(results)} ok, {dead} no-answer, "
              f"{elapsed:4.1f}s  {health}", flush=True)

        # Survival check between levels, sequential and patient. This is the
        # question that matters: not whether individual requests failed, but
        # whether the board is still there afterwards.
        await asyncio.sleep(3)
        if not await alive(base):
            print(f"\n*** board stopped answering after concurrency {level} ***")
            print("    that is the ceiling: a browser opening this many "
                  "parallel connections takes it down")
            return 2

    print("\nsurvived every level")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
