"""Simultaneous bounded-book contracts through the public JSON interface only."""


def exercise(client, unix_seconds, progress=None):
    metadata = client.get("/status")
    prefix = f"g{metadata['journal_generation']}:m{metadata['money_epoch']}:" if metadata["money_epoch"] else ""
    request_number = 0

    def call(path, payload=None, token="", key="", **kwargs):
        nonlocal request_number
        request_number += 1
        if prefix and payload is not None:
            key = prefix + (key or f"capacity-request-{request_number}")
        return client.call(path, payload, token, key, **kwargs)

    def commerce(action, token, key, **kwargs):
        return call("/commerce/commands", action, token, key, **kwargs)

    def stage(name):
        if progress:
            progress(name)

    stage("members")
    users = client.get("/users", client.nana)["users"]
    for index in range(32 - len(users)):
        call("/users", dict(username=f"capacity{index}", display_name=f"Capacity {index}",
                    password="5678", grant=False), client.nana, expected=201)
    users = client.get("/users", client.nana)["users"]
    client.assertEqual(len(users), 32)
    call("/users", dict(username="overcapacity", display_name="Overflow", password="5678", grant=False),
                client.nana, expected=507)
    call("/admin/issue", dict(to=client.accounts["alice"], amount=1000000, reason="Capacity funds"), client.nana, "capacity-funds", expected=201)

    stage("quotes")
    tokens = [client.nana, client.alice, client.bob]
    tokens += [client.login(f"capacity{i}", "5678") for i in range(5)]
    decimals = client.get("/loans", client.nana)["decimals"]
    terms = dict(side="ASK", cents_per_coin=100, coins=10 ** decimals)
    # Existing terminal quotes are recyclable; the active-book limit is 16.
    for token in tokens:
        for _ in range(2):
            call("/quotes", terms, token, expected=201)
    extra = client.login("capacity5", "5678")
    call("/quotes", terms, extra, expected=507)
    client.assertEqual(len(client.get("/quotes", client.nana)["quotes"]), 16)

    stage("gift requests and art")
    commerce_view = client.get("/commerce", client.nana)
    for index in range(32 - len(commerce_view["requests"])):
        commerce({"create_request": dict(title=f"Capacity gift {index}", description="",
                        target=None, deadline=None)}, client.bob, f"capacity-gift-{index}")
    commerce({"create_request": dict(title="Overflow", description="", target=None, deadline=None)},
                    client.bob, "capacity-gift-overflow", expected=507)
    art = dict(title="Capacity edition", license="Display", sha256="ab" * 32,
               locator="https://example.invalid/capacity.png")
    for index in range(64 - len(commerce_view["artworks"])):
        commerce({"mint_art": dict(art, title=f"Capacity edition {index}")},
                        client.bob, f"capacity-art-{index}")
    commerce({"mint_art": art}, client.bob, "capacity-art-overflow", expected=507)
    commerce_view = client.get("/commerce", client.nana)
    client.assertEqual((len(commerce_view["requests"]), len(commerce_view["artworks"])), (32, 64))

    stage("loan applications and lotto pools")
    for index in range(32):
        call("/loans/request", client.loan_terms(), client.bob, f"capacity-loan-{index}")
    call("/loans/request", client.loan_terms(), client.bob, "capacity-loan-overflow", expected=507)
    client.assertEqual(len(client.get("/loans", client.nana)["loans"]), 32)
    lottos = client.get("/lottos", client.nana)["lottos"]
    terms = dict(kind="SIMPLE", title="Capacity draw", ticket_price=10,
                 closes_at=unix_seconds + 3600, rate_bps=0)
    for index in range(16 - len(lottos)):
        call("/lottos", terms, client.nana, f"capacity-lotto-{index}")
    call("/lottos", terms, client.nana, "capacity-lotto-overflow", expected=507)
    client.assertEqual(len(client.get("/lottos", client.nana)["lottos"]), 16)

    stage("listings and offers")
    listings = [call("/listings", dict(title=f"Capacity listing {index}", description="", side="SELL", price=30), client.bob, expected=201) for index in range(48)]
    call("/listings", dict(title="Overflow", description="", side="SELL", price=30),
                client.bob, expected=507)
    client.assertEqual(client.get("/status")["active_listings"], 48)
    for row in listings[:32]:
        call("/listings/" + row["id"] + "/offers", dict(amount=20), client.alice, expected=201)
    call("/listings/" + listings[32]["id"] + "/offers", dict(amount=20), client.alice, expected=507)
    client.assertEqual(len(client.get("/offers", client.alice)["offers"]), 32)

    stage("fulfillment obligations")
    before = client.get("/fulfillments", client.alice)["fulfillments"]
    payload = dict(to=client.accounts["bob"], amount=1, memo="Capacity work",
                   economic_kind="LABOR", quantity="1", unit="HOUR")
    for index in range(128 - len(before)):
        call("/transfers", payload, client.alice, f"capacity-work-{index}", expected=201)
    call("/transfers", payload, client.alice, "capacity-work-overflow", expected=507)
    client.assertEqual(len(client.get("/fulfillments", client.alice)["fulfillments"]), 128)

    stage("whole-bank reform with all books full")
    before = client.balance("alice")
    meta = client.get("/loans", client.nana)
    request = dict(decimals=meta["decimals"] + 1, power=0, expected_epoch=meta["money_epoch"],
                   expected_sequence=meta["sequence"], preview=True)
    client.assertTrue(call("/admin/reform", request, client.nana)["preview"])
    call("/admin/reform", dict(request, preview=False), client.nana, "capacity-reform")
    client.assertEqual(client.balance("alice"), before * 10)
    client.assertTrue(client.get("/status")["ledger_balanced"])
    return dict(members=32, quotes=16, requests=32, artworks=64, loans=32, lottos=16,
                active_listings=48, offers=32, fulfillments=128)


def saturate_history(client, progress=None):
    """Cross the advertised retention limit while balancing every round trip."""
    metadata = client.get("/status")
    capacity = metadata["transaction_capacity"]
    client.assertTrue(0 < capacity <= 10000, "resource probe requires a bounded retention profile")
    count = capacity + 8
    if count % 2:
        count += 1
    balances = (client.balance("alice"), client.balance("bob"))
    start_transactions = metadata["transactions"]
    for index in range(count):
        if index % 250 == 0:
            metadata = client.get("/status")
            if progress:
                progress(f"retained history: {index}/{count}")
        token, recipient = (client.alice, "bob") if index % 2 == 0 else (client.bob, "alice")
        payload = dict(to=client.accounts[recipient], amount=1, memo="Capacity retention payment")
        for attempt in range(2):
            key = f"g{metadata['journal_generation']}:m{metadata['money_epoch']}:capacity-retention-{index}"
            status, body = client.request("/transfers", payload, token, key)
            if status == 409 and body.get("error") == "stale_request" and attempt == 0:
                metadata = client.get("/status")
                continue
            client.assertEqual(status, 201, str(body))
            break
    metadata = client.get("/status")
    client.assertEqual(metadata["transactions"], start_transactions + count)
    client.assertEqual(metadata["retained_transactions"], capacity)
    client.assertEqual((client.balance("alice"), client.balance("bob")), balances)
    client.assertTrue(metadata["ledger_balanced"])
    return dict(retained_transactions=capacity, payments=count,
                total_transactions=metadata["transactions"], journal_generation=metadata["journal_generation"])
