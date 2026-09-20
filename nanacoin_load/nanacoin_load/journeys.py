import os
import threading
import uuid
from concurrent.futures import ThreadPoolExecutor

from .client import API
from .common import STATE, credentials, write_json


def prepare(host, evidence):
    api = API(host, evidence)
    status = api.call("GET", "/api/v1/status")
    username, password = credentials()
    if not status["provisioned"]:
        api.call(
            "POST",
            "/api/v1/provision",
            {
                "username": username,
                "password": password,
                "display_name": "Load Nana",
                "household_name": "Locust test household",
            },
            expected=201,
        )
    nana = api.login(username, password)
    users = api.call("GET", "/api/v1/users", token=nana["token"])["users"]
    members = []
    member_password = os.getenv("NANA_MEMBER_PASSWORD", "locust-test-pin")
    for name in ("load_alice", "load_bob"):
        if not any(u["username"] == name for u in users):
            api.call(
                "POST",
                "/api/v1/users",
                {"username": name, "display_name": name, "password": member_password},
                token=nana["token"],
                expected=201,
            )
        member = api.login(name, member_password)
        me = api.call("GET", "/api/v1/me", token=member["token"])
        if me["balance"] < 10000:
            api.call(
                "POST",
                "/api/v1/admin/issue",
                {"to": me["account"], "amount": 10000 - me["balance"], "reason": "Locust fixture funding"},
                token=nana["token"],
                expected=201,
                key=uuid.uuid4().hex,
            )
        # Dollars as well as coins: the forex scenario trades one for the
        # other, and a member with no dollars can only ever sell.
        if me.get("usd_cents", 0) < 100000:
            api.call(
                "POST",
                "/api/v1/admin/issue-usd",
                {
                    "to": me["account"],
                    "cents": 100000 - me.get("usd_cents", 0),
                    "reason": "Locust fixture funding",
                },
                token=nana["token"],
                expected=201,
                key=uuid.uuid4().hex,
            )
        members.append(member)
    fixture = {"host": host.rstrip("/"), "nana": nana, "members": members}
    STATE.parent.mkdir(exist_ok=True)
    write_json(STATE, fixture)
    evidence.emit("check", name="fixture prepared: Nana and two funded members", passed=True)
    return fixture


def e2e(host, fixture, evidence):
    api = API(host, evidence)
    nana, (alice, bob) = fixture["nana"], fixture["members"]
    at, bt, nt = alice["token"], bob["token"], nana["token"]
    aid, bid = alice["user"]["account"], bob["user"]["account"]

    def check(name, condition):
        evidence.emit("check", name=name, passed=bool(condition))
        if not condition:
            raise AssertionError(name)

    before = api.call("GET", "/api/v1/me", token=at)["balance"]
    tx = api.call(
        "POST",
        "/api/v1/transfers",
        {"to": bid, "amount": 7, "memo": "E2E transfer"},
        token=at,
        expected=201,
        key=uuid.uuid4().hex,
    )
    check("transfer debits exactly seven", api.call("GET", "/api/v1/me", token=at)["balance"] == before - 7)
    api.call(
        "POST",
        f"/api/v1/transactions/{tx['id']}/reverse",
        {"reason": "E2E correction"},
        token=nt,
        expected=201,
        key=uuid.uuid4().hex,
    )
    check("reversal restores balance", api.call("GET", "/api/v1/me", token=at)["balance"] == before)
    listing = api.call(
        "POST",
        "/api/v1/listings",
        {"title": "Locust E2E item", "description": "Disposable test listing", "price": 3},
        token=at,
        expected=201,
    )
    key = uuid.uuid4().hex
    purchase = api.call(
        "POST", f"/api/v1/listings/{listing['id']}/purchase", {}, token=bt, expected=201, key=key
    )
    replay = api.call(
        "POST", f"/api/v1/listings/{listing['id']}/purchase", {}, token=bt, expected=201, key=key
    )
    check(
        "purchase retry returns the original transaction",
        purchase["transaction"]["id"] == replay["transaction"]["id"],
    )
    check("purchase pays seller once", api.call("GET", "/api/v1/me", token=at)["balance"] == before + 3)
    # Each worker has its own TCP session; a barrier aligns the four attempts.
    before = api.call("GET", "/api/v1/me", token=bt)["balance"]
    key = uuid.uuid4().hex
    barrier = threading.Barrier(4, timeout=10)

    def issue(_):
        barrier.wait()
        return API(host, evidence).call(
            "POST",
            "/api/v1/admin/issue",
            {"to": bid, "amount": 1, "reason": "E2E concurrent replay"},
            token=nt,
            expected=201,
            key=key,
        )

    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(issue, range(4)))
    check("four overlapping retries share one transaction ID", len({r["id"] for r in results}) == 1)
    check("four retries credit once", api.call("GET", "/api/v1/me", token=bt)["balance"] == before + 1)
    api.call("GET", "/api/v1/transactions", token=at, expected=403)
    check("member cannot read Nana's full ledger", True)
    history = api.call("GET", f"/api/v1/accounts/{aid}/transactions?limit=30", token=at)
    check("account history is readable", bool(history["transactions"]))

    # Foreign exchange.
    #
    # A trade is the only write that produces two ledger records - the coin
    # leg and the cash leg - so it is checked here rather than only in Go
    # tests: what matters is that both land on the real board, in the right
    # direction, and that the ledger still balances across both currencies
    # afterwards. Direction is the part worth asserting; "sell" and "buy" are
    # each ambiguous about which currency is moving, and an earlier
    # marketplace bug paid want-ads backwards for exactly that reason.
    a_coins = api.call("GET", "/api/v1/me", token=at)["balance"]
    a_cents = api.call("GET", "/api/v1/me", token=at).get("usd_cents", 0)
    b_coins = api.call("GET", "/api/v1/me", token=bt)["balance"]
    b_cents = api.call("GET", "/api/v1/me", token=bt).get("usd_cents", 0)

    ask = api.call(
        "POST",
        "/api/v1/quotes",
        {"side": "ASK", "cents_per_coin": 25, "coins": 4},
        token=at,
        expected=201,
    )
    check("posting a quote moves no money",
          api.call("GET", "/api/v1/me", token=at)["balance"] == a_coins)
    check("the dollar side is computed by the server", ask["cents"] == 100)

    key = uuid.uuid4().hex
    trade = api.call(
        "POST", f"/api/v1/quotes/{ask['id']}/take", {}, token=bt, expected=201, key=key
    )
    check("a trade returns both legs",
          "coin_transaction" in trade and "cash_transaction" in trade)

    # Taking an ask is buying coins with dollars: the maker gives up coins and
    # receives cash, the taker the reverse.
    a_now = api.call("GET", "/api/v1/me", token=at)
    b_now = api.call("GET", "/api/v1/me", token=bt)
    check("seller gave four coins", a_now["balance"] == a_coins - 4)
    check("seller received one dollar", a_now.get("usd_cents", 0) == a_cents + 100)
    check("buyer received four coins", b_now["balance"] == b_coins + 4)
    check("buyer paid one dollar", b_now.get("usd_cents", 0) == b_cents - 100)

    # Retrying with the same key replays the receipt rather than trading
    # again - the same guarantee every other money-moving endpoint gives.
    replay = api.call(
        "POST", f"/api/v1/quotes/{ask['id']}/take", {}, token=bt, expected=201, key=key
    )
    check(
        "retrying a trade returns the original, without trading twice",
        replay["coin_transaction"]["id"] == trade["coin_transaction"]["id"],
    )
    check(
        "the retry moved no additional money",
        api.call("GET", "/api/v1/me", token=bt)["balance"] == b_coins + 4,
    )
    # A *fresh* attempt on a filled quote is the refusal, and it is a
    # different question from the retry above.
    api.call(
        "POST", f"/api/v1/quotes/{ask['id']}/take", {}, token=bt, expected=409,
        key=uuid.uuid4().hex,
    )
    check("a filled quote cannot be taken again", True)

    mine = api.call(
        "POST",
        "/api/v1/quotes",
        {"side": "BID", "cents_per_coin": 5, "coins": 1},
        token=at,
        expected=201,
    )
    api.call("POST", f"/api/v1/quotes/{mine['id']}/take", {}, token=at, expected=400)
    check("you cannot take your own quote", True)
    cancelled = api.call("POST", f"/api/v1/quotes/{mine['id']}/cancel", {}, token=at)
    check("the maker can withdraw a quote", cancelled["status"] == "CANCELLED")

    book = api.call("GET", "/api/v1/quotes", token=at)
    asks = [q["cents_per_coin"] for q in book["quotes"] if q["side"] == "ASK" and q["live"]]
    bids = [q["cents_per_coin"] for q in book["quotes"] if q["side"] == "BID" and q["live"]]
    check("asks are ordered cheapest first", asks == sorted(asks))
    check("bids are ordered best price first", bids == sorted(bids, reverse=True))

    status = api.call("GET", "/api/v1/status")
    check("ledger remains balanced", status["ledger_balanced"] is True)
    evidence.emit("snapshot", role="e2e", data=api.call("GET", "/api/v1/diag"))
