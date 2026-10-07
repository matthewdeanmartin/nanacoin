"""Black-box NanaCoin JSON v1 contracts; Python 3.10+, standard library only."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import base64
import hashlib
import http.server
import json
import os
from pathlib import Path
import random
import secrets
import socket
import subprocess
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request

SERVER = None
SERVER_ARGS = []
RESTART_SERVER = None
CLOCK = False
CLOCK_START = None
# Loopback traffic must stay local even on machines with a configured proxy.
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


class BankContract(unittest.TestCase):
    def setUp(self):
        if self._testMethodName.startswith("test_clock_") and not CLOCK:
            self.skipTest("requires the launcher clock (--clock)")
        self.temp = tempfile.TemporaryDirectory(prefix="nanacoin-conformance-")
        self.addCleanup(self.temp.cleanup)
        self.process = None
        self.addCleanup(self.stop)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            self.port = sock.getsockname()[1]
        self.base = f"http://127.0.0.1:{self.port}/api/v1"
        self.clock_path = Path(self.temp.name) / "clock.txt"
        self.clock_path.write_text(str(CLOCK_START if CLOCK_START is not None else int(time.time())), encoding="ascii")
        self.start()
        self.assertFalse(self.get("/status")["provisioned"])
        self.provision = dict(household_name="Contract home", username="nana",
                              display_name="Nana", password="1234")
        self.call("/provision", self.provision, expected=201)
        self.nana = self.login("nana", "1234")
        self.users = {}
        for name in ("alice", "bob"):
            user = self.call("/users", dict(username=name, display_name=name.title(),
                             password="5678", grant=False), self.nana, expected=201)
            self.users[name] = user
        self.alice = self.login("alice", "5678")
        self.bob = self.login("bob", "5678")
        # Public IDs are discovered rather than inferred from journal sequences.
        self.accounts = {u["username"]: u["account"] for u in self.get("/users", self.nana)["users"]}

    def start(self, executable=None):
        env = dict(os.environ, NANACOIN_PORT=str(self.port),
                   NANACOIN_JOURNAL=str(Path(self.temp.name) / "bank.journal"),
                   NANACOIN_ORIGINS="http://localhost:4200",
                   NANACOIN_MINICLOUD_URL=getattr(self, "screen_url", "disabled-for-conformance"))
        self.log = open(Path(self.temp.name) / "server.log", "ab")
        clock_args = ["--conformance-clock-file", str(self.clock_path)] if CLOCK else []
        self.process = subprocess.Popen([executable or SERVER, *SERVER_ARGS, *clock_args], env=env,
            stdout=self.log, stderr=self.log,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                self.fail("Server exited: " + (Path(self.temp.name) / "server.log").read_text(errors="replace"))
            try:
                if self.request("/status")[0] == 200:
                    return
            except (OSError, urllib.error.URLError):
                time.sleep(.05)
        self.fail("Server startup timed out")

    def stop(self):
        if self.process is not None:
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=5)
            self.process = None
        if getattr(self, "log", None) is not None:
            self.log.close()

    def request(self, path, payload=None, token="", key="", method=None, raw=None):
        headers = {"Origin": "http://localhost:4200"}
        if token:
            headers["Authorization"] = "Bearer " + token
        if key:
            headers["Idempotency-Key"] = key
        data = raw if raw is not None else (json.dumps(payload, ensure_ascii=False).encode("utf-8") if payload is not None else None)
        if data is not None:
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(self.base + path, data=data, headers=headers, method=method)
        try:
            response = HTTP.open(req, timeout=10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            body = response.read()
            if body:
                self.assertEqual(response.headers.get_content_type(), "application/json")
            return response.status, json.loads(body) if body else None

    def call(self, path, payload=None, token="", key="", expected=200, method=None, raw=None):
        status, body = self.request(path, payload, token, key, method, raw)
        self.assertEqual(status, expected, f"{path}: {body}")
        if expected >= 400:
            self.assertIsInstance(body, dict)
            self.assertIsInstance(body.get("error"), str)
            self.assertIsInstance(body.get("message"), str)
        return body

    def get(self, path, token=""):
        return self.call(path, token=token)

    def restart(self):
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.nana = self.login("nana", "1234")
        self.alice = self.login("alice", "5678")
        self.bob = self.login("bob", "5678")

    def wait_for(self, read, predicate, timeout=10):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = read()
            if predicate(value):
                return value
            time.sleep(.2)
        self.fail(f"Public state did not reach expected condition: {value}")

    def advance(self, seconds):
        self.assertTrue(CLOCK, "Use --clock with a conformance-clock server build")
        instant = int(self.clock_path.read_text(encoding="ascii")) + seconds
        temporary = self.clock_path.with_suffix(".next")
        temporary.write_text(str(instant), encoding="ascii")
        os.replace(temporary, self.clock_path)
        # Renew credentials after jumping past their normal session lifetime.
        self.nana = self.login("nana", "1234")
        self.alice = self.login("alice", "5678")
        self.bob = self.login("bob", "5678")

    def now(self):
        return int(self.clock_path.read_text(encoding="ascii")) if CLOCK else int(time.time())

    def new_user(self, name):
        user = self.call("/users", dict(username=name, display_name=name.title(),
            password="5678", grant=False), self.nana, expected=201)
        self.users[name] = user
        self.accounts[name] = user["account"]
        return self.login(name, "5678")

    def listing(self, token, side="SELL", price=30, **fields):
        return self.call("/listings", dict(title="Contract listing", description="Private description",
            side=side, price=price, **fields), token, expected=201)

    def commerce(self, action, token, key, expected=200):
        return self.call("/commerce/commands", action, token, key, expected=expected)

    def artwork(self, art_id):
        return next(a for a in self.get("/commerce", self.nana)["artworks"] if a["id"] == art_id)

    def request_view(self, request_id):
        return next(r for r in self.get("/commerce", self.nana)["requests"] if r["id"] == request_id)

    def loan_terms(self, borrower="bob", **changes):
        return dict(dict(borrower=self.accounts[borrower], amount=40, rate_bps=0,
            rate_days=365, payment_days=30, installment=10, credit=False, memo="Private loan note"), **changes)

    def authorize(self, username, password):
        verifier = secrets.token_urlsafe(32)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
        auth = self.call("/auth/authorize", dict(username=username, password=password,
            code_challenge=challenge, code_challenge_method="S256", redirect_uri="http://localhost:4200/"))
        return dict(code=auth["code"], code_verifier=verifier, redirect_uri="http://localhost:4200/")

    def login(self, username, password):
        grant = self.call("/auth/token", self.authorize(username, password))
        self.assertEqual(grant["token_type"], "Bearer")
        self.assertEqual(grant["expires_in"], 28800)
        self.assertEqual(grant["user"]["username"], username)
        return grant["access_token"]

    def balance(self, name):
        return self.get("/accounts/" + self.accounts[name], self.nana)["balance"]

    def issue(self, name, amount, key="seed"):
        return self.call("/admin/issue", dict(to=self.accounts[name], amount=amount, reason="Contract 🍪"),
                         self.nana, key, expected=201)

    def transfer(self, token, to, amount, key, expected=201, memo="Contract payment"):
        response = self.call("/transfers", dict(to=self.accounts[to], amount=amount, memo=memo),
                             token, key, expected=expected)
        if expected == 409:
            self.assertEqual(response["error"], "insufficient_funds")
        return response

    def test_provision_auth_and_public_routes(self):
        self.assertTrue(self.get("/status")["provisioned"])
        self.call("/provision", self.provision, expected=403)
        self.call("/me", expected=401)
        self.call("/me", token="invalid", expected=401)
        self.call("/unknown-contract-route", token=self.alice, expected=404)
        self.call("/auth/logout", {}, self.alice, expected=204)
        self.call("/me", token=self.alice, expected=401)

    def test_pkce_codes_are_one_use_even_after_bad_verifier(self):
        grant = self.authorize("alice", "5678")
        self.call("/auth/token", grant)
        self.call("/auth/token", grant, expected=401)
        grant = self.authorize("alice", "5678")
        wrong = dict(grant, code_verifier=secrets.token_urlsafe(32))
        self.call("/auth/token", wrong, expected=401)
        self.call("/auth/token", grant, expected=401)

    def test_permissions_and_last_nana(self):
        self.call("/state", token=self.alice, expected=403)
        self.call("/admin/issue", dict(to=self.accounts["bob"], amount=1, reason="No"), self.alice, "no", expected=403)
        uid = self.get("/me", self.nana)["id"]
        self.call("/users/" + uid, {"status": "DISABLED"}, self.nana, method="PATCH", expected=403)
        self.assertEqual(self.get("/me", self.nana)["role"], "nana")
        self.assertEqual(self.balance("bob"), 0)

    def test_member_creation_text_duplicates_and_atomic_failure(self):
        original = self.get("/users", self.nana)["users"]
        for field in ("username", "display_name"):
            for value in (("", "\u3000", "bad\nname") if field == "username" else ("bad\nname",)):
                payload = dict(username="charlie", display_name="Charlie", password="5678", grant=False)
                payload[field] = value
                self.call("/users", payload, self.nana, expected=400)
        self.call("/users", dict(username="alice", display_name="Different", password="5678", grant=False), self.nana, expected=409)
        for value in ("alice", "@alice", "alice@localhost", "alice@@example.com", "alice@example.com/path", "alice@exam ple.com"):
            self.call("/users", dict(username="charlie", display_name="Charlie", password="5678", grant=False, mastodon_id=value), self.nana, expected=400)
        self.assertEqual(self.get("/users", self.nana)["users"], original)
        fallback = self.call("/users", dict(username="fallback", display_name="\u3000", password="5678", grant=False), self.nana, expected=201)
        self.assertEqual(fallback["display_name"], "fallback")
        self.call("/users", dict(username="charlie", display_name="Charlie", password="5678", grant=False, mastodon_id="@charlie@example.com"), self.nana, expected=201)
        self.restart()
        self.assertEqual(len(self.get("/users", self.nana)["users"]), 5)
        self.assertEqual(self.get("/me", self.login("charlie", "5678"))["mastodon_id"], "@charlie@example.com")

    def test_member_capacity_and_duplicate_error_precedence(self):
        for index in range(29):
            self.call("/users", dict(username=f"member{index}", display_name=f"Member {index}", password="5678", grant=False), self.nana, expected=201)
        self.assertEqual(len(self.get("/users", self.nana)["users"]), 32)
        for username in ("overflow", "alice"):
            rejected = self.call("/users", dict(username=username, display_name="Capacity", password="5678", grant=False), self.nana, expected=507)
            self.assertEqual(rejected["error"], "capacity")
        self.call("/users", dict(username=" ", display_name="Capacity", password="5678", grant=False), self.nana, expected=400)
        self.restart()
        self.assertEqual(len(self.get("/users", self.nana)["users"]), 32)
        self.assertEqual(self.get("/me", self.login("member28", "5678"))["username"], "member28")

    def test_profile_text_and_self_permission_boundaries(self):
        path = "/users/" + self.users["alice"]["id"]
        for payload in ({"display_name": "\u3000"}, {"display_name": "bad\nname"}, {"bio": "bad\ntext"}, {"mastodon_id": "alice@localhost"}):
            self.call(path, payload, self.alice, method="PATCH", expected=400)
        for payload in ({"role": "nana"}, {"status": "DISABLED"}):
            self.call(path, payload, self.alice, method="PATCH", expected=403)
        self.call("/users/" + self.users["bob"]["id"], {"display_name": "Changed"}, self.alice, method="PATCH", expected=403)
        self.call(path, {"display_name": "Alice Updated", "bio": "Hello world", "mastodon_id": "alice@example.com"}, self.alice, method="PATCH")
        self.restart()
        profile = self.get("/me", self.alice)
        self.assertEqual(profile["display_name"], "Alice Updated")
        self.assertEqual(profile["bio"], "Hello world")
        self.assertEqual(profile["mastodon_id"], "alice@example.com")

    def test_issue_retire_transfer_and_reversal(self):
        self.issue("alice", 100)
        tx = self.transfer(self.alice, "bob", 30, "pay")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        self.call("/admin/retire", {"from": self.accounts["bob"], "amount": 10, "reason": "Retire"}, self.nana, "retire", expected=201)
        self.call("/transactions/" + tx["id"] + "/reverse", {"reason": "Correction"}, self.nana, "undo", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, -10))

    def test_invalid_money_does_not_mutate_balances(self):
        self.issue("alice", 50)
        for index, amount in enumerate((-1, 1_000_000_000_000_001, 1.5)):
            self.transfer(self.alice, "bob", amount, f"bad-{index}", expected=400)
        self.transfer(self.alice, "alice", 1, "self", expected=400)
        self.transfer(self.alice, "bob", 51, "poor", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (50, 0))

    def test_idempotency_conflict_and_intervening_commands(self):
        first = self.issue("alice", 100, "stable")
        self.transfer(self.alice, "bob", 20, "payment")
        self.assertEqual(self.issue("alice", 100, "stable"), first)
        self.call("/admin/issue", dict(to=self.accounts["alice"], amount=101, reason="Contract 🍪"), self.nana, "stable", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_restart_recovers_money_and_receipts_but_not_sessions(self):
        first = self.issue("alice", 100, "durable")
        payment = self.transfer(self.alice, "bob", 20, "durable-payment")
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.call("/me", token=self.nana, expected=401)
        self.nana = self.login("nana", "1234")
        self.alice = self.login("alice", "5678")
        self.assertEqual(self.issue("alice", 100, "durable"), first)
        self.assertEqual(self.transfer(self.alice, "bob", 20, "durable-payment"), payment)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_seeded_payment_sequences_against_client_model(self):
        self.issue("alice", 1000, "seed-a")
        self.issue("bob", 1000, "seed-b")
        model = {"alice": 1000, "bob": 1000}
        rng = random.Random(20261006)
        for step in range(40):
            sender, recipient = rng.sample(list(model), 2)
            amount = rng.randint(1, 1200)
            success = model[sender] >= amount
            self.transfer(getattr(self, sender), recipient, amount, f"model-{step}", expected=201 if success else 409)
            if success:
                model[sender] -= amount
                model[recipient] += amount
            self.assertEqual({name: self.balance(name) for name in model}, model)

    def test_account_currency_views_disabled_targets_and_cross_build_restart(self):
        self.issue("alice", 120)
        self.call("/admin/issue-usd", dict(to=self.accounts["alice"], cents=45,
            reason="Account view cash"), self.nana, "view-usd", expected=201)
        nc = self.get("/accounts/" + self.accounts["alice"], self.bob)
        usd = self.get("/accounts/" + self.accounts["alice"] + "-usd", self.bob)
        self.assertEqual((nc["balance"], usd["balance"]), (120, 45))
        self.assertEqual((nc["name"], usd["name"]), ("Alice", "Alice"))
        self.assertEqual((nc["user_id"], usd["user_id"]), (self.users["alice"]["id"], self.users["alice"]["id"]))
        for account, expected in (("account:system-issuance", -120), ("account:usd-issuance", -45)):
            row = self.get("/accounts/" + account, self.bob)
            self.assertEqual((row["id"], row["balance"], row["name"], row["status"], row["user_id"]),
                (account, expected, "Issuance", "ACTIVE", ""))
        self.call("/accounts/account-32", token=self.bob, expected=404)
        self.call("/accounts/account-32-usd", token=self.bob, expected=404)
        self.call("/users/" + self.users["alice"]["id"], {"status": "DISABLED"}, self.nana, method="PATCH")
        disabled_nc = self.get("/accounts/" + self.accounts["alice"], self.bob)
        disabled_usd = self.get("/accounts/" + self.accounts["alice"] + "-usd", self.bob)
        self.assertEqual((disabled_nc["status"], disabled_usd["status"]), ("DISABLED", "DISABLED"))
        self.assertEqual((disabled_nc["balance"], disabled_usd["balance"]), (120, 45))
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.bob = self.login("bob", "5678")
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"], self.bob), disabled_nc)
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.bob), disabled_usd)
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_household_statistics_delivered_over_http_after_cross_build_restart(self):
        records = []
        class Screen(http.server.BaseHTTPRequestHandler):
            def do_POST(handler):
                size = int(handler.headers["Content-Length"])
                records.append((handler.path, json.loads(handler.rfile.read(size))))
                handler.send_response(202)
                handler.send_header("Content-Length", "0")
                handler.end_headers()
            def log_message(handler, *_args):
                pass
        receiver = http.server.HTTPServer(("127.0.0.1", 0), Screen)
        thread = threading.Thread(target=receiver.serve_forever, daemon=True)
        thread.start()
        def close_receiver():
            self.stop()
            receiver.shutdown()
            receiver.server_close()
            thread.join(timeout=2)
        self.addCleanup(close_receiver)
        self.issue("alice", 50000)
        for index, price in enumerate((100, 200, 300)):
            row = self.call("/listings", dict(title="Stats bread", price=price, side="SELL",
                economic_kind="GOOD", quantity="1", unit="EACH"), self.bob, expected=201)
            self.call("/listings/" + row["id"] + "/purchase", {}, self.alice,
                f"stats-sale-{index}", expected=201)
        self.call("/transfers", dict(to=self.accounts["bob"], amount=100, memo="Stats work",
            economic_kind="LABOR", quantity="1", unit="HOUR"), self.alice, "stats-labor", expected=201)
        loan = self.call("/loans", self.loan_terms(amount=1000, installment=100, rate_bps=500), self.alice, "stats-loan")
        self.call("/loans/" + str(loan["id"]) + "/accept", {}, self.bob, "stats-fund")
        self.call("/admin/issue-usd", dict(to=self.accounts["bob"], cents=1000, reason="Stats cash"),
            self.nana, "stats-cash", expected=201)
        quote = self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=201)
        self.call("/quotes/" + quote["id"] + "/take", {}, self.bob, "stats-exchange", expected=201)
        balances = (self.balance("alice"), self.balance("bob"))
        self.screen_url = f"http://127.0.0.1:{receiver.server_address[1]}"
        self.restart()
        expected = {"Retained year\nInflation: 50.0%\nEmployment: 50% (1/2)",
                    "Interest: 5.00%/yr\nExchange: $2.50/NC"}
        delivered = self.wait_for(lambda: list(records), lambda rows: expected <= {
            body["text"] for _, body in rows if body["recipient"] == "Household economy"}, timeout=45)
        for path, body in delivered:
            if body["text"] in expected:
                self.assertEqual(path, "/api/screen/notify")
                self.assertEqual((body["source"], body["size"]), ("nanacoin", "medium"))
                self.assertTrue(body["id"].startswith("stats-"))
                self.assertGreater(body["expires_at"], self.now())
        self.assertEqual((self.balance("alice"), self.balance("bob")), balances)
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_private_messages_and_public_money(self):
        secret = self.transfer(self.alice, "bob", 0, "message", memo="Private note")
        self.call("/transactions/" + secret["id"], token=self.nana, expected=404)
        page = self.get("/accounts/" + self.accounts["bob"] + "/transactions", self.bob)
        self.assertEqual(next(t for t in page["transactions"] if t["id"] == secret["id"])["description"], "Private note")
        public = self.get("/transactions")
        self.assertNotIn(secret["id"], [t["id"] for t in public["transactions"]])
        outsider = self.get("/accounts/" + self.accounts["bob"] + "/transactions", self.nana)
        self.assertNotIn(secret["id"], [t["id"] for t in outsider["transactions"]])
        feed = self.get("/activity?after=0&limit=50", self.bob)
        self.assertNotIn("Private note", json.dumps(feed))
        for user in self.get("/users", self.bob)["users"]:
            self.assertNotIn("password", user)
            self.assertNotIn("token_hash", user)

    def test_activity_public_events_hide_private_changes_and_notes(self):
        after = self.get("/activity", self.alice)["sequence"]
        self.issue("alice", 100)
        listing = self.call("/listings", dict(title="Public bread", description="PRIVATE listing description", side="SELL", price=20), self.bob, expected=201)
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20, "message": "PRIVATE offer note"}, self.alice, expected=201)
        self.call("/offers/" + offer["id"] + "/accept", {}, self.bob, "feed-accept", expected=201)
        self.transfer(self.alice, "bob", 0, "feed-message", memo="PRIVATE message")
        self.call("/users/" + self.users["alice"]["id"], {"bio": "PRIVATE biography"}, self.alice, method="PATCH")
        self.commerce({"create_request": dict(title="Public supplies", description="PRIVATE request description", target=None, deadline=None)}, self.bob, "feed-request")
        self.commerce({"mint_art": dict(title="Public artwork", license="PRIVATE license", sha256="ab" * 32, locator="https://example.invalid/art.png")}, self.alice, "feed-art")
        self.new_user("carol")
        read = self.call("/me/api-key", {"password": "5678", "scope": "read"}, self.alice)["api_key"]
        feed = self.get(f"/activity?after={after}&limit=50", read)
        events = feed["events"]
        self.assertEqual([e["kind"] for e in events], ["listing_opened", "listing_sold", "gift_request_opened", "art_minted", "member_joined"])
        self.assertEqual([e["seq"] for e in events], sorted(e["seq"] for e in events))
        self.assertEqual((events[0]["subject"], events[0]["amount"], events[0]["side"]), (listing["id"], 20, "SELL"))
        self.assertEqual((events[1]["actor"], events[1]["other"], events[1]["amount"]), (self.accounts["bob"], self.accounts["alice"], 20))
        self.assertEqual(events[-1]["actor"], self.accounts["carol"])
        self.assertNotIn("amount", events[2])
        self.assertNotIn("PRIVATE", json.dumps(feed))
        self.assertNotIn(read, json.dumps(feed))
        self.restart()
        self.assertEqual(self.get(f"/activity?after={after}", self.bob)["events"], events)

    def test_activity_loan_and_quote_public_fields_and_paid_projection(self):
        after = self.get("/activity", self.alice)["sequence"]
        self.issue("alice", 20000)
        self.call("/admin/issue-usd", dict(to=self.accounts["bob"], cents=500, reason="PRIVATE cash"), self.nana, "feed-cash", expected=201)
        terms = self.loan_terms(amount=40, installment=10, memo="PRIVATE loan note")
        loan = self.call("/loans/request", terms, self.bob, "feed-loan-request")
        path = "/loans/" + str(loan["id"])
        self.call(path + "/offer", terms, self.alice, "feed-loan-offer")
        self.call(path + "/accept", {}, self.bob, "feed-loan-accept")
        self.call(path + "/repay", {"amount": 10}, self.bob, "feed-loan-part")
        self.call(path + "/repay", {"amount": 30}, self.bob, "feed-loan-done")
        quote = self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=201)
        self.call("/quotes/" + quote["id"] + "/take", {}, self.bob, "feed-quote-take", expected=201)
        events = self.get(f"/activity?after={after}", self.nana)["events"]
        self.assertEqual([e["kind"] for e in events], ["loan_requested", "loan_funded", "loan_paid", "quote_posted", "quote_taken"])
        self.assertEqual((events[0]["amount"], events[0]["apr_bps"]), (40, 0))
        self.assertEqual((events[1]["actor"], events[1]["other"]), (self.accounts["alice"], self.accounts["bob"]))
        self.assertEqual((events[2]["actor"], events[2]["other"]), (self.accounts["bob"], self.accounts["alice"]))
        self.assertEqual((events[-1]["subject"], events[-1]["side"], events[-1]["rate"], events[-1]["coins"]), (quote["id"], "ASK", 250, 10000))
        self.assertNotIn("PRIVATE", json.dumps(events))
        self.restart()
        self.assertEqual(self.get(f"/activity?after={after}", self.alice)["events"], events)

    def test_activity_limit_and_cursor_boundaries(self):
        self.call("/activity", expected=401)
        for query in ("limit=0", "limit=51", "limit=18446744073709551615", "after=bad", "limit=bad"):
            self.call("/activity?" + query, token=self.alice, expected=400)
        initial = self.get("/activity?limit=1", self.alice)
        self.assertEqual(len(initial["events"]), 1)
        after = initial["events"][0]["seq"]
        rest = self.get(f"/activity?after={after}", self.alice)["events"]
        self.assertTrue(all(e["seq"] > after for e in rest))
        self.assertEqual(self.get("/activity?after=18446744073709551615", self.alice)["events"], [])
        self.call("/activity", {}, self.alice, expected=404)

    def test_audit_pagination_permissions_stale_inputs_and_restart(self):
        for i in range(36):
            self.transfer(self.alice, "bob", 0, f"audit-{i}", memo="Private audit fixture")
        self.call("/audit", token=self.alice, expected=403)
        first = self.get("/audit", self.nana)
        self.assertEqual(len(first["audit"]), 16)
        self.call("/audit?incarnation=bad&before=bad", token=self.nana, expected=409)
        self.call("/audit?page=18446744073709551615", token=self.nana, expected=409)
        self.call("/audit?before=bad", token=self.nana, expected=400)
        sequence = self.get("/activity", self.alice)["sequence"]
        self.restart()
        rows = first["audit"][:]
        page = first
        for _ in range(10):
            before = page.get("next_before")
            if before is None:
                break
            page = self.get(f"/audit?incarnation={first['incarnation']}&before={before}&page={page['next_page']}", self.nana)
            rows.extend(page["audit"])
        else:
            self.fail("Audit cursor did not terminate")
        ids = [a["sequence"] for a in rows]
        self.assertEqual(ids, sorted(set(ids), reverse=True))
        self.assertEqual(len(ids), 39)
        self.assertEqual(self.get("/activity", self.alice)["sequence"], sequence)

    def test_clock_identity_updates_are_not_member_join_events(self):
        initial = self.get("/activity", self.nana)
        self.assertEqual([e["kind"] for e in initial["events"]], ["member_joined"] * 3)
        after = initial["sequence"]
        path = "/users/" + self.users["alice"]["id"]
        self.assertEqual(self.get("/me", self.alice)["created_at"], self.now())
        self.call(path, {"display_name": "Alice Admin", "role": "nana"}, self.nana, method="PATCH")
        self.call(path, {"display_name": "Alice Updated", "role": "user", "password": "9876"}, self.nana, method="PATCH")
        self.assertEqual(self.get(f"/activity?after={after}", self.nana)["events"], [])
        self.assertEqual(len(self.get("/activity", self.nana)["events"]), 3)
        audit = self.get("/audit", self.nana)["audit"]
        changed = [a["action"]["Identity"] for a in audit if a["sequence"] > after]
        self.assertEqual(len(changed), 2)
        self.assertTrue(all(not a["created"] for a in changed))
        self.assertTrue(any(a["credentials_changed"] for a in changed))
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.nana = self.login("nana", "1234")
        self.alice = self.login("alice", "9876")
        self.assertEqual(self.get(f"/activity?after={after}", self.alice)["events"], [])
        self.assertEqual(self.get("/me", self.alice)["display_name"], "Alice Updated")
        self.assertEqual(len(self.get("/activity", self.alice)["events"]), 3)

    def test_read_keys_and_password_revocation(self):
        read = self.call("/me/api-key", {"password": "5678", "scope": "read"}, self.alice)["api_key"]
        self.get("/me", read)
        self.transfer(read, "bob", 1, "read-write", expected=403)
        uid = self.get("/me", self.alice)["id"]
        self.call("/users/" + uid, {"password": "9876"}, self.alice, method="PATCH")
        self.call("/me", token=read, expected=401)
        self.call("/me", token=self.alice, expected=401)
        self.login("alice", "9876")

    def test_password_change_revokes_both_key_scopes_and_persists(self):
        full = self.call("/me/api-key", {"password": "5678"}, self.alice)["api_key"]
        read = self.call("/me/api-key", {"password": "5678", "scope": "read"}, self.alice)["api_key"]
        self.get("/me", full)
        self.get("/me", read)
        path = "/users/" + self.users["alice"]["id"]
        self.call(path, {"password": "9876", "display_name": "Alice Secure"}, self.alice, method="PATCH")
        for token in (full, read, self.alice):
            self.call("/me", token=token, expected=401)
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.call("/me", token=full, expected=401)
        self.call("/me", token=read, expected=401)
        self.assertEqual(self.get("/me", self.login("alice", "9876"))["display_name"], "Alice Secure")
        self.call("/auth/authorize", dict(username="alice", password="5678", code_challenge="a" * 43,
            code_challenge_method="S256", redirect_uri="http://localhost:4200"), expected=401)

    def test_last_active_nana_counts_status_and_role_together(self):
        nana_id = self.get("/me", self.nana)["id"]
        alice_path = "/users/" + self.users["alice"]["id"]
        self.call(alice_path, {"role": "nana"}, self.nana, method="PATCH")
        self.alice = self.login("alice", "5678")
        self.call("/users/" + nana_id, {"status": "DISABLED"}, self.alice, method="PATCH")
        for payload in ({"role": "user"}, {"status": "DISABLED"}, {"role": "user", "status": "ACTIVE"}):
            self.call(alice_path, payload, self.alice, method="PATCH", expected=403)
        self.assertEqual(self.get("/me", self.alice)["role"], "nana")
        self.call("/users/" + nana_id, {"status": "ACTIVE"}, self.alice, method="PATCH")
        self.call(alice_path, {"role": "user"}, self.alice, method="PATCH")
        self.call("/me", token=self.alice, expected=401)
        self.restart()
        self.assertEqual(self.get("/me", self.alice)["role"], "user")
        self.assertEqual(self.get("/me", self.nana)["status"], "ACTIVE")

    def test_last_active_nana_in_member_32_and_cross_build_restart(self):
        for index in range(4, 33):
            last = self.call("/users", dict(username=f"member{index}", display_name=f"Member {index}",
                password="5678", role="nana" if index in (8, 32) else "user", grant=False), self.nana, expected=201)
            self.assertEqual((last["status"], last["kind"]), ("ACTIVE", "human"))
        self.assertEqual(last["id"], "user-32")
        final_nana = self.login("member32", "5678")
        self.call("/users/user-1", dict(status="DISABLED"), final_nana, method="PATCH")
        self.call("/users/user-8", dict(status="DISABLED"), final_nana, method="PATCH")
        for changes in (dict(status="DISABLED"), dict(role="user")):
            self.call("/users/" + last["id"], changes, final_nana, method="PATCH", expected=403)
        before = self.get("/users", final_nana)
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        final_nana = self.login("member32", "5678")
        self.assertEqual(self.get("/users", final_nana), before)
        self.assertEqual((self.get("/me", final_nana)["role"], self.get("/me", final_nana)["status"]), ("nana", "ACTIVE"))
        self.call("/users/" + last["id"], dict(status="DISABLED"), final_nana, method="PATCH", expected=403)

    def test_private_message_validation_and_classified_transfer_rejections(self):
        self.issue("alice", 100)
        for index, memo in enumerate(("", "\u3000", "Bad\nmessage")):
            self.transfer(self.alice, "bob", 0, f"blank-{index}", expected=400, memo=memo)
        self.transfer(self.alice, "alice", 0, "self-message", expected=400, memo="Hello")
        listing = self.call("/listings", dict(title="Bread", price=10, side="SELL",
            economic_kind="GOOD", quantity="1", unit="EACH"), self.bob, expected=201)
        base = dict(to=self.accounts["bob"], amount=10, memo="Bread", economic_kind="GOOD",
            quantity="1", unit="EACH", thing=listing["thing"])
        for index, changes in enumerate(({"quantity": "0"}, {"quantity": "1000000.001"},
                {"unit": "HOUR"}, {"economic_kind": "LABOR"}, {"thing": "thing-999999"})):
            self.call("/transfers", dict(base, **changes), self.alice, f"bad-class-{index}",
                expected=404 if "thing" in changes else 400)
        self.call("/transfers", dict(base, to=self.accounts["nana"], thing=None,
            economic_kind="LABOR", unit="HOUR"), self.alice, "labor-to-nana", expected=403)
        paid = self.call("/transfers", base, self.alice, "bread", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (90, 10))
        self.restart()
        self.assertEqual(self.call("/transfers", base, self.alice, "bread", expected=201), paid)

    def test_classified_decimal_quantity_boundaries_and_cross_build_retry(self):
        self.issue("alice", 100)
        base = dict(to=self.accounts["bob"], amount=1, memo="Exact quantity",
                    economic_kind="GOOD", unit="EACH")
        for index, quantity in enumerate(("", ".1", "0", "0.000", "-1", "+1", "1e3",
                "1.0001", "1..2", "1. 2", "4294967296", "4294967295",
                "1000000.001", "١", "1\u0000", " 1")):
            body = self.call("/transfers", dict(base, quantity=quantity), self.alice,
                             f"quantity-bad-{index}", expected=400)
            self.assertEqual(body["error"], "invalid_input")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))
        receipts = []
        for index, (quantity, milli) in enumerate((("0.001", 1), ("1", 1000),
                ("1.2", 1200), ("1.23", 1230), ("1.234", 1234),
                ("1000000", 1_000_000_000), ("1.", 1000), ("0001.001", 1001))):
            payload = dict(base, quantity=quantity)
            key = f"quantity-good-{index}"
            tx = self.call("/transfers", payload, self.alice, key, expected=201)
            self.assertEqual(tx["quantity_milli"], milli)
            receipts.append((payload, key, tx))
        self.assertEqual((self.balance("alice"), self.balance("bob")), (92, 8))
        self.restart()
        for payload, key, tx in receipts:
            self.assertEqual(self.call("/transfers", payload, self.alice, key, expected=201), tx)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (92, 8))

    def test_historical_amounts_round_trip_multiple_epochs_and_usd_is_unchanged(self):
        self.issue("alice", 1000)
        tx = self.transfer(self.alice, "bob", 100, "payment")
        self.call("/admin/issue-usd", dict(to=self.accounts["alice"], cents=123, reason="Cash"), self.nana, "usd", expected=201)
        for index, decimals in enumerate((5, 6, 4)):
            meta = self.get("/loans", self.nana)
            prefix = f'g{self.get("/status")["journal_generation"]}:m{meta["money_epoch"]}:'
            self.call("/admin/reform", dict(decimals=decimals, power=0,
                expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=False),
                self.nana, prefix + f"reform-{index}")
            view = self.get("/transactions/" + tx["id"])
            factor = 10 ** (decimals - 4)
            self.assertEqual(view["postings"][1]["amount"], 100)
            self.assertEqual(view["current_postings"][1]["amount"], 100 * factor)
            self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.nana)["balance"], 123)
        self.restart()
        meta = self.get("/loans", self.nana)
        prefix = f'g{self.get("/status")["journal_generation"]}:m{meta["money_epoch"]}:'
        self.call(f"/transactions/{tx['id']}/refund", dict(amount=40, reason="Original units"), self.bob, prefix + "refund")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (940, 60))
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_disabled_member_cannot_receive_or_sign_in(self):
        uid = self.get("/me", self.bob)["id"]
        self.call("/users/" + uid, {"status": "DISABLED"}, self.nana, method="PATCH")
        self.call("/me", token=self.bob, expected=401)
        self.issue("alice", 10)
        self.transfer(self.alice, "bob", 1, "disabled", expected=403)
        self.assertEqual(self.balance("alice"), 10)

    def test_disabled_accounts_block_normal_refunds_but_allow_nana_correction(self):
        self.issue("alice", 100)
        tx = self.transfer(self.alice, "bob", 30, "payment")
        self.call("/users/" + self.users["alice"]["id"], {"status": "DISABLED"}, self.nana, method="PATCH")
        self.call(f"/transactions/{tx['id']}/refund", dict(amount=10, reason="Refund"), self.bob, "blocked", expected=403)
        self.call("/admin/issue", dict(to=self.accounts["alice"], amount=1, reason="Blocked"), self.nana, "blocked-issue", expected=403)
        self.call("/admin/retire", {"from": self.accounts["alice"], "amount": 1, "reason": "Blocked"}, self.nana, "blocked-retire", expected=403)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        corrected = self.call(f"/transactions/{tx['id']}/reverse", {"reason": "Administrative correction"}, self.nana, "undo", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))
        # Restart manually: a disabled user cannot renew a session.
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.nana = self.login("nana", "1234")
        self.bob = self.login("bob", "5678")
        self.assertEqual(self.call(f"/transactions/{tx['id']}/reverse", {"reason": "Administrative correction"}, self.nana, "undo", expected=201), corrected)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_partial_refunds_preclude_reversal_and_never_overdraw_members(self):
        issued = self.issue("alice", 100)
        tx = self.transfer(self.alice, "bob", 30, "payment")
        self.transfer(self.bob, "alice", 30, "spent")
        path = f"/transactions/{tx['id']}/refund"
        self.call(path, dict(amount=1, reason="Unfunded"), self.bob, "refund", expected=409)
        self.issue("bob", 10, "fund-refund")
        refund = self.call(path, dict(amount=10, reason="Unfunded"), self.bob, "refund")
        self.call(f"/transactions/{tx['id']}/reverse", {"reason": "Partial exists"}, self.nana, "undo", expected=409)
        self.call(path, dict(amount=21, reason="Excess"), self.bob, "excess", expected=409)
        self.call(path, dict(amount=0, reason="Zero"), self.bob, "zero", expected=409)
        self.call(f"/transactions/{issued['id']}/refund", dict(amount=1, reason="Issuance"), self.alice, "issue-refund", expected=403)
        self.call(f"/transactions/{refund['id']}/refund", dict(amount=1, reason="Correction"), self.alice, "refund-refund", expected=403)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (110, 0))
        self.restart()
        self.assertEqual(self.call(path, dict(amount=10, reason="Unfunded"), self.bob, "refund"), refund)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (110, 0))

    def test_offer_accept_undo_is_atomic_and_retry_stable(self):
        self.issue("alice", 100)
        listing = self.call("/listings", dict(title="Cookies", description="Fresh", price=30, side="SELL"), self.bob, expected=201)
        offer = self.call("/listings/" + listing["id"] + "/offers", dict(amount=20, message="Twenty?"), self.alice, expected=201)
        path = "/offers/" + offer["id"]
        self.assertEqual(self.balance("alice"), 100)
        self.call(path + "/accept", {}, self.alice, "wrong-owner", expected=403)
        accepted = self.call(path + "/accept", {}, self.bob, "accept", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))
        self.assertEqual(self.get("/listings/" + listing["id"], self.alice)["status"], "SOLD")
        undone = self.call(path + "/unaccept", {"reason": "Cancelled"}, self.alice, "undo")
        self.assertEqual(undone["offer"]["status"], "REVERSED")
        self.assertEqual(self.get("/listings/" + listing["id"], self.alice)["status"], "ACTIVE")
        self.assertEqual(self.call(path + "/accept", {}, self.bob, "accept", expected=201), accepted)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_forex_exchanges_both_legs_once(self):
        self.issue("alice", 10000)
        self.call("/admin/issue-usd", dict(to=self.accounts["bob"], cents=500, reason="Cash"), self.nana, "cash", expected=201)
        quote = self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=201)
        path = "/quotes/" + quote["id"] + "/take"
        trade = self.call(path, {}, self.bob, "trade", expected=201)
        self.assertEqual(self.call(path, {}, self.bob, "trade", expected=201), trade)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 10000))
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.bob)["balance"], 250)
        self.assertEqual(self.get("/accounts/" + self.accounts["bob"] + "-usd", self.bob)["balance"], 250)

    def test_forex_cash_leg_failure_then_trade_retry_across_build_restart(self):
        self.issue("alice", 10000)
        quote = self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=201)
        path = "/quotes/" + quote["id"]
        refused = self.call(path + "/take", {}, self.alice, "self-trade", expected=400)
        self.assertEqual(refused["error"], "self_deal")
        self.call(path + "/take", {}, self.bob, "cash-short", expected=409)
        self.assertEqual(self.get(path, self.bob), quote)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (10000, 0))
        self.call("/admin/issue-usd", dict(to=self.accounts["bob"], cents=250, reason="Cash"), self.nana, "trade-cash", expected=201)
        trade = self.call(path + "/take", {}, self.bob, "cash-short", expected=201)
        self.assertEqual(trade["quote"]["status"], "FILLED")
        self.assertNotEqual(trade["coin_transaction"]["id"], trade["cash_transaction"]["id"])
        self.call(path + "/cancel", {}, self.nana, expected=409)
        self.call(path + "/take", {}, self.bob, "second-trade", expected=409)
        self.restart()
        self.assertEqual(self.call(path + "/take", {}, self.bob, "cash-short", expected=201), trade)
        self.assertEqual(self.get(path, self.bob), trade["quote"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 10000))
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.alice)["balance"], 250)
        self.assertEqual(self.get("/accounts/" + self.accounts["bob"] + "-usd", self.bob)["balance"], 0)

    def test_malformed_and_oversized_json(self):
        self.call("/transfers", token=self.alice, key="broken", raw=b'{"amount":', expected=400)
        self.call("/transfers", token=self.alice, key="large", raw=b'{"memo":"' + b"x" * 1100 + b'"}', expected=413)
        self.assertEqual(self.balance("alice"), 0)

    def test_concurrent_retries_pay_once(self):
        self.issue("alice", 100)
        def retry(_):
            return self.request("/transfers", dict(to=self.accounts["bob"], amount=10, memo="Concurrent"), self.alice, "parallel")
        with ThreadPoolExecutor(max_workers=4) as pool:
            replies = list(pool.map(retry, range(8)))
        # A busy transport may reject before execution; retry those responses.
        accepted = [reply for reply in replies if reply[0] == 201]
        self.assertTrue(accepted, replies)
        for status, body in replies:
            self.assertIn(status, (201, 503), body)
            if status == 201:
                self.assertEqual(body, accepted[0][1])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (90, 10))

    def test_browser_exact_balance_limit_and_atomic_overflow(self):
        for i in range(9):
            self.issue("alice", 1_000_000_000_000_000, f"large-{i}")
        self.issue("alice", 7_199_254_740_991, "exact")
        self.assertEqual(self.balance("alice"), 9_007_199_254_740_991)
        self.call("/admin/issue", dict(to=self.accounts["alice"], amount=1, reason="Too far"), self.nana, "overflow", expected=400)
        self.assertEqual(self.balance("alice"), 9_007_199_254_740_991)

    def test_exact_nc_and_usd_limits_corrections_and_cross_build_recovery(self):
        maximum = 9_007_199_254_740_991
        tranche = 1_000_000_000_000_000
        for index, amount in enumerate([tranche] * 9 + [maximum - 9 * tranche]):
            self.issue("alice", amount, f"nc-bound-{index}")
            self.call("/admin/issue-usd", dict(to=self.accounts["alice"], cents=amount,
                reason="Exact cash boundary"), self.nana, f"usd-bound-{index}", expected=201)
        usd_account = "/accounts/" + self.accounts["alice"] + "-usd"
        self.assertEqual(self.balance("alice"), maximum)
        self.assertEqual(self.get(usd_account, self.nana)["balance"], maximum)
        payment = self.transfer(self.alice, "bob", tranche, "boundary-payment")
        refunded = self.call(f"/transactions/{payment['id']}/refund",
            dict(amount=tranche - 1, reason="Leave one unit"), self.bob, "boundary-refund")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (maximum - 1, 1))
        self.call("/admin/issue", dict(to=self.accounts["alice"], amount=2,
            reason="Overflow"), self.nana, "nc-overflow", expected=400)
        self.call("/admin/issue-usd", dict(to=self.accounts["alice"], cents=1,
            reason="Overflow"), self.nana, "usd-overflow", expected=400)
        before = self.get("/transactions?limit=100")
        for row in before["transactions"]:
            self.assertEqual(sum(p["amount"] for p in row["postings"]), 0)
        self.assertTrue(self.get("/status")["ledger_balanced"])
        self.restart()
        self.assertEqual((self.balance("alice"), self.balance("bob")), (maximum - 1, 1))
        self.assertEqual(self.get(usd_account, self.nana)["balance"], maximum)
        self.assertEqual(self.get("/transactions?limit=100"), before)
        self.assertEqual(self.call(f"/transactions/{payment['id']}/refund",
            dict(amount=tranche - 1, reason="Leave one unit"), self.bob, "boundary-refund"), refunded)
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_ledger_postings_balance_and_cursor_pages(self):
        issued = self.issue("alice", 100)
        payments = [self.transfer(self.alice, "bob", 1, f"page-{i}") for i in range(7)]
        ids = []
        path = "/transactions?limit=2"
        for _ in range(20):
            page = self.get(path)
            self.assertLessEqual(len(page["transactions"]), 2)
            for tx in page["transactions"]:
                self.assertEqual(sum(p["amount"] for p in tx["postings"]), 0)
                ids.append(tx["id"])
            cursor = page.get("next_cursor")
            if not cursor:
                break
            path = "/transactions?limit=2&cursor=" + cursor
        else:
            self.fail("Cursor did not terminate")
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(set(ids), {issued["id"], *(tx["id"] for tx in payments)})

    def test_ledger_cursor_limits_staleness_and_snapshot_restart(self):
        issued = self.issue("alice", 100)
        payments = [self.transfer(self.alice, "bob", 1, f"snapshot-{i}") for i in range(4)]
        self.assertEqual(len(self.get("/transactions?limit=0")["transactions"]), 1)
        self.assertEqual(len(self.get("/transactions?limit=18446744073709551615")["transactions"]), 5)
        self.call("/transactions?limit=bad", expected=400)
        first = self.get("/transactions?limit=2")
        cursor = first["next_cursor"]
        numbers = list(map(int, cursor.split(':')))
        for index in range(4):
            wrong = numbers.copy()
            wrong[index] = ((wrong[0] + 1) % (2**64)) if index == 0 else 2**64 - 1
            rejected = self.call("/transactions?cursor=" + ':'.join(map(str, wrong)), expected=409)
            self.assertEqual(rejected["error"], "stale_request")
        self.call("/transactions?before=18446744073709551615", expected=409)
        late = self.transfer(self.alice, "bob", 1, "after-snapshot")
        self.restart()
        ids = [t["id"] for t in first["transactions"]]
        while cursor:
            page = self.get("/transactions?limit=2&cursor=" + cursor)
            self.assertEqual(page["snapshot_upper"], first["snapshot_upper"])
            ids.extend(t["id"] for t in page["transactions"])
            cursor = page.get("next_cursor")
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(set(ids), {issued["id"], *(t["id"] for t in payments)})
        self.assertNotIn(late["id"], ids)
        self.assertFalse(self.get("/transactions")["history_truncated"])

    def test_direct_purchase_retry_and_cancel(self):
        self.issue("alice", 100)
        listing = self.call("/listings", dict(title="Bread", description="Loaf", price=30, side="SELL"), self.bob, expected=201)
        path = "/listings/" + listing["id"]
        purchase = self.call(path + "/purchase", {}, self.alice, "buy", expected=201)
        self.assertEqual(self.call(path + "/purchase", {}, self.alice, "buy", expected=201), purchase)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        listing = self.call("/listings", dict(title="Jam", price=10, side="SELL"), self.bob, expected=201)
        path = "/listings/" + listing["id"]
        self.call(path + "/cancel", {}, self.bob)
        self.call(path + "/purchase", {}, self.alice, "closed", expected=409)
        self.assertEqual(self.balance("alice"), 70)

    def test_clock_listing_order_filters_and_cross_build_restart(self):
        self.assertEqual(self.get("/listings", self.alice)["listings"], [])
        self.call("/listings?status=unknown", token=self.alice, expected=400)
        self.issue("alice", 100)
        listings = [self.call("/listings", dict(title=f"Ordered {i}", price=1, side="SELL"),
            self.bob, expected=201) for i in range(23)]
        self.assertEqual(len({row["created_at"] for row in listings}), 1)
        self.call("/listings/" + listings[0]["id"] + "/cancel", {}, self.bob)
        self.call("/listings/" + listings[1]["id"] + "/purchase", {}, self.alice, "ordered-sale", expected=201)
        self.advance(1)
        newest = self.listing(self.bob, price=2)
        self.call("/listings/" + listings[2]["id"], {"price": 3}, self.bob, method="PATCH")
        rows = self.get("/listings", self.alice)["listings"]
        self.assertEqual([row["id"] for row in rows], [newest["id"], *(row["id"] for row in listings)])
        for status in ("ACTIVE", "SOLD", "CANCELLED"):
            self.assertEqual(self.get("/listings?status=" + status, self.alice)["listings"],
                [row for row in rows if row["status"] == status])
        for status in ("active", "unknown"):
            self.call("/listings?status=" + status, token=self.alice, expected=400)
        self.assertEqual(self.get("/listings?status=SOLD", self.alice)["listings"][0]["buyer"], self.accounts["alice"])
        self.assertEqual(self.get("/status")["active_listings"], 22)
        self.assertEqual(self.get("/status")["circulation"], 100)
        self.restart()
        self.assertEqual(self.get("/listings", self.alice)["listings"], rows)
        self.assertEqual(self.get("/listings?status=CANCELLED", self.alice)["listings"],
            [row for row in rows if row["status"] == "CANCELLED"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (99, 1))
        self.assertEqual(self.get("/status")["active_listings"], 22)

    def test_listing_capacity_pins_accepted_deals_and_recycles_cancelled_rows(self):
        self.issue("alice", 100)
        listings = [self.call("/listings", dict(title=f"Item {i}", price=30, side="SELL"), self.bob, expected=201) for i in range(48)]
        offer = self.call("/listings/" + listings[0]["id"] + "/offers", {"amount": 20}, self.alice, expected=201)
        unselected = self.call("/listings/" + listings[0]["id"] + "/offers", {"amount": 15}, self.nana, expected=201)
        path = "/offers/" + offer["id"]
        self.call(path + "/accept", {}, self.bob, "pin-sale", expected=201)
        self.assertEqual(self.get("/offers/" + unselected["id"], self.nana)["status"], "NOT_SELECTED")
        self.call("/listings", dict(title="No room", price=30), self.bob, expected=507)
        self.call("/listings/" + listings[1]["id"] + "/cancel", {}, self.bob)
        new = self.call("/listings", dict(title="Replacement", price=30), self.bob, expected=201)
        self.assertNotIn(new["id"], {l["id"] for l in listings})
        self.call("/listings/" + listings[1]["id"], token=self.alice, expected=404)
        self.restart()
        self.assertEqual(self.get("/listings/" + listings[0]["id"], self.alice)["status"], "SOLD")
        self.assertTrue(self.get(path, self.alice)["reversible"])
        self.call(path + "/unaccept", {"reason": "Return"}, self.alice, "unpin")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))
        self.assertEqual(self.get("/listings/" + listings[0]["id"], self.alice)["status"], "ACTIVE")
        self.assertEqual(self.get("/offers/" + unselected["id"], self.nana)["status"], "OPEN")
        self.call("/listings", dict(title="Still full", price=30), self.bob, expected=507)

    def test_buy_listing_edits_permissions_and_closed_purchase(self):
        self.call("/listings", dict(title="Bad units", price=1, minor_units=-(2**63)), self.bob, expected=400)
        self.call("/listings", dict(title="Unknown kind", price=1, kind="unknown"), self.bob, expected=400)
        self.issue("alice", 100)
        listing = self.listing(self.alice, side="BUY", price=30)
        path = "/listings/" + listing["id"]
        self.call(path, {"price": 20}, self.bob, method="PATCH", expected=403)
        self.call(path, {"title": " "}, self.nana, method="PATCH", expected=400)
        self.assertEqual(self.get(path, self.bob)["price"], 30)
        self.call(path + "/purchase", {}, self.alice, "self-purchase", expected=400)
        changed = self.call(path, {"price": 20, "title": "Wanted item"}, self.alice, method="PATCH")
        self.assertEqual((changed["price"], changed["title"]), (20, "Wanted item"))
        self.call(path + "/purchase", {}, self.bob, "supply", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))
        self.call(path, {"price": 10}, self.alice, method="PATCH", expected=409)
        self.call(path + "/cancel", {}, self.nana, expected=409)

    def test_classified_listings_reuse_things_and_preserve_units(self):
        item = dict(title="Bread", price=30, side="SELL", economic_kind="GOOD", quantity="1.5", unit="EACH")
        first = self.call("/listings", item, self.bob, expected=201)
        second = self.call("/listings", dict(item, title="BREAD", standard=True), self.alice, expected=201)
        self.assertEqual((second["thing"], second["title"], second["quantity_milli"]), (first["thing"], "Bread", 1500))
        self.assertTrue(self.get("/listings/" + first["id"], self.alice)["standard"])
        self.call("/listings", dict(item, unit="KILOGRAM"), self.bob, expected=409)
        self.call("/listings", dict(item, thing=first["thing"], unit="KILOGRAM"), self.bob, expected=400)
        self.call("/listings", dict(item, quantity="0"), self.bob, expected=400)
        self.call("/listings", dict(item, title="Work", economic_kind="LABOR"), self.nana, expected=403)
        self.restart()
        things = self.get("/things", self.alice)["things"]
        self.assertEqual(len(things), 1)
        self.assertEqual((things[0]["id"], things[0]["unit"], things[0]["standard"]), (first["thing"], "EACH", True))

    def test_marketplace_labor_role_subject_and_prepared_fulfillment_recovery(self):
        self.issue("nana", 100, "nana-labor-funds")
        self.issue("bob", 100, "bob-labor-funds")
        fields = dict(title="Work", price=30, side="BUY", economic_kind="LABOR", quantity="1", unit="HOUR")
        wanted = self.call("/listings", fields, self.bob, expected=201)
        target = "/listings/" + wanted["id"]
        self.call(target + "/purchase", {}, self.nana, "nana-provider", expected=403)
        offer = self.call(target + "/offers", {"amount": 20}, self.nana, expected=201)
        self.call("/offers/" + offer["id"] + "/accept", {}, self.bob, "nana-provider-offer", expected=403)
        self.assertEqual(self.get("/offers/" + offer["id"], self.bob)["status"], "OPEN")
        self.assertEqual(self.get(target, self.bob)["status"], "ACTIVE")
        wanted = self.call("/listings", fields, self.nana, expected=201)
        target = "/listings/" + wanted["id"]
        offer = self.call(target + "/offers", {"amount": 20}, self.bob, expected=201)
        offer_path = "/offers/" + offer["id"]
        accepted = self.call(offer_path + "/accept", {}, self.nana, "nana-payer-offer", expected=201)
        offer_tx = accepted["transaction"]["id"]
        self.assertEqual(next(f for f in self.get("/fulfillments", self.nana)["fulfillments"] if f["transaction"] == offer_tx)["status"], "TODO")
        self.call(offer_path + "/unaccept", {"reason": "Changed plan"}, self.nana, "undo-labor")
        purchase = self.call(target + "/purchase", {}, self.bob, "nana-payer-direct", expected=201)
        tx = purchase["transaction"]["id"]
        self.assertEqual(purchase["transaction"]["fulfillment"]["status"], "TODO")
        self.restart()
        self.assertEqual(self.call(target + "/purchase", {}, self.bob, "nana-payer-direct", expected=201), purchase)
        correction = self.call("/transactions/" + tx + "/reverse", {"reason": "Undo work"}, self.nana, "reverse-labor", expected=201)
        self.restart()
        self.assertEqual(self.call("/transactions/" + tx + "/reverse", {"reason": "Undo work"}, self.nana, "reverse-labor", expected=201), correction)
        self.assertEqual(next(f for f in self.get("/fulfillments", self.nana)["fulfillments"] if f["transaction"] == tx)["status"], "REVERSED")
        self.assertEqual((self.balance("nana"), self.balance("bob")), (100, 100))

    def test_refund_reason_controls_precede_lookup_and_leave_payments_unchanged(self):
        self.issue("alice", 100)
        tx = self.transfer(self.alice, "bob", 40, "reason-payment")
        balances = (self.balance("alice"), self.balance("bob"))
        for reason in ("bad\nreason", "bad\treason", "bad\u0085reason"):
            for transaction in (tx["id"], "tx-99999999"):
                rejected = self.call("/transactions/" + transaction + "/refund",
                    dict(amount=10, reason=reason), self.bob,
                    f"bad-reason-{transaction}-{ord(reason[3])}", expected=400)
                self.assertEqual(rejected["error"], "invalid_input")
        self.assertEqual((self.balance("alice"), self.balance("bob")), balances)
        refunded = self.call("/transactions/" + tx["id"] + "/refund",
            dict(amount=10, reason=""), self.bob, "empty-reason")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        self.restart()
        self.assertEqual(self.call("/transactions/" + tx["id"] + "/refund",
            dict(amount=10, reason=""), self.bob, "empty-reason"), refunded)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_recipient_refund_is_bounded_and_retry_stable(self):
        self.issue("alice", 100)
        tx = self.transfer(self.alice, "bob", 30, "pay")
        path = "/transactions/" + tx["id"] + "/refund"
        self.call(path, {"amount": 31, "reason": "Too much"}, self.bob, "large-refund", expected=409)
        refund = self.call(path, {"amount": 10, "reason": "Partial"}, self.bob, "partial")
        self.assertEqual(self.call(path, {"amount": 10, "reason": "Partial"}, self.bob, "partial"), refund)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_loan_proposal_acceptance_and_repayment(self):
        self.issue("alice", 100)
        loan = self.call("/loans", dict(borrower=self.accounts["bob"], amount=40,
            rate_bps=0, rate_days=365, payment_days=30, installment=10, credit=False, memo="Lunch"), self.alice, "loan")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))
        path = "/loans/" + str(loan["id"])
        accepted = self.call(path + "/accept", {}, self.bob, "accept-loan")
        self.assertEqual(accepted["status"], "ACTIVE")
        self.assertEqual(self.call(path + "/accept", {}, self.bob, "accept-loan"), accepted)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (60, 40))
        repaid = self.call(path + "/repay", {"amount": 40}, self.bob, "repay")
        self.assertEqual(repaid["status"], "PAID")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_configuration_and_member_grants(self):
        self.call("/admin/config", {"initial_grant": 25, "household_name": "New home"}, self.nana, method="PATCH")
        config = self.get("/admin/config", self.nana)
        self.assertEqual(config["initial_grant"], 25)
        user = self.call("/users", dict(username="carol", display_name="Carol", password="8765"), self.nana, expected=201)
        self.assertEqual(user["balance"], 25)
        self.call("/admin/config", {"initial_grant": 1}, self.alice, method="PATCH", expected=403)

    def test_member_wire_defaults_bot_role_and_grant_selection(self):
        self.call("/admin/config", {"initial_grant": 25}, self.nana, method="PATCH")
        before = self.get("/users", self.nana)["users"]
        self.call("/users", dict(username="robot", display_name="", password="5678", kind="bot", role="nana"), self.nana, expected=400)
        self.assertEqual(self.get("/users", self.nana)["users"], before)
        bot = self.call("/users", dict(username="robot", display_name="\u3000", password="5678", kind="bot"), self.nana, expected=201)
        human = self.call("/users", dict(username="helper", display_name="", password="5678", role="nana", grant=False), self.nana, expected=201)
        self.assertEqual((bot["display_name"], bot["role"], bot["kind"], bot["balance"]), ("robot", "user", "bot", 25))
        self.assertEqual((human["display_name"], human["role"], human["balance"]), ("helper", "nana", 0))
        self.restart()
        rows = {u["username"]: u for u in self.get("/users", self.nana)["users"]}
        self.assertEqual((rows["robot"]["kind"], rows["robot"]["balance"], rows["helper"]["role"]), ("bot", 25, "nana"))

    def test_reformed_wire_epoch_guard_precedes_body_and_keeps_reads_available(self):
        self.issue("alice", 100)
        self.call("/provision", token=self.nana, raw=b"{", expected=403)
        meta = self.get("/loans", self.nana)
        self.call("/admin/reform", dict(decimals=5, power=0,
            expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=False),
            self.nana, "epoch-reform")
        stable = (self.balance("alice"), self.balance("bob"))
        for path in ("/transfers", "/users", "/admin/config", "/commands"):
            for key in ("", "g0:m0:old", "g0:m18446744073709551615:huge", "g0:mbad:bad"):
                body = self.call(path, token=self.nana, key=key, raw=b"{", expected=409,
                                 method="PATCH" if path == "/admin/config" else "POST")
                self.assertEqual(body["error"], "stale_request")
        self.assertEqual((self.balance("alice"), self.balance("bob")), stable)
        self.assertEqual(self.get("/me", self.alice)["balance"], 1000)
        payload = dict(to=self.accounts["bob"], amount=10, memo="Current units")
        paid = self.call("/transfers", payload, self.alice, "g0:m1:current", expected=201)
        self.restart()
        self.assertEqual(self.call("/transfers", payload, self.alice, "g0:m1:current", expected=201), paid)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (990, 10))

    def test_delegated_bot_key_views_scope_guards_and_cross_build_restart(self):
        human = "/users/" + self.users["bob"]["id"] + "/api-key"
        self.call(human, token=self.nana, expected=403)
        self.call("/users/user-32/api-key", token=self.alice, expected=403)
        self.call("/users/user-32/api-key", token=self.nana, expected=404)
        bot = self.call("/users", dict(username="keybot", display_name="Key bot", password="5678",
                                      kind="bot", grant=False), self.nana, expected=201)
        target = "/users/" + bot["id"] + "/api-key"
        empty = self.get(target, self.nana)
        self.assertEqual(empty, dict(active=False, created_at=None,
            full=dict(active=False, created_at=None), read=dict(active=False, created_at=None)))
        made = self.call(target, {}, self.nana)
        active = self.get(target + "?scope=read", self.nana)
        self.assertTrue(active["active"])
        self.assertEqual(active["created_at"], made["created_at"])
        self.assertFalse(active["read"]["active"])
        for method in ("POST", "DELETE"):
            self.call(target + "?scope=read", {}, self.nana, method=method, expected=403)
        self.assertEqual(self.get(target, self.nana), active)
        self.restart()
        self.assertEqual(self.get(target, self.nana), active)
        self.assertEqual(self.get("/me", made["api_key"])["id"], bot["id"])
        self.call(target, token=self.nana, method="DELETE")
        self.call("/me", token=made["api_key"], expected=401)
        self.assertEqual(self.get(target, self.nana), empty)

    def test_cors_preflight_and_denied_origin(self):
        req = urllib.request.Request(self.base + "/me", method="OPTIONS", headers={
            "Origin": "http://localhost:4200", "Access-Control-Request-Method": "POST",
            "Access-Control-Request-Headers": "authorization,content-type,idempotency-key"})
        with HTTP.open(req, timeout=10) as response:
            self.assertEqual(response.status, 204)
            self.assertEqual(response.headers["Access-Control-Allow-Origin"], "http://localhost:4200")
            allowed = response.headers["Access-Control-Allow-Headers"].lower()
            self.assertIn("idempotency-key", allowed)
        denied = urllib.request.Request(self.base + "/status", headers={"Origin": "https://untrusted.invalid"})
        with self.assertRaises(urllib.error.HTTPError) as error:
            HTTP.open(denied, timeout=10)
        self.assertEqual(error.exception.code, 403)
        error.exception.close()

    def test_pkce_redirect_mismatch_consumes_code(self):
        grant = self.authorize("alice", "5678")
        self.call("/auth/token", dict(grant, redirect_uri="http://localhost:4200/wrong"), expected=401)
        self.call("/auth/token", grant, expected=401)

    def test_full_key_replacement_persistence_and_credential_restrictions(self):
        first = self.call("/me/api-key", {"password": "5678"}, self.alice)["api_key"]
        key = self.call("/me/api-key", {"password": "5678"}, self.alice)["api_key"]
        self.call("/me", token=first, expected=401)
        uid = self.get("/me", key)["id"]
        self.call("/users/" + uid, {"password": "9999"}, key, method="PATCH", expected=403)
        self.call("/me/api-key", token=key, method="DELETE", expected=403)
        self.issue("alice", 20)
        self.transfer(key, "bob", 5, "key-payment")
        self.restart()
        self.assertEqual(self.get("/me", key)["balance"], 15)
        self.call("/me/api-key", token=self.alice, method="DELETE")
        self.call("/me", token=key, expected=401)

    def test_bot_keys_and_good_deed_restriction(self):
        bot = self.call("/users", dict(username="robot", display_name="Robot", password="5678",
            kind="bot", grant=False), self.nana, expected=201)
        self.assertEqual(bot["kind"], "bot")
        path = "/users/" + bot["id"] + "/api-key"
        self.call(path, {}, self.alice, expected=403)
        key = self.call(path, {}, self.nana)["api_key"]
        self.assertEqual(self.get("/me", key)["id"], bot["id"])
        deed = self.listing(self.nana, side="BUY", kind="good_deed")
        rejected = self.call("/listings/" + deed["id"] + "/offers", {"amount": 10}, key, expected=403)
        self.assertEqual(rejected["error"], "bot_good_deed")
        self.call(path, token=self.nana, method="DELETE")
        self.call("/me", token=key, expected=401)

    def test_role_changes_revoke_sessions_and_change_permissions(self):
        uid = self.get("/me", self.alice)["id"]
        self.call("/users/" + uid, {"role": "nana"}, self.nana, method="PATCH")
        self.call("/me", token=self.alice, expected=401)
        self.alice = self.login("alice", "5678")
        self.call("/admin/issue", dict(to=self.accounts["bob"], amount=10, reason="New Nana"), self.alice, "role-issue", expected=201)
        self.call("/users/" + uid, {"role": "user"}, self.nana, method="PATCH")
        self.call("/me", token=self.alice, expected=401)
        self.alice = self.login("alice", "5678")
        self.call("/admin/issue", dict(to=self.accounts["bob"], amount=10, reason="Old Nana"), self.alice, "role-denied", expected=403)
        self.assertEqual(self.balance("bob"), 10)

    def test_idempotency_header_bounds(self):
        self.issue("alice", 20)
        for key in ("", "x" * 81):
            self.transfer(self.alice, "bob", 1, key, expected=400)
        self.transfer(self.alice, "bob", 1, "x" * 80)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (19, 1))

    def test_failed_request_can_retry_and_keys_are_per_member(self):
        self.transfer(self.alice, "bob", 10, "shared", expected=409)
        self.issue("alice", 20)
        first = self.transfer(self.alice, "bob", 10, "shared")
        second = self.transfer(self.bob, "alice", 5, "shared")
        self.assertNotEqual(first["id"], second["id"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (15, 5))

    def test_typed_command_watermarks_and_receipts(self):
        self.issue("alice", 20)
        command = {"transfer": dict(to=int(self.accounts["bob"].removeprefix("account-")), amount=5, memo="Typed")}
        first = self.call("/commands", {"request_id": 1, "command": command}, self.alice)
        retry = self.call("/commands", {"request_id": 1, "command": command}, self.alice)
        self.assertEqual(retry["sequence"], first["sequence"])
        self.assertTrue(retry["replayed"])
        changed = {"transfer": dict(command["transfer"], amount=6)}
        self.call("/commands", {"request_id": 1, "command": changed}, self.alice, expected=409)
        self.call("/commands", {"request_id": 2, "command": command}, self.alice)
        old = self.call("/commands", {"request_id": 1, "command": command}, self.alice, expected=409)
        self.assertEqual(old["error"], "stale_request")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (10, 10))

    def test_buy_listing_offer_pays_the_provider(self):
        self.issue("alice", 100)
        listing = self.listing(self.alice, side="BUY")
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20}, self.bob, expected=201)
        self.call("/offers/" + offer["id"] + "/accept", {}, self.alice, "buy-offer", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_offer_privacy_decline_and_withdrawal(self):
        outsider = self.new_user("carol")
        listing = self.listing(self.bob)
        path = "/listings/" + listing["id"] + "/offers"
        offer = self.call(path, {"amount": 20, "message": "Private bid"}, self.alice, expected=201)
        target = "/offers/" + offer["id"]
        self.call(target, token=outsider, expected=404)
        self.assertEqual(self.get("/offers", outsider)["offers"], [])
        profile = self.get("/offers?member=" + self.users["alice"]["id"], outsider)
        self.assertEqual(profile["offers"][0]["message"], "")
        self.assertEqual(self.call(target + "/decline", {}, self.bob)["status"], "DECLINED")
        offer = self.call(path, {"amount": 10}, self.alice, expected=201)
        self.assertEqual(self.call("/offers/" + offer["id"] + "/withdraw", {}, self.alice)["status"], "WITHDRAWN")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_offer_capacity_preserves_open_deals_and_recycles_closed_ones(self):
        listing = self.listing(self.bob)
        path = "/listings/" + listing["id"] + "/offers"
        offers = [self.call(path, {"amount": 20}, self.alice, expected=201) for _ in range(32)]
        self.call(path, {"amount": 20}, self.alice, expected=507)
        self.assertEqual(len(self.get("/offers", self.alice)["offers"]), 32)
        self.call("/offers/" + offers[0]["id"] + "/decline", {}, self.bob)
        replacement = self.call(path, {"amount": 25}, self.alice, expected=201)
        self.assertNotEqual(replacement["id"], offers[0]["id"])
        self.call("/offers/" + offers[0]["id"], token=self.alice, expected=404)
        self.restart()
        self.assertEqual(len(self.get("/offers", self.alice)["offers"]), 32)
        for offer in offers[1:]:
            self.assertEqual(self.get("/offers/" + offer["id"], self.alice)["status"], "OPEN")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_newest_first_offer_loan_lotto_books_and_cross_build_recovery(self):
        listing = self.listing(self.bob)
        path = "/listings/" + listing["id"] + "/offers"
        offers = [self.call(path, {"amount": 20}, self.alice, expected=201)["id"] for _ in range(32)]
        loans = [self.call("/loans/request", self.loan_terms(borrower="alice"), self.alice,
            f"book-loan-{i}")["id"] for i in range(32)]
        lottos = [self.call("/lottos", dict(kind="SIMPLE", title=f"Book draw {i}", ticket_price=10,
            closes_at=self.now() + 86400, rate_bps=0), self.nana, f"book-lotto-{i}")["id"] for i in range(16)]
        def check():
            self.assertEqual([o["id"] for o in self.get("/offers", self.alice)["offers"]], list(reversed(offers)))
            subject = self.accounts["alice"].replace("account-", "user-")
            self.assertEqual([o["id"] for o in self.get("/offers?member=" + subject, self.bob)["offers"]], list(reversed(offers)))
            self.assertEqual([l["id"] for l in self.get("/loans", self.nana)["loans"]], list(reversed(loans)))
            self.assertEqual([l["id"] for l in self.get("/lottos", self.alice)["lottos"]], list(reversed(lottos)))
        check()
        self.call("/offers/" + offers.pop(0) + "/decline", {}, self.bob)
        offers.append(self.call(path, {"amount": 25}, self.alice, expected=201)["id"])
        self.call("/loans/" + str(loans.pop(0)) + "/close", {}, self.alice, "book-close")
        loans.append(self.call("/loans/request", self.loan_terms(borrower="alice"), self.alice, "book-replacement")["id"])
        check()
        self.restart()
        check()
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_concurrent_distinct_offer_acceptances_pay_once(self):
        self.issue("alice", 100)
        listing = self.listing(self.bob)
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20}, self.alice, expected=201)
        path = "/offers/" + offer["id"] + "/accept"
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(lambda key: self.request(path, {}, self.bob, key), ("race-one", "race-two")))
        self.assertEqual(sorted(status for status, _ in results), [201, 409])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))
        self.restart()
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_unfunded_offer_acceptance_leaves_everything_open(self):
        listing = self.listing(self.bob)
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20}, self.alice, expected=201)
        self.call("/offers/" + offer["id"] + "/accept", {}, self.bob, "unfunded", expected=409)
        self.assertEqual(self.get("/offers/" + offer["id"], self.alice)["status"], "OPEN")
        self.assertEqual(self.get("/listings/" + listing["id"], self.alice)["status"], "ACTIVE")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_offer_settlement_deadline_is_persistent_and_reads_do_not_write(self):
        self.call("/admin/config", {"offer_settles_after": 2}, self.nana, method="PATCH")
        self.issue("alice", 100)
        listing = self.listing(self.bob)
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20}, self.alice, expected=201)
        path = "/offers/" + offer["id"]
        accepted = self.call(path + "/accept", {}, self.bob, "deadline", expected=201)
        self.restart()
        self.assertEqual(self.get(path, self.alice)["settles_at"], accepted["offer"]["settles_at"])
        before = self.get("/activity", self.alice)["sequence"]
        if CLOCK:
            self.advance(3)
        settled = self.wait_for(lambda: self.get(path, self.alice), lambda v: v["status"] == "SETTLED")
        self.assertFalse(settled["reversible"])
        self.assertEqual(self.get("/activity", self.alice)["sequence"], before)
        error = self.call(path + "/unaccept", {"reason": "Late"}, self.alice, "late", expected=409)
        self.assertEqual(error["error"], "offer_settled")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_manual_reversal_prevents_second_offer_refund(self):
        self.issue("alice", 100)
        listing = self.listing(self.bob)
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20}, self.alice, expected=201)
        path = "/offers/" + offer["id"]
        accepted = self.call(path + "/accept", {}, self.bob, "accept", expected=201)
        self.call("/transactions/" + accepted["transaction"]["id"] + "/reverse", {"reason": "Correct"}, self.nana, "reverse", expected=201)
        self.call(path + "/unaccept", {"reason": "Again"}, self.alice, "twice", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_good_deeds_issue_and_undo_retires_reward(self):
        listing = self.listing(self.nana, side="BUY", kind="good_deed")
        offer = self.call("/listings/" + listing["id"] + "/offers", {"amount": 20}, self.alice, expected=201)
        path = "/offers/" + offer["id"]
        accepted = self.call(path + "/accept", {}, self.nana, "reward", expected=201)
        self.assertEqual(accepted["transaction"]["kind"], "ISSUE")
        self.assertEqual(self.balance("alice"), 20)
        self.assertEqual(self.get("/listings/" + listing["id"], self.alice)["status"], "ACTIVE")
        self.call(path + "/unaccept", {"reason": "Correction"}, self.nana, "reward-undo")
        self.assertEqual(self.balance("alice"), 0)

    def test_forex_bid_pays_cash_from_maker(self):
        self.issue("alice", 10000)
        self.call("/admin/issue-usd", dict(to=self.accounts["bob"], cents=250, reason="Bid cash"), self.nana, "cash", expected=201)
        quote = self.call("/quotes", dict(side="BID", cents_per_coin=250, coins=10000), self.bob, expected=201)
        self.call("/quotes/" + quote["id"] + "/take", {}, self.alice, "take-bid", expected=201)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 10000))
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.alice)["balance"], 250)
        self.assertEqual(self.get("/accounts/" + self.accounts["bob"] + "-usd", self.alice)["balance"], 0)

    def test_forex_failed_take_cancellation_and_member_limit(self):
        self.issue("alice", 10000)
        quotes = [self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=201) for _ in range(2)]
        limited = self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=507)
        self.assertEqual(limited["error"], "member_quote_limit")
        path = "/quotes/" + quotes[0]["id"]
        self.call(path + "/take", {}, self.bob, "unfunded-trade", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (10000, 0))
        self.assertEqual(self.get(path, self.alice)["status"], "OPEN")
        self.call(path + "/cancel", {}, self.bob, expected=403)
        self.call(path + "/cancel", {}, self.alice)
        self.call(path + "/take", {}, self.bob, "closed-trade", expected=409)

    def test_forex_book_ordering_and_exact_quote_amounts(self):
        quotes = [self.call("/quotes", dict(side=side, cents_per_coin=price, coins=10000), token, expected=201)
                  for token, side, price in [(self.alice, "ASK", 200), (self.alice, "ASK", 100),
                                             (self.bob, "BID", 300), (self.bob, "BID", 400),
                                             (self.nana, "ASK", 100)]]
        order = [quotes[i]["id"] for i in (1, 4, 0, 3, 2)]
        self.assertEqual([q["id"] for q in self.get("/quotes", self.alice)["quotes"]], order)
        for changes in [dict(coins=1), dict(coins=0), dict(cents_per_coin=0),
                        dict(coins=10**15, cents_per_coin=10**15)]:
            self.call("/quotes", dict(dict(side="ASK", cents_per_coin=1, coins=10000), **changes), self.nana, expected=400)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))
        self.restart()
        self.assertEqual([q["id"] for q in self.get("/quotes", self.alice)["quotes"]], order)

    def test_forex_book_capacity_and_closed_record_recycling(self):
        tokens = [self.nana, self.alice, self.bob] + [self.new_user(f"trader{i}") for i in range(5)]
        quotes = [self.call("/quotes", dict(side="ASK", cents_per_coin=100, coins=10000), token, expected=201)
                  for token in tokens for _ in range(2)]
        extra = self.new_user("extra")
        self.call("/quotes", dict(side="ASK", cents_per_coin=100, coins=10000), extra, expected=507)
        self.call("/quotes/" + quotes[0]["id"] + "/cancel", {}, self.nana)
        replacement = self.call("/quotes", dict(side="ASK", cents_per_coin=100, coins=10000), extra, expected=201)
        self.call("/quotes/" + quotes[0]["id"], token=self.alice, expected=404)
        expected = {q["id"] for q in quotes[1:]} | {replacement["id"]}
        self.restart()
        self.assertEqual({q["id"] for q in self.get("/quotes", self.alice)["quotes"]}, expected)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_clock_forex_expiry_boundary_releases_member_limit(self):
        deadline = self.now() + 10
        expiring = self.call("/quotes", dict(side="ASK", cents_per_coin=100, coins=10000, expires_at=deadline), self.alice, expected=201)
        self.call("/quotes", dict(side="ASK", cents_per_coin=200, coins=10000), self.alice, expected=201)
        path = "/quotes/" + expiring["id"]
        self.advance(9)
        self.assertTrue(self.get(path, self.alice)["live"])
        self.restart()
        self.advance(1)
        before = self.get("/activity", self.alice)["sequence"]
        quote = self.get(path, self.alice)
        self.assertEqual((quote["status"], quote["live"]), ("EXPIRED", False))
        self.assertEqual(self.get("/activity", self.alice)["sequence"], before)
        self.call(path + "/take", {}, self.bob, "expired-take", expected=409)
        self.call("/quotes", dict(side="ASK", cents_per_coin=100, coins=10000, expires_at=deadline), self.alice, expected=400)
        self.call("/quotes", dict(side="ASK", cents_per_coin=300, coins=10000), self.alice, expected=201)
        self.assertEqual(self.call(path + "/cancel", {}, self.nana)["status"], "CANCELLED")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_gift_contribution_close_refund_and_restart(self):
        self.issue("alice", 100)
        receipt = self.commerce({"create_request": dict(title="Supplies", description="Thanks", target=20, deadline=None)}, self.bob, "request")
        rid = receipt["sequence"]
        action = {"contribute": dict(request=rid, amount=30, memo="Gift")}
        paid = self.commerce(action, self.alice, "gift")
        self.assertEqual(self.commerce(action, self.alice, "gift")["sequence"], paid["sequence"])
        self.assertEqual(self.request_view(rid)["received"], 30)
        self.commerce({"close_request": {"request": rid}}, self.bob, "close")
        self.commerce(action, self.alice, "closed-gift", expected=409)
        tx = next(t for t in self.get("/transactions")["transactions"] if t["metadata"]["gift_request"] == rid)
        self.call("/transactions/" + tx["id"] + "/refund", {"amount": 10, "reason": "Partial gift"}, self.bob, "gift-refund")
        self.restart()
        self.assertEqual(self.request_view(rid)["received"], 20)
        self.assertTrue(self.request_view(rid)["closed"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 20))

    def test_gift_request_permissions_and_capacity(self):
        ids = []
        for i in range(32):
            ids.append(self.commerce({"create_request": dict(title=f"Request {i}", description="", target=None, deadline=None)}, self.bob, f"request-{i}")["sequence"])
        self.commerce({"close_request": {"request": ids[0]}}, self.alice, "wrong-close", expected=403)
        self.commerce({"contribute": dict(request=ids[0], amount=1, memo="Self")}, self.bob, "self-gift", expected=400)
        self.commerce({"create_request": dict(title="Overflow", description="", target=None, deadline=None)}, self.bob, "extra", expected=507)
        self.assertEqual({r["id"] for r in self.get("/commerce", self.nana)["requests"]}, set(ids))

    def test_clock_gift_deadline_boundary_and_refund_after_expiry(self):
        self.issue("alice", 100)
        deadline = self.now() + 10
        base = dict(title="Picnic", description="Supplies", target=5, deadline=deadline)
        for changes in [dict(title=" "), dict(target=0), dict(target=-1), dict(deadline=self.now())]:
            self.commerce({"create_request": dict(base, **changes)}, self.bob, "bad-" + next(iter(changes)), expected=400)
        rid = self.commerce({"create_request": base}, self.bob, "picnic")["sequence"]
        self.advance(9)
        self.commerce({"contribute": dict(request=rid, amount=20, memo="More than target")}, self.alice, "near-deadline")
        self.assertEqual(self.request_view(rid)["received"], 20)
        self.restart()
        self.advance(1)
        self.commerce({"contribute": dict(request=rid, amount=1, memo="Late")}, self.alice, "late-gift", expected=409)
        self.assertFalse(self.request_view(rid)["closed"])
        tx = next(t for t in self.get("/transactions")["transactions"] if t["metadata"]["gift_request"] == rid)
        self.call("/transactions/" + tx["id"] + "/refund", dict(amount=20, reason="Returned"), self.bob, "expired-refund")
        self.assertEqual(self.request_view(rid)["received"], 0)
        self.commerce({"close_request": dict(request=rid)}, self.bob, "close-expired")
        self.assertTrue(self.request_view(rid)["closed"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_gift_rejections_and_cumulative_refunds_preserve_net_total(self):
        self.issue("alice", 100)
        rid = self.commerce({"create_request": dict(title="Supplies", description="", target=None, deadline=None)}, self.bob, "supplies")["sequence"]
        action = {"contribute": dict(request=rid, amount=30, memo="Thanks")}
        self.commerce(action, self.alice, "first-gift")
        tx = next(t for t in self.get("/transactions")["transactions"] if t["metadata"]["gift_request"] == rid)
        self.commerce({"contribute": dict(request=rid, amount=100, memo="Too much")}, self.alice, "unfunded-gift", expected=409)
        self.assertEqual(self.request_view(rid)["received"], 30)
        self.call("/transactions/" + tx["id"] + "/refund", dict(amount=10, reason="Partial"), self.bob, "gift-part")
        self.call("/transactions/" + tx["id"] + "/refund", dict(amount=21, reason="Excess"), self.bob, "gift-excess", expected=409)
        self.assertEqual(self.request_view(rid)["received"], 20)
        self.restart()
        self.call("/transactions/" + tx["id"] + "/refund", dict(amount=20, reason="Rest"), self.bob, "gift-rest")
        self.assertEqual(self.request_view(rid)["received"], 0)
        self.commerce({"close_request": dict(request=rid)}, self.bob, "close")
        self.commerce({"close_request": dict(request=rid)}, self.bob, "second-close", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_gift_refunds_after_reform_use_current_currency_totals(self):
        self.issue("alice", 100)
        rid = self.commerce({"create_request": dict(title="Trip", description="", target=20, deadline=None)}, self.bob, "trip")["sequence"]
        self.commerce({"contribute": dict(request=rid, amount=30, memo="Gift")}, self.alice, "trip-gift")
        tx = next(t for t in self.get("/transactions")["transactions"] if t["metadata"]["gift_request"] == rid)
        meta = self.get("/loans", self.nana)
        changed = self.call("/admin/reform", dict(decimals=5, power=0, expected_epoch=meta["money_epoch"],
            expected_sequence=meta["sequence"], preview=False), self.nana, "gift-reform")
        self.assertEqual((self.request_view(rid)["target"], self.request_view(rid)["received"]), (200, 300))
        path = "/transactions/" + tx["id"] + "/refund"
        rejected = self.call(path, dict(amount=10, reason="Partial"), self.bob, "old-epoch-key", expected=409)
        self.assertEqual(rejected["error"], "stale_request")
        prefix = f'g{self.get("/status")["journal_generation"]}:m{changed["money_epoch"]}:'
        self.call(path, dict(amount=10, reason="Partial"), self.bob, prefix + "scaled-part")
        self.assertEqual(self.request_view(rid)["received"], 200)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (800, 200))
        self.restart()
        self.call(path, dict(amount=20, reason="Rest"), self.bob, prefix + "scaled-rest")
        self.assertEqual(self.request_view(rid)["received"], 0)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (1000, 0))

    def test_art_sale_is_atomic_and_checks_revision(self):
        self.issue("alice", 100)
        art = self.commerce({"mint_art": dict(title="Sunrise", license="Display", sha256="ab" * 32, locator="https://example.invalid/art.png")}, self.bob, "mint")["sequence"]
        self.commerce({"list_art": dict(art=art, price=30)}, self.bob, "art-list")
        view = self.artwork(art)
        action = {"buy_art": dict(art=art, expected_owner=view["owner"], expected_revision=view["revision"] - 1, expected_price=30)}
        self.commerce(action, self.alice, "stale-art", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))
        action["buy_art"]["expected_revision"] = view["revision"]
        bought = self.commerce(action, self.alice, "buy-art")
        self.assertEqual(self.commerce(action, self.alice, "buy-art")["sequence"], bought["sequence"])
        self.assertEqual(self.artwork(art)["owner"], int(self.accounts["alice"].removeprefix("account-")))
        self.assertIsNone(self.artwork(art)["price"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        tx = next(t for t in self.get("/transactions")["transactions"] if t["metadata"]["art"] == art)
        self.call("/transactions/" + tx["id"] + "/reverse", {"reason": "Money only"}, self.nana, "art-reverse", expected=409)
        self.restart()
        self.assertEqual(self.artwork(art)["sha256"], "ab" * 32)
        self.assertEqual(self.balance("alice"), 70)

    def test_art_equipping_gifting_and_validation(self):
        payload = dict(title="Art", license="Display", sha256="ab" * 32, locator="https://example.invalid/art.png")
        self.commerce({"mint_art": dict(payload, locator="http://example.invalid/art.png")}, self.bob, "insecure-art", expected=400)
        ids = [self.commerce({"mint_art": payload}, self.bob, f"mint-{i}")["sequence"] for i in range(2)]
        for i, art in enumerate(ids):
            self.commerce({"equip_art": dict(art=art, equipped=True)}, self.bob, f"equip-{i}")
        self.assertFalse(self.artwork(ids[0])["equipped"])
        self.assertTrue(self.artwork(ids[1])["equipped"])
        self.commerce({"gift_art": dict(art=ids[1], to=int(self.accounts["alice"].removeprefix("account-")))}, self.bob, "art-gift")
        self.assertFalse(self.artwork(ids[1])["equipped"])
        self.commerce({"list_art": dict(art=ids[1], price=10)}, self.bob, "old-owner", expected=403)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_art_metadata_rejections_do_not_create_editions(self):
        payload = dict(title="Art", license="Display", sha256="Ab" * 32, locator="https://example.invalid/art.png")
        for i, changes in enumerate([dict(title="\u3000"), dict(license=" "), dict(sha256="gh" * 32),
            dict(sha256="ab" * 31), dict(locator="https://"), dict(locator="https:///art"),
            dict(locator="https://?art"), dict(locator="https://#art"),
            dict(locator="https://name@example.invalid/art"), dict(locator="https://example.invalid/a b"),
            dict(locator="https://example.invalid/é")]):
            self.commerce({"mint_art": dict(payload, **changes)}, self.bob, f"bad-art-{i}", expected=400)
        self.assertEqual(self.get("/commerce", self.alice)["artworks"], [])
        art = self.commerce({"mint_art": payload}, self.bob, "valid-art")["sequence"]
        self.assertEqual(self.artwork(art)["sha256"], payload["sha256"])

    def test_art_capacity_retains_editions_and_survives_restart(self):
        payload = dict(title="Edition", license="Display", sha256="ab" * 32, locator="https://example.invalid/art.png")
        ids = [self.commerce({"mint_art": dict(payload, title=f"Edition {i}")}, self.bob, f"edition-{i}")["sequence"] for i in range(64)]
        self.commerce({"mint_art": payload}, self.bob, "art-full", expected=507)
        self.commerce({"gift_art": dict(art=ids[0], to=int(self.accounts["alice"].removeprefix("account-")))}, self.bob, "edition-gift")
        self.commerce({"equip_art": dict(art=ids[0], equipped=True)}, self.alice, "alice-edition")
        self.commerce({"equip_art": dict(art=ids[31], equipped=True)}, self.bob, "bob-first-chunk")
        self.commerce({"equip_art": dict(art=ids[63], equipped=True)}, self.bob, "bob-last-chunk")
        self.assertFalse(self.artwork(ids[31])["equipped"])
        self.assertTrue(self.artwork(ids[63])["equipped"])
        self.assertTrue(self.artwork(ids[0])["equipped"])
        self.restart()
        self.assertEqual({a["id"] for a in self.get("/commerce", self.alice)["artworks"]}, set(ids))
        self.assertEqual(self.artwork(ids[0])["owner"], int(self.accounts["alice"].removeprefix("account-")))
        self.assertTrue(self.artwork(ids[63])["equipped"])
        self.commerce({"mint_art": payload}, self.alice, "art-full-restart", expected=507)

    def test_competing_art_buyers_commit_one_payment_and_owner(self):
        self.issue("alice", 100)
        self.issue("nana", 100, "fund-nana")
        art = self.commerce({"mint_art": dict(title="Race", license="Display", sha256="ab" * 32,
            locator="https://example.invalid/art.png")}, self.bob, "race-art")["sequence"]
        self.commerce({"list_art": dict(art=art, price=30)}, self.bob, "race-list")
        view = self.artwork(art)
        action = {"buy_art": dict(art=art, expected_owner=view["owner"], expected_revision=view["revision"], expected_price=30)}
        buyers = [("alice", self.alice), ("nana", self.nana)]
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(lambda b: self.request("/commerce/commands", action, b[1], "race-" + b[0]), buyers))
        self.assertEqual(sorted(code for code, _ in results), [200, 409])
        winner = buyers[next(i for i, (code, _) in enumerate(results) if code == 200)][0]
        self.assertEqual(self.artwork(art)["owner"], int(self.accounts[winner].removeprefix("account-")))
        self.assertEqual((self.balance(winner), self.balance("bob")), (70, 30))
        self.assertIsNone(self.artwork(art)["price"])
        self.restart()
        self.assertEqual(self.artwork(art)["owner"], int(self.accounts[winner].removeprefix("account-")))
        payments = [t for t in self.get("/transactions")["transactions"] if t["metadata"]["art"] == art]
        self.assertEqual(len(payments), 1)
        self.assertEqual(self.get("/fulfillments", self.alice)["fulfillments"], [])

    def test_fulfillment_dispute_completion_and_reversal(self):
        self.issue("alice", 100)
        listing = self.listing(self.bob, kind="service")
        purchase = self.call("/listings/" + listing["id"] + "/purchase", {}, self.alice, "purchase", expected=201)
        tx = purchase["transaction"]["id"]
        path = "/transactions/" + tx + "/fulfillment"
        self.assertEqual(purchase["transaction"]["fulfillment"]["status"], "TODO")
        self.call(path, {"action": "DISPUTE", "reason": "Provider"}, self.bob, "bad-dispute", expected=403)
        self.assertEqual(self.call(path, {"action": "COMPLETE"}, self.bob, "complete")["status"], "DONE")
        disputed = self.call(path, {"action": "DISPUTE", "reason": "Incomplete"}, self.alice, "dispute")
        self.assertEqual(disputed["status"], "DISPUTED")
        self.assertEqual(self.call(path, {"action": "DISPUTE", "reason": "Incomplete"}, self.alice, "dispute"), disputed)
        self.assertEqual(self.call(path, {"action": "WITHDRAW_DISPUTE"}, self.alice, "withdraw-dispute")["status"], "DONE")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (70, 30))
        self.call("/transactions/" + tx + "/reverse", {"reason": "Correction"}, self.nana, "reverse-fulfilled", expected=201)
        self.call(path, {"action": "COMPLETE"}, self.bob, "after-reverse", expected=409)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))

    def test_fulfillment_book_parties_read_keys_and_cross_build_restart(self):
        carol = self.new_user("carol")
        outsider = self.new_user("dave")
        self.issue("alice", 100)
        transactions = []
        for index, seller in enumerate((self.bob, carol)):
            row = self.listing(seller, price=10, kind="service")
            purchase = self.call("/listings/" + row["id"] + "/purchase", {}, self.alice,
                f"private-obligation-{index}", expected=201)
            transactions.append(purchase["transaction"]["id"])
        self.call("/transactions/" + transactions[0] + "/fulfillment",
            {"action": "DISPUTE", "reason": "Delivery pending"}, self.alice, "book-dispute")
        views = {name: self.get("/fulfillments", token)["fulfillments"] for name, token in
            (("nana", self.nana), ("alice", self.alice), ("bob", self.bob), ("carol", carol), ("dave", outsider))}
        self.assertEqual({row["transaction"] for row in views["nana"]}, set(transactions))
        self.assertEqual(views["alice"], views["nana"])
        self.assertEqual([row["transaction"] for row in views["bob"]], transactions[:1])
        self.assertEqual([row["transaction"] for row in views["carol"]], transactions[1:])
        self.assertEqual(views["dave"], [])
        read = self.call("/me/api-key", {"password": "5678", "scope": "read"}, self.bob)["api_key"]
        self.assertEqual(self.get("/fulfillments", read)["fulfillments"], views["bob"])
        self.restart()
        carol = self.login("carol", "5678")
        outsider = self.login("dave", "5678")
        for name, token in (("nana", self.nana), ("alice", self.alice), ("bob", read), ("carol", carol), ("dave", outsider)):
            self.assertEqual(self.get("/fulfillments", token)["fulfillments"], views[name])
        self.assertEqual((self.balance("alice"), self.balance("bob"), self.balance("carol")), (80, 10, 10))

    def test_fulfillment_types_parties_and_partial_refunds(self):
        self.issue("alice", 200)
        expected = {}
        for kind, physical, fields in [
            ("item", "GOODS", {}),
            ("service", "WORK", {}),
            ("currency", "CASH", dict(currency="USD", minor_units=100)),
        ]:
            listing = self.listing(self.bob, price=10, kind=kind, **fields)
            purchase = self.call("/listings/" + listing["id"] + "/purchase", {}, self.alice, "buy-" + kind, expected=201)
            tx = purchase["transaction"]["id"]
            f = purchase["transaction"]["fulfillment"]
            self.assertEqual((f["kind"], f["status"], f["provider"], f["recipient"]),
                             (physical, "TODO", self.accounts["bob"], self.accounts["alice"]))
            expected[tx] = physical
        classified = self.call("/transfers", dict(to=self.accounts["bob"], amount=20,
            memo="Two hours", economic_kind="LABOR", quantity="2", unit="HOUR"), self.alice, "labor", expected=201)
        tx = classified["id"]
        expected[tx] = "WORK"
        self.transfer(self.alice, "bob", 10, "plain")
        self.transfer(self.alice, "bob", 0, "message")
        self.assertEqual({f["transaction"]: f["kind"] for f in self.get("/fulfillments", self.alice)["fulfillments"]}, expected)
        self.call("/transactions/" + tx + "/refund", dict(amount=5, reason="Part returned"), self.bob, "partial-labor")
        self.assertEqual(next(f for f in self.get("/fulfillments", self.alice)["fulfillments"] if f["transaction"] == tx)["status"], "TODO")
        self.restart()
        self.call("/transactions/" + tx + "/refund", dict(amount=15, reason="Rest returned"), self.bob, "rest-labor")
        f = next(f for f in self.get("/fulfillments", self.alice)["fulfillments"] if f["transaction"] == tx)
        self.assertEqual((f["status"], len(f["updates"])), ("REVERSED", 2))
        self.assertEqual((self.balance("alice"), self.balance("bob")), (160, 40))

    def test_fulfillment_capacity_keeps_recent_completed_obligations(self):
        self.issue("alice", 200)
        payload = dict(to=self.accounts["bob"], amount=1, memo="Work",
                       economic_kind="LABOR", quantity="1", unit="HOUR")
        payments = [self.call("/transfers", payload, self.alice, f"work-{i}", expected=201) for i in range(128)]
        first = payments[0]["id"]
        self.call("/transactions/" + first + "/fulfillment", {"action": "COMPLETE"}, self.bob, "done")
        self.call("/transfers", payload, self.alice, "capacity", expected=507)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (72, 128))
        self.restart()
        book = self.get("/fulfillments", self.alice)["fulfillments"]
        self.assertEqual(len(book), 128)
        self.assertEqual(next(f for f in book if f["transaction"] == first)["status"], "DONE")
        self.call("/transfers", payload, self.alice, "capacity-restart", expected=507)
        self.transfer(self.alice, "bob", 1, "ordinary-still-works")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (71, 129))

    def test_exact_currency_reform_preview_and_restart(self):
        issued = self.issue("alice", 100)
        listing = self.listing(self.bob)
        self.call("/admin/issue-usd", dict(to=self.accounts["alice"], cents=123, reason="USD"), self.nana, "usd", expected=201)
        meta = self.get("/loans", self.nana)
        request = dict(decimals=5, power=0, expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=True)
        preview = self.call("/admin/reform", request, self.nana)
        self.assertTrue(preview["preview"])
        self.assertEqual(self.balance("alice"), 100)
        changed = self.call("/admin/reform", dict(request, preview=False), self.nana, "reform")
        self.assertEqual(changed["money_epoch"], meta["money_epoch"] + 1)
        self.assertEqual(self.balance("alice"), 1000)
        self.assertEqual(self.get("/listings/" + listing["id"], self.alice)["price"], 300)
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.alice)["balance"], 123)
        tx = self.get("/transactions/" + issued["id"])
        self.assertEqual(tx["postings"][1]["amount"], 100)
        self.assertEqual(tx["current_postings"][1]["amount"], 1000)
        self.restart()
        self.assertEqual(self.get("/loans", self.alice)["decimals"], 5)
        self.assertEqual(self.balance("alice"), 1000)

    def test_inexact_and_stale_currency_reforms_do_not_mutate(self):
        self.issue("alice", 101)
        meta = self.get("/loans", self.nana)
        request = dict(decimals=3, power=0, expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=True)
        self.call("/admin/reform", request, self.nana, expected=409)
        self.issue("bob", 10, "intervening")
        self.call("/admin/reform", dict(request, decimals=5), self.nana, expected=409)
        self.assertEqual(self.get("/loans", self.alice)["money_epoch"], meta["money_epoch"])
        self.assertEqual((self.balance("alice"), self.balance("bob")), (101, 10))

    def test_reform_scales_pending_products_and_preserves_original_refund_units(self):
        self.issue("alice", 100000)
        self.call("/admin/issue-usd", dict(to=self.accounts["bob"], cents=500, reason="Cash"), self.nana, "usd", expected=201)
        loan = self.call("/loans", self.loan_terms(amount=1000, installment=100), self.alice, "loan")
        self.call(f"/loans/{loan['id']}/accept", {}, self.bob, "accept")
        draw = self.call("/lottos", dict(kind="SAVINGS", title="Pending savings", ticket_price=100,
            closes_at=self.now() + 3600, rate_bps=1000), self.nana, "draw")
        self.call(f"/lottos/{draw['id']}/tickets", {"count": 2}, self.alice, "tickets")
        quote = self.call("/quotes", dict(side="ASK", cents_per_coin=250, coins=10000), self.alice, expected=201)
        listing = self.listing(self.bob, price=30)
        rid = self.commerce({"create_request": dict(title="Trip", description="", target=100, deadline=None)}, self.bob, "trip")["sequence"]
        self.commerce({"contribute": dict(request=rid, amount=30, memo="Gift")}, self.alice, "gift")
        gift = next(t for t in self.get("/transactions")["transactions"] if t["metadata"]["gift_request"] == rid)
        art = self.commerce({"mint_art": dict(title="Art", license="Display", sha256="ab" * 32,
            locator="https://example.invalid/art.png")}, self.bob, "mint")["sequence"]
        self.commerce({"list_art": dict(art=art, price=50)}, self.bob, "art-list")
        work = self.call("/transfers", dict(to=self.accounts["bob"], amount=30, memo="Work",
            economic_kind="LABOR", quantity="1", unit="HOUR"), self.alice, "work", expected=201)
        self.call(f"/transactions/{work['id']}/refund", dict(amount=10, reason="Partial"), self.bob, "partial")
        before = (self.balance("alice"), self.balance("bob"))
        meta = self.get("/loans", self.nana)
        request = dict(decimals=5, power=0, expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=True)
        self.call("/admin/reform", request, self.alice, expected=403)
        self.call("/admin/reform", request, self.nana)
        self.assertEqual((self.balance("alice"), self.balance("bob")), before)
        changed = self.call("/admin/reform", dict(request, preview=False), self.nana, "reform")
        self.assertEqual((self.balance("alice"), self.balance("bob")), tuple(n * 10 for n in before))
        self.assertEqual(self.request_view(rid)["received"], 300)
        self.assertEqual(self.request_view(rid)["target"], 1000)
        self.assertEqual(self.artwork(art)["price"], 500)
        self.assertEqual(self.get("/listings/" + listing["id"], self.alice)["price"], 300)
        book = self.get("/lottos", self.alice)["lottos"][0]
        self.assertEqual((book["pool"], book["interest"], book["terms"]["ticket_price"], book["my_tickets"]), (2000, 200, 1000, 2))
        projected = next(l for l in self.get("/loans", self.bob)["loans"] if l["id"] == loan["id"])
        self.assertEqual((projected["amount"], projected["principal"], projected["installment"]), (10000, 10000, 1000))
        prefix = f'g{self.get("/status")["journal_generation"]}:m{changed["money_epoch"]}:'
        self.restart()
        self.call(f"/transactions/{work['id']}/refund", dict(amount=20, reason="Rest"), self.bob, prefix + "work-rest")
        self.call(f"/transactions/{gift['id']}/refund", dict(amount=30, reason="Returned"), self.bob, prefix + "gift-rest")
        self.assertEqual(self.request_view(rid)["received"], 0)
        traded = self.call("/quotes/" + quote["id"] + "/take", {}, self.bob, prefix + "trade", expected=201)
        self.assertIsInstance(traded, dict)
        self.assertEqual(self.get("/accounts/" + self.accounts["bob"] + "-usd", self.nana)["balance"], 250)
        self.assertEqual(self.get("/accounts/" + self.accounts["alice"] + "-usd", self.nana)["balance"], 250)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (888000, 110000))
        self.assertTrue(self.get("/status")["ledger_balanced"])
        self.restart()
        self.assertEqual((self.balance("alice"), self.balance("bob")), (888000, 110000))

    def test_reform_rejects_inconsistent_lotto_rounding_and_overflow_atomically(self):
        self.issue("alice", 100)
        draw = self.call("/lottos", dict(kind="SAVINGS", title="Fractional interest", ticket_price=100,
            closes_at=self.now() + 3600, rate_bps=1), self.nana, "draw")
        self.call(f"/lottos/{draw['id']}/tickets", {"count": 1}, self.alice, "tickets")
        before = self.get("/lottos", self.alice)
        meta = self.get("/loans", self.nana)
        request = dict(decimals=8, power=0, expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=True)
        # Recomputing the rounded interest in finer units would change the debt.
        self.call("/admin/reform", request, self.nana, expected=409)
        self.call("/admin/reform", dict(request, preview=False), self.nana, "bad-rounding", expected=409)
        self.assertEqual(self.get("/lottos", self.alice), before)
        self.issue("bob", 1000000000000000, "near-limit")
        meta = self.get("/loans", self.nana)
        request.update(decimals=4, power=-1, expected_sequence=meta["sequence"])
        self.call("/admin/reform", request, self.nana, expected=400)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 1000000000000000))
        self.assertTrue(self.get("/status")["ledger_balanced"])
        self.restart()
        self.assertEqual(self.get("/lottos", self.alice), before)

    def test_reform_parameter_bounds_and_epoch_capacity_survive_restart(self):
        meta = self.get("/loans", self.nana)
        base = dict(decimals=5, power=0, expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=True)
        for changes in ({"decimals": 9}, {"power": 13}, {"power": -13}, {"decimals": 4}):
            self.call("/admin/reform", dict(base, **changes), self.nana, expected=400)
        for index in range(31):
            meta = self.get("/loans", self.nana)
            request = dict(decimals=5 if index % 2 == 0 else 4, power=0,
                expected_epoch=meta["money_epoch"], expected_sequence=meta["sequence"], preview=False)
            prefix = f'g{self.get("/status")["journal_generation"]}:m{meta["money_epoch"]}:'
            self.call("/admin/reform", request, self.nana, prefix + "reform")
        self.restart()
        meta = self.get("/loans", self.nana)
        self.assertEqual(meta["money_epoch"], 31)
        request = dict(decimals=4, power=0, expected_epoch=31, expected_sequence=meta["sequence"], preview=True)
        self.call("/admin/reform", request, self.nana, expected=400)
        self.assertEqual(self.get("/loans", self.nana)["money_epoch"], 31)

    def test_loan_terms_and_lender_offer_limit(self):
        for i, changes in enumerate([dict(amount=0), dict(installment=0), dict(installment=41),
                                     dict(rate_days=2), dict(payment_days=365), dict(memo="Bad\nNote")]):
            self.call("/loans", self.loan_terms(**changes), self.alice, f"bad-loan-{i}", expected=400)
        loans = [self.call("/loans", self.loan_terms(), self.alice, f"offer-{i}") for i in range(4)]
        limited = self.call("/loans", self.loan_terms(), self.alice, "offer-limit", expected=507)
        self.assertEqual(limited["error"], "member_loan_limit")
        self.call("/loans/" + str(loans[0]["id"]) + "/close", {}, self.bob, "decline-slot")
        self.restart()
        new = self.call("/loans", self.loan_terms(), self.alice, "replacement-offer")
        self.assertEqual(new["status"], "OFFERED")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_loan_requests_responses_and_borrower_identity(self):
        outsider = self.new_user("observer")
        terms = self.loan_terms(borrower="alice")
        self.call("/loans/request", terms, self.bob, "wrong-requester", expected=403)
        request = self.call("/loans/request", terms, self.alice, "application")
        self.assertEqual(request["status"], "REQUESTED")
        self.assertIn(request["id"], {l["id"] for l in self.get("/loans", outsider)["loans"]})
        path = "/loans/" + str(request["id"])
        self.call(path + "/offer", self.loan_terms(), self.bob, "wrong-borrower", expected=400)
        self.call(path + "/offer", terms, self.alice, "self-lender", expected=400)
        offer = self.call(path + "/offer", terms, self.bob, "respond")
        self.assertEqual((offer["status"], offer["borrower"], offer["lender"]),
                         ("OFFERED", self.accounts["alice"], self.accounts["bob"]))
        self.call(path + "/offer", terms, self.bob, "second-response", expected=409)
        self.restart()
        self.assertEqual(self.get("/loans", self.nana)["loans"][0]["status"], "OFFERED")
        self.call(path + "/close", {}, self.alice, "decline-application")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_credit_loan_waits_for_zero_balance_before_funding(self):
        self.issue("alice", 100)
        self.issue("bob", 20, "nonzero-borrower")
        loan = self.call("/loans", self.loan_terms(credit=True), self.alice, "credit")
        path = "/loans/" + str(loan["id"])
        accepted = self.call(path + "/accept", {}, self.bob, "arm-credit")
        self.assertEqual(accepted["status"], "ARMED")
        self.assertEqual(accepted["waiting_reason"], "Waiting for a zero balance")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 20))
        self.restart()
        self.transfer(self.bob, "alice", 20, "empty-wallet")
        read = lambda: next(l for l in self.get("/loans", self.bob)["loans"] if l["id"] == loan["id"])
        self.wait_for(read, lambda l: l["status"] == "ACTIVE")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (80, 40))
        self.assertEqual(self.call(path + "/repay", {"amount": 40}, self.bob, "credit-payoff")["status"], "PAID")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (120, 0))

    def test_credit_source_policy_member_32_usd_messages_and_cross_build_restart(self):
        for index in range(4, 33):
            last = self.call("/users", dict(username=f"member{index}", display_name=f"Member {index}",
                password="5678", grant=False), self.nana, expected=201)
        self.users["member32"] = last
        self.accounts["member32"] = last["account"]
        borrower = self.login("member32", "5678")
        self.issue("alice", 200)
        original = self.call("/loans", self.loan_terms(borrower="member32"), self.alice, "original")
        original_path = "/loans/" + str(original["id"])
        self.call(original_path + "/accept", {}, borrower, "original-fund")
        self.call(original_path + "/repay", dict(amount=40), borrower, "original-payoff")
        credit = self.call("/loans", self.loan_terms(borrower="member32", amount=30, credit=True),
                           self.alice, "credit")
        path = "/loans/" + str(credit["id"])
        self.call(path + "/accept", {}, borrower, "arm")
        def read():
            return next(l for l in self.get("/loans", self.alice)["loans"] if l["id"] == credit["id"])
        self.assertEqual(read()["waiting_reason"], "Credit does not fund loan payments")
        self.restart()
        borrower = self.login("member32", "5678")
        self.call("/admin/issue-usd", dict(to=last["account"], cents=5, reason="Cash"),
                  self.nana, "cash", expected=201)
        self.call("/transfers", dict(to=self.accounts["alice"], amount=0, memo="A private note"),
                  borrower, "note", expected=201)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            row = read()
            self.assertEqual((row["status"], row["waiting_reason"]),
                             ("ARMED", "Credit does not fund loan payments"))
            time.sleep(.1)
        self.issue("member32", 1, "coin")
        self.assertEqual(read()["waiting_reason"], "Waiting for a zero balance")
        self.call("/transfers", dict(to=self.accounts["alice"], amount=1, memo="Ordinary NC"),
                  borrower, "clear", expected=201)
        self.wait_for(read, lambda l: l["status"] == "ACTIVE")
        self.assertEqual((self.balance("alice"), self.balance("member32")), (171, 30))
        self.call(path + "/repay", dict(amount=30), borrower, "credit-payoff")
        self.restart()
        self.assertEqual(read()["status"], "PAID")
        self.assertEqual((self.balance("alice"), self.balance("member32")), (201, 0))
        self.assertEqual(self.get("/accounts/" + last["account"] + "-usd", self.nana)["balance"], 5)
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_loan_capacity_recycles_oldest_terminal_record(self):
        terms = self.loan_terms(borrower="alice")
        requests = [self.call("/loans/request", terms, self.alice, f"application-{i}") for i in range(32)]
        self.call("/loans/request", self.loan_terms(), self.bob, "full-applications", expected=507)
        self.call("/loans/" + str(requests[0]["id"]) + "/close", {}, self.alice, "cancel-application")
        replacement = self.call("/loans/request", self.loan_terms(), self.bob, "new-application")
        expected = {l["id"] for l in requests[1:]} | {replacement["id"]}
        self.assertEqual({l["id"] for l in self.get("/loans", self.nana)["loans"]}, expected)
        self.restart()
        self.assertEqual({l["id"] for l in self.get("/loans", self.nana)["loans"]}, expected)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 0))

    def test_loan_book_summary_uses_active_principal_weights(self):
        self.issue("alice", 10000)
        self.issue("bob", 30000, "fund-bob")
        first = self.call("/loans", self.loan_terms(amount=10000, rate_bps=100,
            rate_days=365, installment=1000), self.alice, "annual-one")
        second = self.call("/loans", self.loan_terms(borrower="alice", amount=30000,
            rate_bps=300, rate_days=365, installment=1000), self.bob, "annual-three")
        self.call("/loans/" + str(first["id"]) + "/accept", {}, self.bob, "accept-one")
        self.call("/loans/" + str(second["id"]) + "/accept", {}, self.alice, "accept-three")
        summary = self.get("/loans", self.alice)["summary"]
        self.assertEqual((summary["outstanding"], summary["overdue"], summary["active"]), ("40000", "0", 2))
        self.assertAlmostEqual(summary["weighted_annual_percent"], 2.5, places=10)
        self.call("/loans/" + str(first["id"]) + "/repay", {"amount": 10000}, self.bob, "pay-one")
        self.restart()
        summary = self.get("/loans", self.alice)["summary"]
        self.assertEqual((summary["outstanding"], summary["active"]), ("30000", 1))
        self.assertAlmostEqual(summary["weighted_annual_percent"], 3.0, places=10)

    def test_credit_loan_waiting_reason_tracks_lender_funds(self):
        loan = self.call("/loans", self.loan_terms(credit=True), self.alice, "unfunded-credit")
        self.call("/loans/" + str(loan["id"]) + "/accept", {}, self.bob, "arm-unfunded")
        read = lambda: next(l for l in self.get("/loans", self.bob)["loans"] if l["id"] == loan["id"])
        self.assertEqual(read()["waiting_reason"], "Waiting for lender funds")
        self.restart()
        self.assertEqual(read()["waiting_reason"], "Waiting for lender funds")
        self.issue("alice", 40)
        funded = self.wait_for(read, lambda l: l["status"] == "ACTIVE")
        self.assertEqual(funded["waiting_reason"], "")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (0, 40))

    def test_loan_funding_permissions_privacy_and_closure(self):
        outsider = self.new_user("carol")
        loan = self.call("/loans", self.loan_terms(), self.alice, "loan")
        path = "/loans/" + str(loan["id"])
        self.call(path + "/accept", {}, self.alice, "wrong-borrower", expected=403)
        self.call(path + "/accept", {}, self.bob, "no-funds", expected=409)
        self.assertEqual(self.get("/loans", outsider)["loans"], [])
        self.issue("alice", 100)
        self.call(path + "/accept", {}, self.bob, "no-funds")
        profile = self.get("/loans?member=" + self.users["bob"]["id"], outsider)
        self.assertEqual(profile["loans"][0]["memo"], "")
        self.call(path + "/repay", {"amount": 10}, self.alice, "lender-repay", expected=403)
        other = self.call("/loans", self.loan_terms(), self.alice, "other-loan")
        self.assertEqual(self.call("/loans/" + str(other["id"]) + "/close", {}, self.bob, "close-loan")["status"], "DECLINED")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (60, 40))

    def test_disabled_lender_pauses_settlement_and_failed_key_recovers_across_build(self):
        self.issue("alice", 100)
        loan = self.call("/loans", self.loan_terms(), self.alice, "pause-loan")
        path = "/loans/" + str(loan["id"])
        self.call(path + "/accept", {}, self.bob, "pause-accept")
        user_path = "/users/" + self.users["alice"]["id"]
        self.call(user_path, {"status": "DISABLED"}, self.nana, method="PATCH")
        for amount, key in [(0,"disabled-zero"),(10,"paused-payment")]:
            rejected = self.call(path + "/repay", {"amount": amount}, self.bob, key, expected=403)
            self.assertEqual(rejected["error"], "disabled")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (60,40))
        self.stop()
        self.start(RESTART_SERVER or SERVER)
        self.nana = self.login("nana","1234")
        self.bob = self.login("bob","5678")
        row = next(l for l in self.get("/loans",self.nana)["loans"] if l["id"]==loan["id"])
        self.assertEqual((row["status"],row["principal"],row["waiting_reason"]), ("ACTIVE",40,"An account is disabled"))
        self.call(path + "/repay", {"amount":10}, self.bob, "paused-payment", expected=403)
        self.call(user_path, {"status":"ACTIVE"}, self.nana, method="PATCH")
        self.alice = self.login("alice","5678")
        paid = self.call(path + "/repay", {"amount":10}, self.bob, "paused-payment")
        self.assertEqual(paid["principal"],30)
        self.restart()
        self.assertEqual(self.call(path + "/repay", {"amount":10}, self.bob, "paused-payment"),paid)
        done = self.call(path + "/repay", {"amount":30}, self.bob, "pause-payoff")
        self.assertEqual((done["status"],done["principal"],done["interest"]),("PAID",0,0))
        self.assertEqual((self.balance("alice"),self.balance("bob")),(100,0))
        self.assertTrue(self.get("/status")["ledger_balanced"])

    def test_lotto_ticket_escrow_receipts_and_restart(self):
        self.issue("alice", 100)
        terms = dict(kind="SIMPLE", title="Draw", ticket_price=10, closes_at=self.now() + 3600, rate_bps=0)
        self.call("/lottos", terms, self.alice, "unauthorized-lotto", expected=403)
        lotto = self.call("/lottos", terms, self.nana, "draw")
        path = "/lottos/" + str(lotto["id"]) + "/tickets"
        self.call(path, {"count": 0}, self.alice, "zero-ticket", expected=400)
        bought = self.call(path, {"count": 3}, self.alice, "tickets")
        self.assertEqual(self.call(path, {"count": 3}, self.alice, "tickets"), bought)
        self.assertEqual((bought["pool"], bought["tickets"], bought["my_tickets"]), (30, 3, 3))
        self.assertEqual(self.balance("alice"), 70)
        self.restart()
        self.assertEqual(self.call(path, {"count": 3}, self.alice, "tickets"), bought)
        self.assertEqual(self.balance("alice"), 70)

    def test_simple_lotto_settles_once_and_cannot_sell_late_tickets(self):
        self.issue("alice", 100)
        lotto = self.call("/lottos", dict(kind="SIMPLE", title="Quick draw", ticket_price=10,
            closes_at=self.now() + 3, rate_bps=0), self.nana, "draw")
        path = "/lottos/" + str(lotto["id"]) + "/tickets"
        self.call(path, {"count": 3}, self.alice, "tickets")
        if CLOCK:
            self.advance(4)
        settled = self.wait_for(lambda: self.get("/lottos", self.alice)["lottos"][0],
            lambda v: v["status"] == "SETTLED", timeout=50)
        self.assertEqual(settled["winner"], self.accounts["alice"])
        self.assertEqual(self.balance("alice"), 100)
        self.call(path, {"count": 1}, self.alice, "late-tickets", expected=409)
        self.restart()
        self.assertEqual(self.get("/lottos", self.alice)["lottos"][0]["winner"], self.accounts["alice"])
        self.assertEqual(self.balance("alice"), 100)

    def test_lotto_creation_bounds_house_restriction_and_atomic_purchase(self):
        terms = dict(kind="SIMPLE", title="Bounded draw", ticket_price=10,
            closes_at=self.now() + 3600, rate_bps=0)
        for index, fields in enumerate(({"title": " "}, {"title": "Bad\nname"},
                {"ticket_price": 0}, {"ticket_price": 1000000000000001},
                {"closes_at": self.now()}, {"rate_bps": 1},
                {"kind": "SAVINGS", "rate_bps": 10001})):
            self.call("/lottos", dict(terms, **fields), self.nana, f"bad-draw-{index}", expected=400)
        draw = self.call("/lottos", terms, self.nana, "draw")
        path = f"/lottos/{draw['id']}/tickets"
        self.call(path, {"count": 1}, self.nana, "house-ticket", expected=403)
        self.call(path, {"count": 1}, self.alice, "unfunded", expected=409)
        unchanged = self.get("/lottos", self.alice)["lottos"][0]
        self.assertEqual((unchanged["pool"], unchanged["tickets"], unchanged["my_tickets"]), (0, 0, 0))
        self.issue("alice", 20)
        bought = self.call(path, {"count": 2}, self.alice, "unfunded")
        self.assertEqual((bought["pool"], bought["my_tickets"]), (20, 2))
        # The bounded book rejects a seventeenth live pool without changing cash.
        for index in range(15):
            self.call("/lottos", terms, self.nana, f"draw-{index}")
        self.call("/lottos", terms, self.nana, "full", expected=507)
        self.assertEqual(len(self.get("/lottos", self.alice)["lottos"]), 16)
        self.assertEqual(self.balance("alice"), 0)
        self.restart()
        self.assertEqual(self.call(path, {"count": 2}, self.alice, "unfunded"), bought)

    def test_clock_lotto_member_32_wins_and_receipt_survives_settlement(self):
        for index in range(4, 33):
            token = self.new_user(f"member{index}")
        name = "member32"
        self.issue(name, 100)
        draw = self.call("/lottos", dict(kind="SIMPLE", title="Last member wins",
            ticket_price=10, closes_at=self.now() + 100, rate_bps=0), self.nana, "draw")
        path = f"/lottos/{draw['id']}/tickets"
        bought = self.call(path, {"count": 3}, token, "tickets")
        self.assertEqual(self.balance(name), 70)
        self.restart()
        token = self.login(name, "5678")
        self.assertEqual(self.call(path, {"count": 3}, token, "tickets"), bought)
        self.advance(100)
        paying = self.wait_for(lambda: self.get("/lottos", self.nana)["lottos"][0],
            lambda row: row["status"] == "PAYING")
        self.assertEqual(paying["winner"], self.accounts[name])
        self.restart()
        settled = self.wait_for(lambda: self.get("/lottos", self.nana)["lottos"][0],
            lambda row: row["status"] == "SETTLED", timeout=50)
        self.assertEqual(settled["winner"], self.accounts[name])
        self.assertEqual(self.balance(name), 100)
        self.assertTrue(self.get("/status")["ledger_balanced"])
        self.restart()
        token = self.login(name, "5678")
        replayed = self.call(path, {"count": 3}, token, "tickets")
        self.assertEqual((replayed["id"], replayed["tickets"], replayed["my_tickets"], replayed["pool"]),
            (draw["id"], 3, 3, 30))
        self.assertEqual((replayed["status"], replayed["winner"]), ("SETTLED", self.accounts[name]))
        self.call(path, {"count": 1}, token, "late", expected=409)
        self.assertEqual(self.balance(name), 100)

    def test_clock_savings_lotto_returns_all_principal_and_issues_only_shortfall(self):
        for index in range(4, 33):
            token = self.new_user(f"member{index}")
        self.issue("alice", 1000)
        self.issue("member32", 1000, "high-seed")
        self.issue("nana", 7, "house-seed")
        self.assertEqual(self.get("/status")["circulation"], 2007)
        closes = self.now() + 100
        draw = self.call("/lottos", dict(kind="SAVINGS", title="All savings returned",
            ticket_price=100, closes_at=closes, rate_bps=1000), self.nana, "draw")
        path = f"/lottos/{draw['id']}/tickets"
        self.call(path, {"count": 1}, self.alice, "low-tickets")
        self.call(path, {"count": 2}, token, "high-tickets")
        self.advance(100)
        waiting = self.get("/lottos", self.nana)["lottos"][0]
        self.assertEqual((waiting["status"], waiting["interest"], waiting["due_at"]),
            ("WAITING", 30, closes + 30 * 86400))
        self.call(path, {"count": 1}, self.alice, "late", expected=409)
        self.restart()
        self.advance(30 * 86400 - 1)
        self.assertEqual(self.get("/lottos", self.nana)["lottos"][0]["status"], "WAITING")
        self.assertEqual((self.balance("alice"), self.balance("member32")), (900, 800))
        self.advance(1)
        self.wait_for(lambda: (self.get("/lottos", self.nana)["lottos"][0], self.balance("alice")),
            lambda state: state[0]["status"] == "PAYING" and state[1] == 1000)
        self.assertEqual(self.balance("member32"), 800)
        self.restart()
        settled = self.wait_for(lambda: self.get("/lottos", self.nana)["lottos"][0],
            lambda row: row["status"] == "SETTLED", timeout=50)
        winner = settled["winner"]
        self.assertIn(winner, (self.accounts["alice"], self.accounts["member32"]))
        expected = tuple(1030 if self.accounts[name] == winner else 1000
            for name in ("alice", "member32"))
        self.assertEqual((self.balance("alice"), self.balance("member32")), expected)
        self.assertEqual(self.balance("nana"), 0)
        status = self.get("/status")
        self.assertEqual(status["circulation"], 2030)
        self.assertTrue(status["ledger_balanced"])
        self.restart()
        self.assertEqual((self.balance("alice"), self.balance("member32")), expected)
        self.assertEqual(self.get("/status")["circulation"], 2030)
        self.assertEqual(self.get("/lottos", self.nana)["lottos"][0]["winner"], winner)

    def test_clock_delayed_and_empty_lottos_observe_due_date_and_recycle(self):
        self.issue("alice", 1000)
        closes = self.now() + 100
        terms = dict(kind="DELAYED", title="Delayed draw", ticket_price=100,
            closes_at=closes, rate_bps=1000)
        draw = self.call("/lottos", terms, self.nana, "draw")
        self.call(f"/lottos/{draw['id']}/tickets", {"count": 3}, self.alice, "tickets")
        for index in range(15):
            self.call("/lottos", dict(terms, kind="SIMPLE", rate_bps=0), self.nana, f"empty-{index}")
        self.advance(100)
        self.wait_for(lambda: self.get("/lottos", self.nana)["lottos"],
            lambda rows: sum(row["status"] == "SETTLED" for row in rows) == 15, timeout=15)
        self.assertEqual(self.balance("alice"), 700)
        self.call("/lottos", dict(terms, closes_at=self.now() + 3600), self.nana, "recycled")
        self.restart()
        self.advance(30 * 86400)
        settled = self.wait_for(lambda: next(row for row in self.get("/lottos", self.nana)["lottos"]
            if row["id"] == draw["id"]), lambda row: row["status"] == "SETTLED", timeout=50)
        self.assertEqual(settled["winner"], self.accounts["alice"])
        self.assertEqual(self.balance("alice"), 1030)
        self.restart()
        self.assertEqual(self.balance("alice"), 1030)

    def test_clock_combined_book_capacities_and_reform(self):
        import capacity
        capacity.exercise(self, self.now())
        history = capacity.saturate_history(self)
        paths = ("/users", "/quotes", "/commerce", "/loans", "/lottos",
                 "/listings", "/offers", "/fulfillments",
                 "/accounts/" + self.accounts["alice"], "/transactions?limit=200")
        before = {path: self.get(path, self.nana) for path in paths}
        self.restart()
        self.assertEqual({path: self.get(path, self.nana) for path in paths}, before)
        status = self.get("/status")
        self.assertEqual(status["retained_transactions"], history["retained_transactions"])
        self.assertEqual(status["transactions"], history["total_transactions"])
        self.assertTrue(status["ledger_balanced"])

    def test_clock_loan_interest_payment_and_cross_build_restart(self):
        if not CLOCK:
            self.skipTest("requires the launcher clock (--clock)")
        self.issue("alice", 20000)
        loan = self.call("/loans", self.loan_terms(amount=10000, rate_bps=100,
            rate_days=1, installment=1000), self.alice, "interest-loan")
        path = "/loans/" + str(loan["id"])
        self.call(path + "/accept", {}, self.bob, "accept-interest")
        self.advance(86400)
        view = next(l for l in self.get("/loans", self.bob)["loans"] if l["id"] == loan["id"])
        self.assertEqual((view["principal"], view["interest"]), (10000, 100))
        paid = self.call(path + "/repay", {"amount": 50}, self.bob, "interest-first")
        self.assertEqual((paid["principal"], paid["interest"]), (10000, 50))
        self.restart()
        self.assertEqual(self.call(path + "/repay", {"amount": 50}, self.bob, "interest-first"), paid)
        self.issue("bob", 100, "fund-interest")
        done = self.call(path + "/repay", {"amount": 10050}, self.bob, "payoff")
        self.assertEqual((done["status"], done["principal"], done["interest"]), ("PAID", 0, 0))
        self.assertEqual((self.balance("alice"), self.balance("bob")), (20100, 0))

    def test_clock_scheduled_loan_catches_up_once_across_restart(self):
        if not CLOCK:
            self.skipTest("requires the launcher clock (--clock)")
        self.issue("alice", 100)
        loan = self.call("/loans", self.loan_terms(payment_days=1), self.alice, "scheduled")
        path = "/loans/" + str(loan["id"])
        self.call(path + "/accept", {}, self.bob, "accept-scheduled")
        self.advance(3 * 86400)
        read = lambda: next(l for l in self.get("/loans", self.bob)["loans"] if l["id"] == loan["id"])
        caught = self.wait_for(read, lambda l: l["principal"] == 10)
        self.assertEqual(caught["overdue"], 0)
        self.assertEqual((self.balance("alice"), self.balance("bob")), (90, 10))
        self.restart()
        self.assertEqual(read()["principal"], 10)
        self.advance(86400)
        self.wait_for(read, lambda l: l["status"] == "PAID")
        self.assertEqual((self.balance("alice"), self.balance("bob")), (100, 0))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", required=True, help="Desktop server executable; never a board URL")
    parser.add_argument("--server-arg", action="append", default=[])
    parser.add_argument("--restart-server", help="Use another build when reopening restart fixtures")
    parser.add_argument("--test", help="Run one named contract")
    parser.add_argument("--xml", help="Write a small JUnit report for CI")
    parser.add_argument("--clock", action="store_true", help="Enable deterministic launcher-clock contracts; build with conformance-clock")
    args = parser.parse_args()
    SERVER = str(Path(args.server).resolve())
    SERVER_ARGS = args.server_arg
    CLOCK = args.clock
    RESTART_SERVER = str(Path(args.restart_server).resolve()) if args.restart_server else None
    suite = (unittest.TestSuite([BankContract(args.test)]) if args.test else
             unittest.defaultTestLoader.loadTestsFromTestCase(BankContract))
    tests = list(suite)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if args.xml:
        import xml.etree.ElementTree as ET
        root = ET.Element("testsuite", name="nanacoin-json-v1", tests=str(result.testsRun),
                          failures=str(len(result.failures)), errors=str(len(result.errors)), skipped=str(len(result.skipped)))
        failures = dict(result.failures)
        errors = dict(result.errors)
        skipped = dict(result.skipped)
        for test in tests:
            node = ET.SubElement(root, "testcase", name=str(test))
            if test in failures:
                ET.SubElement(node, "failure").text = failures[test]
            if test in errors:
                ET.SubElement(node, "error").text = errors[test]
            if test in skipped:
                ET.SubElement(node, "skipped").text = skipped[test]
        ET.ElementTree(root).write(args.xml, encoding="utf-8", xml_declaration=True)
    raise SystemExit(0 if result.wasSuccessful() else 1)
