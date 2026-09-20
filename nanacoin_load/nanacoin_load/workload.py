"""Locust imports this after gevent monkey-patching; don't import from the CLI."""

import os
import time
import uuid
from pathlib import Path

import gevent
import requests
from locust import HttpUser, LoadTestShape, between, events, task

from .common import MONITOR_PATH, ORIGIN, VERIFY, Evidence, credentials, health, load_fixture, pkce

SCENARIO = os.getenv("NANA_SCENARIO", "status")
RUN = Path(os.environ["NANA_RUN"])
TRACE = Evidence(RUN)
FIXTURE = None
USER_INDEX = 0
STOPPING = False


def stop(environment, reason):
    global STOPPING
    if STOPPING:
        return
    STOPPING = True
    TRACE.emit("anomaly", reason=reason, users=environment.runner.user_count)
    environment.process_exit_code = 2
    # The shape owns shutdown. Calling runner.quit here races Locust's own
    # shape-completion shutdown and can attempt to stop each User twice.


@events.request.add_listener
def request_event(
    request_type, name, response_time, response_length, response=None, exception=None, context=None, **kw
):
    TRACE.emit(
        "request",
        role="load",
        method=request_type,
        name=name,
        ms=response_time,
        size=response_length,
        status=getattr(response, "status_code", 0),
        error=str(exception)[:160] if exception else None,
        transport_error=type(response.error).__name__ if getattr(response, "error", None) else None,
        health=health(response.headers.get("X-Nanacoin-Health")) if response is not None else {},
    )


def monitor(environment):
    """Independent, low-rate diagnostic traffic, excluded from Locust workload statistics."""
    session = requests.Session()
    session.trust_env = False
    session.verify = VERIFY
    last_uptime = None
    misses = 0
    while not STOPPING:
        started = time.monotonic()
        try:
            r = session.get(environment.host + MONITOR_PATH, timeout=(3, 5))
            r.raise_for_status()
            data = r.json()
            uptime = data.get("uptime_seconds")
            TRACE.emit(
                "snapshot",
                role="monitor",
                users=environment.runner.user_count,
                ms=(time.monotonic() - started) * 1000,
                data=data,
            )
            misses = 0
            if uptime is not None:
                if last_uptime is not None and uptime < last_uptime:
                    stop(environment, "uptime decreased: reboot observed")
                last_uptime = uptime
            elif data.get("ledger_balanced") is False:
                stop(environment, "ledger invariant failed in monitor probe")
        except (requests.RequestException, ValueError, KeyError) as exc:
            misses += 1
            TRACE.emit(
                "monitor_failure",
                consecutive=misses,
                error=type(exc).__name__,
                ms=(time.monotonic() - started) * 1000,
                users=environment.runner.user_count,
            )
            if misses >= 3:
                stop(environment, "three failed diagnostic probes: unresponsive, crash not yet confirmed")
        gevent.sleep(float(os.getenv("NANA_MONITOR_INTERVAL", "5")))


@events.test_start.add_listener
def start(environment, **kw):
    global FIXTURE, STOPPING, USER_INDEX
    STOPPING = False
    USER_INDEX = 0
    if SCENARIO != "status":
        try:
            FIXTURE = load_fixture(environment.host)
        except OSError, ValueError, KeyError:
            stop(environment, "missing or mismatched fixture: run prepare")
            return
    TRACE.emit("start", scenario=SCENARIO, host=environment.host)
    environment.nana_monitor = gevent.spawn(monitor, environment)


@events.test_stop.add_listener
def end(environment, **kw):
    global STOPPING
    STOPPING = True
    worker = getattr(environment, "nana_monitor", None)
    if worker:
        worker.kill()
    TRACE.emit(
        "stop", requests=environment.stats.total.num_requests, failures=environment.stats.total.num_failures
    )


class Stages(LoadTestShape):
    use_common_options = True

    def reset_time(self):
        global STOPPING
        STOPPING = False
        self.previous = None
        super().reset_time()

    def tick(self):
        if STOPPING:
            return None
        stage_seconds = int(os.getenv("NANA_STAGE", "30"))
        steps = [int(x) for x in os.environ.get("NANA_STEPS", "").split(",") if x]
        elapsed = self.get_run_time()
        if steps:
            index = int(elapsed // stage_seconds)
            if index >= len(steps):
                return None
            users = steps[index]
        else:
            if elapsed >= int(os.getenv("NANA_DURATION", "60")):
                return None
            users = int(os.getenv("NANA_USERS", "1"))
        if getattr(self, "previous", None) != users:
            TRACE.emit("stage", users=users, scenario=SCENARIO)
            self.previous = users
        return users, max(1, users)


class NanaUser(HttpUser):
    wait_time = between(float(os.getenv("NANA_WAIT_MIN", ".3")), float(os.getenv("NANA_WAIT_MAX", "1")))

    def on_start(self):
        global USER_INDEX
        self.client.trust_env = False
        self.client.verify = VERIFY
        self.client.headers.update({"Origin": ORIGIN})
        self.identity = None
        if FIXTURE:
            self.identity = FIXTURE["members"][USER_INDEX % len(FIXTURE["members"])]
            USER_INDEX += 1

    def call(self, method, path, body=None, identity=None, expected=200, key=None, name=None):
        headers = {}
        identity = identity or self.identity
        if identity:
            headers["Authorization"] = "Bearer " + identity["token"]
        if key:
            headers["Idempotency-Key"] = key
        label = name or path
        TRACE.emit("request_start", role="load", name=label, method=method)
        started = time.monotonic()

        def received_headers(response, **kwargs):
            TRACE.emit(
                "response_headers",
                role="load",
                name=label,
                status=response.status_code,
                ms=(time.monotonic() - started) * 1000,
                health=health(response.headers.get("X-Nanacoin-Health")),
            )

        with self.client.request(
            method,
            path,
            json=body,
            headers=headers,
            name=label,
            timeout=(float(os.getenv("NANA_CONNECT_TIMEOUT", "4")),
                     float(os.getenv("NANA_READ_TIMEOUT", "10"))),
            catch_response=True,
            hooks={"response": received_headers},
        ) as response:
            if response.status_code != expected:
                response.failure(f"HTTP {response.status_code}, expected {expected}")
                if response.status_code == 401:
                    stop(self.environment, "session rejected: fixture stale or board rebooted; run prepare")
                return None
            if expected == 204:
                return {}
            try:
                data = response.json()
                if not isinstance(data, dict):
                    raise TypeError("not an object")
                if path.endswith("/status") and data.get("ledger_balanced") is not True:
                    response.failure("ledger invariant failed")
                    stop(self.environment, "ledger invariant failed")
                return data
            except ValueError, TypeError:
                response.failure("invalid JSON response")
                return None

    @task
    def journey(self):
        if STOPPING:
            gevent.sleep(1)
            return
        if SCENARIO == "status":
            self.call("GET", "/api/v1/status")
        elif SCENARIO == "browse":
            # Match the SPA's Promise.all burst; this is intentionally not client-side throttled.
            account = self.identity["user"]["account"]
            paths = [
                "/api/v1/me",
                "/api/v1/users",
                "/api/v1/listings",
                "/api/v1/status",
                f"/api/v1/accounts/{account}/transactions?limit=30",
            ]
            jobs = [
                gevent.spawn(self.call, "GET", path, name="history-30" if "/accounts/" in path else path)
                for path in paths
            ]
            gevent.joinall(jobs, raise_error=True)
        elif SCENARIO == "ledger":
            self.call("GET", "/api/v1/transactions?limit=30", identity=FIXTURE["nana"])
        elif SCENARIO in ("write", "replay"):
            key = uuid.uuid4().hex
            body = {"to": self.identity["user"]["account"], "amount": 1, "reason": "Locust issuance"}
            if SCENARIO == "write":
                self.call("POST", "/api/v1/admin/issue", body, FIXTURE["nana"], 201, key)
            else:
                jobs = [
                    gevent.spawn(self.call, "POST", "/api/v1/admin/issue", body, FIXTURE["nana"], 201, key)
                    for _ in range(4)
                ]
                gevent.joinall(jobs, raise_error=True)
                ids = {j.value.get("id") for j in jobs if j.value}
                if len(ids) > 1:
                    stop(self.environment, "same idempotency key produced multiple transaction IDs")
        elif SCENARIO == "market":
            seller, buyer = FIXTURE["members"]
            item = self.call(
                "POST",
                "/api/v1/listings",
                {"title": "Locust item", "description": "x" * 96, "price": 1},
                seller,
                201,
            )
            if item:
                self.call(
                    "POST",
                    f"/api/v1/listings/{item['id']}/purchase",
                    {},
                    buyer,
                    201,
                    uuid.uuid4().hex,
                    "purchase",
                )
        elif SCENARIO == "forex":
            # Currency trading, which is the heaviest write the board has.
            #
            # Every other write produces one ledger record. A trade produces
            # two - the coin leg and the cash leg - so it is the request most
            # able to exhaust a fragmented heap, and the one worth driving
            # concurrently rather than in a sequential script.
            #
            # Posting and taking are counted separately: posting moves no
            # money and should stay cheap, while taking is the two-record
            # write. If the board is going to struggle it will show up as the
            # gap between those two lines.
            maker, taker = FIXTURE["members"]
            quote = self.call(
                "POST",
                "/api/v1/quotes",
                {"side": "ASK", "cents_per_coin": 1, "coins": 1},
                maker,
                201,
                name="post-quote",
            )
            if quote:
                self.call(
                    "POST",
                    f"/api/v1/quotes/{quote['id']}/take",
                    {},
                    taker,
                    201,
                    uuid.uuid4().hex,
                    "take-quote",
                )
            # Reading the book is what the Angular page polls, so it belongs
            # in the same mix rather than in a separate run.
            self.call("GET", "/api/v1/quotes", None, maker, 200, name="read-book")
        elif SCENARIO == "seed":
            # The write mix the Angular "add a year of history" button makes.
            #
            # That button drives the ordinary API from the browser, so a
            # household seeding itself looks exactly like this to the board:
            # a listing, a transfer, a purchase and an offer, repeated a few
            # hundred times. The question this scenario answers is whether a
            # board survives someone pressing it - which is not something the
            # browser can tell you, because a browser that gets no answer
            # cannot say whether the board is busy or gone.
            seller, buyer = FIXTURE["members"]

            listing = self.call(
                "POST",
                "/api/v1/listings",
                {"title": "Do the dishes", "description": "Chores", "price": 5},
                seller,
                201,
                name="seed-listing",
            )
            self.call(
                "POST",
                "/api/v1/transfers",
                {"to": buyer["user"]["account"], "amount": 1, "memo": "Take out the trash"},
                seller,
                201,
                uuid.uuid4().hex,
                "seed-transfer",
            )
            if listing:
                offer = self.call(
                    "POST",
                    f"/api/v1/listings/{listing['id']}/offers",
                    {"amount": 3, "message": "Would you take this?"},
                    buyer,
                    201,
                    uuid.uuid4().hex,
                    "seed-offer",
                )
                if offer:
                    self.call(
                        "POST",
                        f"/api/v1/offers/{offer['id']}/accept",
                        {},
                        seller,
                        201,
                        uuid.uuid4().hex,
                        "seed-accept",
                    )
        elif SCENARIO == "auth":
            verifier, challenge = pkce()
            username, password = credentials()
            code = self.call(
                "POST",
                "/api/v1/auth/authorize",
                {
                    "username": username,
                    "password": password,
                    "code_challenge": challenge,
                    "code_challenge_method": "S256",
                    "redirect_uri": ORIGIN + "/cb",
                },
            )
            if code:
                result = self.call(
                    "POST",
                    "/api/v1/auth/token",
                    {"code": code["code"], "code_verifier": verifier, "redirect_uri": ORIGIN + "/cb"},
                )
                if result:
                    self.call(
                        "POST",
                        "/api/v1/auth/logout",
                        identity={"token": result["access_token"]},
                        expected=204,
                    )
