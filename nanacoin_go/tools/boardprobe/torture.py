#!/usr/bin/env python3
"""Torture the board the way a household actually would, only harder.

# Why this exists

Everything else in this directory tested one axis and the board survived all
of them: sequential reads (45 transactions), ten parallel connections, twelve
login/logout cycles. Then a person clicked around in a browser for under a
minute and killed it.

The gap was never load. It was *shape*. Three things a real client does that
none of those probes did:

1. **Preflights.** Any cross-origin `fetch` or XHR carrying `Authorization`
   or `Idempotency-Key` gets an `OPTIONS` first - neither header is
   CORS-safelisted, so the browser sends the preflight and waits for it before
   the real request. That is the browser's behaviour, not the client
   framework's, which is why simulating it on the wire is faithful rather than
   a shortcut: the board cannot tell the difference. It roughly doubles the
   request count, and every OPTIONS is a connection to accept, route and
   answer.

2. **Several people at once.** Four family members on four phones is four
   independent sessions, each with its own token, hitting the same global
   service lock.

3. **The expensive verbs.** Reads are cheap. Creating a user costs a PBKDF2
   hash and a permanent map entry; a purchase takes the lock, appends to the
   ledger and caches a ~700 byte response in the idempotency map. The earlier
   stress loop mostly issued and transferred - the two cheapest writes.

4. **The error paths.** A person gets things wrong: overspending, buying
   something already sold, sending to an account that does not exist, letting
   a token expire. Every one of those runs code the happy path never touches -
   `fmt.Errorf` allocating a message, `errors.As` walking an unwrap chain, the
   `writeError` mapping switch - and on a board with 20 kB spare a refusal
   that allocates more than a success is a real possibility. Nothing had
   tested a single failing request on hardware.

This does all four at once and reports the heap as it goes.

# What it does not do

Drive a real browser. That is a lot of machinery to reproduce something that
is fully described by "send an OPTIONS first and set these three headers".
The preflights here are real requests with real CORS headers, and the response
is checked for the `Access-Control-Allow-Origin` the browser would demand - so
a CORS regression fails this test even though nothing is rendering anything.

# Usage

    python torture.py                     # 4 users, 20 rounds
    python torture.py --users 4 --rounds 50
    python torture.py --no-preflight      # to isolate preflight cost
    python torture.py --setup             # (re)create the household first

Exits non-zero if the board stops answering, so CI can gate on it.
"""

import argparse
import asyncio
import base64
import hashlib
import json
import random
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

DEFAULT_IP = "192.168.1.158"
ORIGIN = "http://localhost:4200"

READ_TIMEOUT = 10.0
WRITE_TIMEOUT = 45.0
PREFLIGHT_TIMEOUT = 6.0

# Enough threads that `--users 4` really is four requests in flight, plus room
# for their preflights.
_pool = ThreadPoolExecutor(max_workers=24)

# Every member shares one PKCE verifier. Fine here: this is a load test, not a
# security test, and a fixed verifier keeps the login path cheap to express.
VERIFIER = "a" * 43
CHALLENGE = (
    base64.urlsafe_b64encode(hashlib.sha256(VERIFIER.encode()).digest())
    .decode()
    .rstrip("=")
)

HOUSEHOLD = [
    ("nana", "nana-pin", "Nana"),
    ("alice", "alice-pin", "Alice"),
    ("bob", "bob-pin", "Bob"),
    ("carol", "carol-pin", "Carol"),
]


# Text a household actually types, plus the text that breaks hand-written
# escapers.
#
# The codecs are hand-written now, so emoji, RTL scripts and control
# characters are the board's problem rather than a library's. These go into
# memos and listing descriptions so the encoder and parser meet them on real
# hardware with a nearly full heap - which is where an escaping bug stops
# being a wrong string and becomes a truncated response.
CHAOS_TEXT = [
    "chores",
    "for mowing the lawn",
    "nice work \U0001f389",
    "\U0001f389\U0001f382\U0001f370 party",
    "thumbs up \U0001f44d\U0001f3fd",
    "family \U0001f468\u200d\U0001f469\u200d\U0001f467\u200d\U0001f466",
    "\u65e5\u672c\u8a9e\u306e\u30e1\u30e2",
    "\ud55c\uad6d\uc5b4",
    "\u0645\u0631\u062d\u0628\u0627",
    "\u05e9\u05dc\u05d5\u05dd",
    "\u041f\u0440\u0438\u0432\u0435\u0442",
    "caf\u00e9 vs cafe\u0301",
    'say "hello"',
    "path\\to\\thing",
    '{"nested":"json"}',
    "[1,2,3]",
    "line one\nline two",
    "col\tcol",
    "bell\x07here",
    "x" * 139,
    "",
]


def chaos_text(rng):
    """A memo or description, weighted toward the ordinary."""
    if rng.random() < 0.55:
        return rng.choice(CHAOS_TEXT[:2])
    return rng.choice(CHAOS_TEXT)


class Stats:
    def __init__(self):
        self.ok = 0
        self.refused = 0      # board answered, said no (4xx) - correct behaviour
        self.busy = 0         # board answered 503 - backpressure, also correct
        self.error = 0        # board answered 5xx other than 503
        self.dead = 0         # no answer at all - the only bad outcome
        self.preflights = 0
        self.cors_missing = 0
        self.dead_why = {}
        self.by_op = {}
        self.slowest = 0.0
        self.slowest_op = ""

    def record(self, op, status, elapsed, had_cors=True, why=b""):
        d = self.by_op.setdefault(op, {"ok": 0, "refused": 0, "dead": 0})
        if status == 0:
            self.dead += 1
            d["dead"] += 1
            reason = why.decode("utf-8", "replace")[:24] or "unknown"
            self.dead_why[reason] = self.dead_why.get(reason, 0) + 1
        elif 200 <= status < 300:
            self.ok += 1
            d["ok"] += 1
        elif status == 503:
            # Backpressure, not failure. The board is explicitly saying "not
            # now" with a Retry-After, which is the correct behaviour when
            # every worker is busy - and being able to see it separately from
            # a crash is the whole point of answering rather than dropping
            # the connection.
            self.busy += 1
        elif status >= 500:
            self.error += 1
        else:
            self.refused += 1
            d["refused"] += 1
        if not had_cors:
            self.cors_missing += 1
        if elapsed > self.slowest:
            self.slowest, self.slowest_op = elapsed, op


def _blocking(base, method, path, body, token, idem, timeout, preflight):
    headers = {"Origin": ORIGIN}
    data = None
    if preflight:
        method = "OPTIONS"
        headers["Access-Control-Request-Method"] = preflight
        want = "authorization,content-type"
        if idem:
            want += ",idempotency-key"
        headers["Access-Control-Request-Headers"] = want
    else:
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        if token:
            headers["Authorization"] = f"Bearer {token}"
        if idem:
            headers["Idempotency-Key"] = idem
    req = urllib.request.Request(base + path, data=data, headers=headers,
                                 method=method)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, dict(r.headers), r.read()


async def raw(base, method, path, body=None, token=None, idem=None,
              timeout=None, preflight=None):
    """One request with a hard deadline. Never raises, never outlives it."""
    if timeout is None:
        timeout = PREFLIGHT_TIMEOUT if preflight else (
            WRITE_TIMEOUT if method != "GET" else READ_TIMEOUT
        )
    loop = asyncio.get_running_loop()
    try:
        return await asyncio.wait_for(
            loop.run_in_executor(_pool, _blocking, base, method, path, body,
                                 token, idem, timeout, preflight),
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
        return 0, {}, str(e).encode()[:60]


# Requests designed to be refused.
#
# Every one of these should produce a clean 4xx with a JSON body and correct
# CORS headers. A 5xx, a hang, or a missing Access-Control-Allow-Origin is a
# bug - and a refusal that costs more heap than a success is the kind of thing
# that turns "the user made a mistake" into "the board fell over".
#
# Shapes deliberately included: malformed JSON (the decoder's error path),
# unknown fields (DisallowUnknownFields), oversized bodies (MaxBytesReader and
# the board's own 1 kB limit), bad path segments, wrong methods, absent and
# corrupt tokens, and every domain refusal the ledger can produce.
ABUSE = [
    # (op, method, path, body, token_mode, idem)
    # token_mode: "good" | "none" | "garbage"
    ("abuse/overspend", "POST", "/api/v1/transfers",
     {"to": "%PEER%", "amount": 999999, "memo": "too much"}, "good", True),
    ("abuse/self-deal", "POST", "/api/v1/transfers",
     {"to": "%SELF%", "amount": 1, "memo": "to myself"}, "good", True),
    ("abuse/negative", "POST", "/api/v1/transfers",
     {"to": "%PEER%", "amount": -5, "memo": "negative"}, "good", True),
    ("abuse/zero", "POST", "/api/v1/transfers",
     {"to": "%PEER%", "amount": 0, "memo": "zero"}, "good", True),
    ("abuse/no-such-account", "POST", "/api/v1/transfers",
     {"to": "account-does-not-exist", "amount": 1, "memo": "ghost"},
     "good", True),
    ("abuse/long-memo", "POST", "/api/v1/transfers",
     {"to": "%PEER%", "amount": 1, "memo": "m" * 600}, "good", True),
    ("abuse/unknown-field", "POST", "/api/v1/transfers",
     {"to": "%PEER%", "amount": 1, "memo": "x", "sneaky": "field"},
     "good", True),
    ("abuse/huge-body", "POST", "/api/v1/listings",
     {"title": "big", "description": "z" * 4000, "price": 1}, "good", False),
    ("abuse/bad-price", "POST", "/api/v1/listings",
     {"title": "cheap", "description": "d", "price": -10}, "good", False),
    ("abuse/empty-title", "POST", "/api/v1/listings",
     {"title": "", "description": "d", "price": 1}, "good", False),
    ("abuse/no-token", "GET", "/api/v1/me", None, "none", False),
    ("abuse/bad-token", "GET", "/api/v1/me", None, "garbage", False),
    ("abuse/bad-token-write", "POST", "/api/v1/transfers",
     {"to": "%PEER%", "amount": 1, "memo": "x"}, "garbage", True),
    ("abuse/ghost-listing", "POST",
     "/api/v1/listings/listing-nope/purchase", None, "good", True),
    ("abuse/ghost-txn", "GET", "/api/v1/transactions/txn-999999", None,
     "good", False),
    ("abuse/bad-txn-id", "GET", "/api/v1/transactions/not-a-txn-id", None,
     "good", False),
    ("abuse/ghost-account", "GET",
     "/api/v1/accounts/account-nope/transactions", None, "good", False),
    ("abuse/wrong-method", "GET", "/api/v1/transfers", None, "good", False),
    ("abuse/admin-as-member", "POST", "/api/v1/admin/issue",
     {"to": "%SELF%", "amount": 100, "reason": "cheeky"}, "good", True),
    ("abuse/reprovision", "POST", "/api/v1/provision",
     {"username": "usurper", "display_name": "U", "password": "pw12",
      "household_name": "Mine Now"}, "none", False),
    ("abuse/huge-limit", "GET", "/api/v1/transactions?limit=99999", None,
     "good", False),
    ("abuse/negative-limit", "GET", "/api/v1/transactions?limit=-5", None,
     "good", False),
    ("abuse/junk-limit", "GET", "/api/v1/transactions?limit=abc", None,
     "good", False),
    ("abuse/dupe-user", "POST", "/api/v1/users",
     {"username": "alice", "display_name": "Imposter", "password": "pw12"},
     "good", False),
    ("abuse/short-password", "POST", "/api/v1/users",
     {"username": "tiny", "display_name": "T", "password": "a"},
     "good", False),
]

# Sent as a raw body rather than JSON, to exercise the decoder's own failure
# path rather than the validators behind it.
MALFORMED_BODIES = [
    ("abuse/malformed-json", b'{"to": "x", "amount":'),
    ("abuse/not-json", b"this is not json at all"),
    ("abuse/empty-body", b""),
    ("abuse/json-array", b'[1,2,3]'),
    ("abuse/deep-nest", b'{"a":' * 60 + b'1' + b'}' * 60),
]



class Client:
    """One household member, behaving like their browser tab."""

    def __init__(self, base, username, password, stats, do_preflight=True):
        self.base = base
        self.username = username
        self.password = password
        self.stats = stats
        self.do_preflight = do_preflight
        self.token = None
        self.account = None

    async def call(self, method, path, body=None, idem=None, op=None):
        """A request as a browser would make it: preflight, then the real one.

        The preflight is checked for the CORS headers the browser requires. A
        200 on the real request is worthless if the browser would have
        discarded it - that failure has cost this project a long debugging
        session before, so it is asserted rather than assumed.
        """
        op = op or f"{method} {path.split('?')[0]}"

        needs_preflight = self.do_preflight and (
            self.token is not None or body is not None or idem is not None
        )
        if needs_preflight:
            t0 = time.monotonic()
            st, h, pfBody = await raw(self.base, method, path, token=self.token,
                                      idem=idem, preflight=method)
            self.stats.preflights += 1
            had_cors = bool(h.get("Access-Control-Allow-Origin"))
            self.stats.record("OPTIONS", st, time.monotonic() - t0, had_cors, pfBody)
            if st == 0:
                return 0, {}, b"preflight-timeout"
            # A browser stops here if the preflight was not allowed.
            if not had_cors:
                return st, h, b"preflight-no-cors"

        t0 = time.monotonic()
        st, h, body_out = await raw(self.base, method, path, body=body,
                                    token=self.token, idem=idem)
        had_cors = bool(h.get("Access-Control-Allow-Origin")) or st == 0
        self.stats.record(op, st, time.monotonic() - t0, had_cors, body_out)
        return st, h, body_out

    async def login(self):
        st, _, body = await self.call(
            "POST", "/api/v1/auth/authorize",
            {"username": self.username, "password": self.password,
             "code_challenge": CHALLENGE, "code_challenge_method": "S256",
             "redirect_uri": f"{ORIGIN}/cb"},
            op="login/authorize")
        if st != 200:
            return False
        try:
            code = json.loads(body)["code"]
        except Exception:
            return False

        st, _, body = await self.call(
            "POST", "/api/v1/auth/token",
            {"code": code, "code_verifier": VERIFIER,
             "redirect_uri": f"{ORIGIN}/cb"},
            op="login/token")
        if st != 200:
            return False
        try:
            data = json.loads(body)
        except Exception:
            return False
        self.token = data["access_token"]
        self.account = data.get("user", {}).get("account")
        return True

    async def logout(self):
        await self.call("POST", "/api/v1/auth/logout", {}, op="logout")
        self.token = None

    async def browse(self):
        """What loading the app costs: several reads, each preflighted."""
        for path, op in (
            ("/api/v1/me", "read/me"),
            ("/api/v1/users", "read/users"),
            ("/api/v1/listings", "read/listings"),
            ("/api/v1/transactions?limit=30", "read/ledger"),
        ):
            await self.call("GET", path, op=op)

    async def history(self):
        if self.account:
            await self.call(
                "GET", f"/api/v1/accounts/{self.account}/transactions?limit=30",
                op="read/history")

    async def transfer(self, to_account, n, memo=None):
        if not to_account or to_account == self.account:
            return
        await self.call("POST", "/api/v1/transfers",
                        {"to": to_account, "amount": 1,
                         "memo": memo if memo is not None else f"t{n}"},
                        idem=f"{self.username}-xfer-{n}", op="write/transfer")

    async def create_listing(self, n, title=None, description=None):
        await self.call("POST", "/api/v1/listings",
                        {"title": title if title is not None else f"{self.username} item {n}",
                         "description": description if description is not None else "x" * 120,
                         "price": 2},
                        op="write/listing")

    async def buy_something(self, n):
        """The heaviest path: lock, ledger append, idempotency cache."""
        st, _, body = await self.call("GET", "/api/v1/listings?status=ACTIVE",
                                      op="read/market")
        if st != 200:
            return
        try:
            listings = json.loads(body).get("listings") or []
        except Exception:
            return
        mine = [l for l in listings if l.get("seller") != self.account]
        if not mine:
            return
        target = random.choice(mine)
        await self.call("POST", f"/api/v1/listings/{target['id']}/purchase",
                        idem=f"{self.username}-buy-{n}", op="write/purchase")

    async def raw_body(self, op, path, payload):
        """POST a body that is not valid JSON, to hit the decoder itself."""
        loop = asyncio.get_running_loop()

        def go():
            headers = {"Origin": ORIGIN, "Content-Type": "application/json"}
            if self.token:
                headers["Authorization"] = f"Bearer {self.token}"
            req = urllib.request.Request(self.base + path, data=payload,
                                         headers=headers, method="POST")
            with urllib.request.urlopen(req, timeout=WRITE_TIMEOUT) as r:
                return r.status, dict(r.headers)

        t0 = time.monotonic()
        try:
            st, h = await asyncio.wait_for(
                loop.run_in_executor(_pool, go), timeout=WRITE_TIMEOUT + 0.5)
        except asyncio.TimeoutError:
            st, h = 0, {}
        except urllib.error.HTTPError as e:
            st, h = e.code, dict(e.headers)
        except Exception:
            st, h = 0, {}
        had_cors = bool(h.get("Access-Control-Allow-Origin")) or st == 0
        self.stats.record(op, st, time.monotonic() - t0, had_cors)

    async def abuse(self, peers, rnd):
        """Do things wrong, on purpose.

        Each of these must come back as a clean 4xx with CORS headers. The
        assertion is in the summary: any 5xx, any no-answer, or any missing
        Access-Control-Allow-Origin is reported separately from the refusals,
        because a refusal is correct behaviour and the others are not.
        """
        peer = next((p.account for p in peers if p is not self and p.account),
                    None)
        saved = self.token

        for op, method, path, body, token_mode, idem in ABUSE:
            b = body
            if b is not None:
                b = json.loads(
                    json.dumps(b)
                    .replace("%PEER%", peer or "account-none")
                    .replace("%SELF%", self.account or "account-none")
                )
            if token_mode == "none":
                self.token = None
            elif token_mode == "garbage":
                self.token = "not-a-real-token-" + str(rnd)
            else:
                self.token = saved

            await self.call(method, path, b,
                            idem=f"{self.username}-{op}-{rnd}" if idem else None,
                            op=op)

        self.token = saved
        for op, payload in MALFORMED_BODIES:
            await self.raw_body(op, "/api/v1/transfers", payload)

    async def admin_issue(self, to_account, n):
        if not to_account:
            return
        await self.call("POST", "/api/v1/admin/issue",
                        {"to": to_account, "amount": 20, "reason": f"pay {n}"},
                        idem=f"nana-issue-{n}", op="write/issue")

    async def session(self, rnd, peers, is_nana, rng=None, intensity=1):
        """One full visit: log in, look around, do things, leave.

        Deliberately the whole arc rather than a single request. The board
        survived isolated operations and died on a person's session, so the
        session is the unit worth testing.

        With rng set, the shape is randomised: how many of each operation, in
        what order, with what text. A fixed script exercises one path through
        the state machine very well and every other path not at all - and the
        failures on this board have all come from combinations nobody
        scripted.
        """
        if not await self.login():
            return
        await self.browse()

        peer_accounts = [p.account for p in peers if p.account and p is not self]

        if rng is None:
            # Deterministic shape, as before.
            if is_nana:
                for p in peer_accounts[:2]:
                    await self.admin_issue(p, rnd)
            else:
                await self.transfer(rng_choice(None, peer_accounts), rnd)
            if rnd % 2 == 0:
                await self.create_listing(rnd)
            if rnd % 3 == 0:
                await self.buy_something(rnd)
            await self.abuse(peers, rnd)
            await self.history()
            await self.logout()
            return

        # Chaotic shape: a random bag of operations, shuffled.
        ops = []
        if is_nana:
            ops += [("issue", p) for p in peer_accounts[: rng.randint(1, len(peer_accounts) or 1)]]
        ops += [("transfer", None)] * rng.randint(0, 3 * intensity)
        ops += [("listing", None)] * rng.randint(0, 2 * intensity)
        ops += [("buy", None)] * rng.randint(0, 2 * intensity)
        ops += [("browse", None)] * rng.randint(0, 2 * intensity)
        ops += [("history", None)] * rng.randint(0, 2 * intensity)
        # Abuse is common on purpose: a person gets things wrong constantly,
        # and the error paths allocate differently from the happy ones.
        ops += [("abuse", None)] * rng.randint(1, max(1, intensity))
        rng.shuffle(ops)

        for i, (op, arg) in enumerate(ops):
            tag = f"{rnd}-{i}-{rng.randint(0, 1 << 20)}"
            if op == "issue":
                await self.admin_issue(arg, tag)
            elif op == "transfer":
                if peer_accounts:
                    await self.transfer(rng.choice(peer_accounts), tag,
                                        memo=chaos_text(rng))
            elif op == "listing":
                await self.create_listing(tag, title=chaos_text(rng),
                                          description=chaos_text(rng))
            elif op == "buy":
                await self.buy_something(tag)
            elif op == "browse":
                await self.browse()
            elif op == "history":
                await self.history()
            elif op == "abuse":
                await self.abuse(peers, tag)

        # Most people log out; some just close the tab, leaving the session
        # live. Both have to be survivable.
        if rng.random() < 0.8:
            await self.logout()
        else:
            self.token = None


def rng_choice(rng, seq):
    if not seq:
        return None
    if rng is None:
        return seq[0]
    return rng.choice(seq)


async def setup(base, stats):
    """Provision the household. Tolerates an already-provisioned board."""
    st, _, _ = await raw(base, "POST", "/api/v1/provision",
                         {"username": "nana", "display_name": "Nana",
                          "password": "nana-pin",
                          "household_name": "Torture House"})
    print(f"provision: {st}" + (" (already provisioned)" if st == 403 else ""))

    nana = Client(base, "nana", "nana-pin", stats)
    if not await nana.login():
        # A board provisioned by an earlier run under a different password is
        # the common case here, and it is not a board failure - say which it
        # is rather than reporting the board as down.
        if await alive(base):
            print("board is up but 'nana/nana-pin' does not log in - it was "
                  "provisioned with different credentials. Reflash, or run "
                  "without --setup if the household already exists.")
        else:
            print("cannot log in as nana - board is not answering")
        return None
    for username, password, display in HOUSEHOLD[1:]:
        st, _, _ = await nana.call("POST", "/api/v1/users",
                                   {"username": username,
                                    "display_name": display,
                                    "password": password}, op="setup/user")
        note = " (already exists)" if st == 409 else ""
        print(f"  user {username}: {st}{note}")
    await nana.logout()
    return True


async def alive(base, tries=3):
    for _ in range(tries):
        st, _, _ = await raw(base, "GET", "/api/v1/status", timeout=5.0)
        if st == 200:
            return True
        await asyncio.sleep(2)
    return False


async def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ip", default=DEFAULT_IP)
    ap.add_argument("--users", type=int, default=4,
                    help="concurrent household members (max 4)")
    ap.add_argument("--rounds", type=int, default=20)
    ap.add_argument("--setup", action="store_true",
                    help="provision the household first")
    ap.add_argument("--no-preflight", action="store_true",
                    help="skip OPTIONS, to isolate what preflights cost")
    ap.add_argument("--chaos", action="store_true",
                    help="randomise session shape, order and text")
    ap.add_argument("--intensity", type=int, default=1,
                    help="multiplier on operations per session (chaos mode)")
    ap.add_argument("--seed", type=int, default=0,
                    help="RNG seed; 0 picks one and prints it, so a failing "
                         "run can be replayed exactly")
    ap.add_argument("--give-up-after", type=int, default=2,
                    help="stop after this many rounds with no answers at all")
    args = ap.parse_args()

    base = f"http://{args.ip}"
    stats = Stats()

    if not await alive(base):
        print("board is not answering before we start - reset it first")
        return 1

    if args.setup:
        if not await setup(base, stats):
            return 1
        print()

    members = HOUSEHOLD[: max(1, min(args.users, len(HOUSEHOLD)))]
    clients = [Client(base, u, p, stats, not args.no_preflight)
               for u, p, _ in members]

    # One login up front so each client knows its own account id, which the
    # transfer and purchase logic needs.
    for c in clients:
        await c.login()
        await c.call("GET", "/api/v1/me", op="setup/me")
        if c.account is None:
            st, _, body = await c.call("GET", "/api/v1/me", op="setup/me")
            try:
                c.account = json.loads(body)["account"]
            except Exception:
                pass
        await c.logout()

    # Chaos mode: one master RNG, seeded and printed, so a run that breaks
    # the board can be replayed exactly. A random load test nobody can repeat
    # is a rumour, not a finding.
    rng = None
    if args.chaos:
        seed = args.seed or random.randrange(1 << 30)
        rng = random.Random(seed)
        print(f"chaos mode, seed {seed} (replay with --seed {seed})")

    print(f"{len(clients)} concurrent members, {args.rounds} rounds, "
          f"intensity {args.intensity}, "
          f"preflights {'off' if args.no_preflight else 'on'}\n", flush=True)

    dead_rounds = 0
    t_start = time.monotonic()

    for rnd in range(1, args.rounds + 1):
        before = stats.ok + stats.refused + stats.dead
        t0 = time.monotonic()

        # The whole point: every member's session runs at the same time.
        # Each client gets its own RNG stream, seeded from the master, so a
        # run is reproducible even though the clients interleave
        # unpredictably.
        await asyncio.gather(*[
            c.session(rnd, clients, c.username == "nana",
                      rng=random.Random(rng.randrange(1 << 30)) if rng else None,
                      intensity=args.intensity)
            for c in clients
        ])

        elapsed = time.monotonic() - t0
        done = stats.ok + stats.refused + stats.dead - before

        st, h, body = await raw(base, "GET", "/api/v1/status", timeout=5.0)
        health = h.get("X-Nanacoin-Health", "")
        txns = "?"
        if st == 200:
            try:
                txns = json.loads(body).get("transactions", "?")
            except Exception:
                pass

        print(f"r{rnd:<3} tx={txns:<5} reqs={done:<4} {elapsed:4.1f}s  "
              f"busy={stats.busy:<4} dead={stats.dead:<4} {health}", flush=True)

        if st != 200:
            dead_rounds += 1
            if dead_rounds >= args.give_up_after:
                print(f"\n*** board stopped answering at round {rnd} ***")
                break
        else:
            dead_rounds = 0

    total_s = time.monotonic() - t_start
    print()
    print("=" * 62)
    print(f"ok={stats.ok}  refused(4xx)={stats.refused}  "
          f"busy(503)={stats.busy}  server-error(5xx)={stats.error}  "
          f"no-answer={stats.dead}")
    if stats.dead_why:
        print("no-answer reasons: " + ", ".join(
            f"{k}={v}" for k, v in sorted(stats.dead_why.items(),
                                          key=lambda kv: -kv[1])))
    print(f"preflights sent: {stats.preflights}")
    print(f"responses missing CORS headers: {stats.cors_missing}")
    print(f"slowest: {stats.slowest:.1f}s on {stats.slowest_op}")
    print(f"wall clock: {total_s:.0f}s")
    print()
    print(f"{'operation':<24} {'ok':>6} {'refused':>8} {'dead':>6}")
    for op in sorted(stats.by_op):
        d = stats.by_op[op]
        print(f"{op:<24} {d['ok']:>6} {d['refused']:>8} {d['dead']:>6}")

    up = await alive(base)
    print()
    print("board is UP" if up else "board is DOWN")
    if stats.error:
        print(f"NOTE: {stats.error} server errors (5xx) - the board answered "
              f"but something inside failed")
    if stats.cors_missing:
        print(f"NOTE: {stats.cors_missing} responses a browser would have "
              f"discarded for missing Access-Control-Allow-Origin")
    return 0 if up and not stats.error else 2


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
