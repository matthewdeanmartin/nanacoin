//! Bot members, read-only and Nana-made API keys, the activity feed and the
//! per-member capacity limits (mastomini/sprint/nanabots-2-nanacoin.md).
mod common;
use nanacoin::{api, domain::*, journal::file::FileJournal, journal::*};
use serde_json::{json, Value};
use std::cell::Cell;

thread_local! { static NOW: Cell<u64> = const { Cell::new(1_800_000_000) }; }
fn now() -> u64 {
    NOW.with(Cell::get)
}

/// A household on a real file journal (checkpoints, archive) in its own
/// temporary directory, removed when dropped.
struct House {
    s: Option<Service<FileJournal>>,
    dir: std::path::PathBuf,
    keys: usize,
}

impl Drop for House {
    fn drop(&mut self) {
        // Only this test's own directory, after closing its journal.
        self.s = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl House {
    fn new(name: &str) -> House {
        NOW.with(|n| n.set(1_800_000_000));
        let dir = std::env::temp_dir().join(format!("nanacoin-bots-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s =
            Service::open_with_clock(FileJournal::open(dir.join("economy.journal")).unwrap(), now)
                .unwrap();
        common::provision(&mut s);
        House {
            s: Some(s),
            dir,
            keys: 0,
        }
    }

    fn s(&mut self) -> &mut Service<FileJournal> {
        self.s.as_mut().unwrap()
    }

    /// Close the journal (releasing its file lock), then open it again.
    fn reopen(&mut self) {
        self.s = None;
        let journal = FileJournal::open(self.dir.join("economy.journal")).unwrap();
        self.s = Some(Service::open_with_clock(journal, now).unwrap());
    }

    fn call(&mut self, method: &str, path: &str, auth: &str, body: Value) -> (u16, Value) {
        self.keys += 1;
        let key = format!("g{}:t{}", self.s().generation(), self.keys);
        let mut output = vec![0; api::RESPONSE_LIMIT];
        let body = serde_json::to_vec(&body).unwrap();
        let (status, n) = api::handle_keyed(self.s(), method, path, auth, &key, &body, &mut output);
        let value = if n == 0 {
            Value::Null
        } else {
            serde_json::from_slice(&output[..n]).unwrap()
        };
        (status, value)
    }

    fn get(&mut self, path: &str, auth: &str) -> Value {
        let (status, v) = self.call("GET", path, auth, json!({}));
        assert_eq!(status, 200, "GET {path}: {v}");
        v
    }

    fn ok(&mut self, method: &str, path: &str, auth: &str, body: Value) -> Value {
        let (status, v) = self.call(method, path, auth, body);
        assert!(
            (200..300).contains(&status),
            "{method} {path}: {status} {v}"
        );
        v
    }

    fn member(&mut self, nana: &str, name: &str, kind: &str) -> Value {
        self.ok(
            "POST",
            "/api/v1/users",
            nana,
            json!({"username": name, "display_name": name, "password": "1234", "kind": kind}),
        )
    }
}

fn bearer(key: &Value) -> String {
    format!("Bearer {}", key["api_key"].as_str().unwrap())
}

#[test]
fn bots_get_keys_from_nana_and_cannot_claim_good_deeds() {
    let mut h = House::new("keys");
    let nana = common::login(h.s());
    let bot = h.member(&nana, "trader", "bot");
    assert_eq!(bot["kind"], "bot");
    assert_eq!(bot["role"], "user");
    let alice = h.member(&nana, "alice", "human");
    assert_eq!(alice["kind"], "human");
    let (status, v) = h.call(
        "POST",
        "/api/v1/users",
        &nana,
        json!({"username": "boss", "display_name": "Boss", "password": "1234",
               "kind": "bot", "role": "nana"}),
    );
    assert_eq!((status, v["error"].as_str()), (400, Some("invalid_input")));

    // Nana makes the bot's key; never a person's.
    let key = h.ok("POST", "/api/v1/users/user-2/api-key", &nana, json!({}));
    assert_eq!(key["scope"], "full");
    let bot_key = bearer(&key);
    assert_eq!(h.get("/api/v1/me", &bot_key)["kind"], "bot");
    let (status, _) = h.call("POST", "/api/v1/users/user-3/api-key", &nana, json!({}));
    assert_eq!(status, 403, "a person makes their own key");
    let alice_session = common::login_as(h.s(), "alice", "1234");
    let (status, _) = h.call(
        "POST",
        "/api/v1/users/user-2/api-key",
        &alice_session,
        json!({}),
    );
    assert_eq!(status, 403, "only Nana");
    let (status, _) = h.call("POST", "/api/v1/users/user-2/api-key", &bot_key, json!({}));
    assert_eq!(status, 403, "a key never makes keys");
    assert_eq!(
        h.get("/api/v1/users/user-2/api-key", &nana)["full"]["active"],
        true
    );

    // A bot is refused a good deed; a person may claim it.
    let deed = h.ok(
        "POST",
        "/api/v1/listings",
        &nana,
        json!({"title": "Rake the leaves", "price": 50000, "side": "BUY", "kind": "good_deed"}),
    );
    let offers = format!("/api/v1/listings/{}/offers", deed["id"].as_str().unwrap());
    let (status, v) = h.call("POST", &offers, &bot_key, json!({"amount": 50000}));
    assert_eq!((status, v["error"].as_str()), (403, Some("bot_good_deed")));
    h.ok("POST", &offers, &alice_session, json!({"amount": 50000}));

    // Nana revokes; the key stops working.
    h.ok("DELETE", "/api/v1/users/user-2/api-key", &nana, json!({}));
    assert_eq!(h.call("GET", "/api/v1/me", &bot_key, json!({})).0, 401);
}

#[test]
fn read_keys_only_read() {
    let mut h = House::new("read");
    let nana = common::login(h.s());
    h.member(&nana, "alice", "human");
    let alice = common::login_as(h.s(), "alice", "1234");
    let read = h.ok(
        "POST",
        "/api/v1/me/api-key",
        &alice,
        json!({"password": "1234", "scope": "read"}),
    );
    assert_eq!(read["scope"], "read");
    let read = bearer(&read);
    let status = h.get("/api/v1/me/api-key", &alice);
    assert_eq!(status["read"]["active"], true);
    assert_eq!(status["full"]["active"], false);
    assert_eq!(
        status["active"], false,
        "the old field still means the full key"
    );
    assert!(h.get("/api/v1/users", &read)["users"].is_array());
    h.get("/api/v1/activity", &read);
    for (method, path, body) in [
        (
            "POST",
            "/api/v1/transfers",
            json!({"to": "account-1", "amount": 1, "memo": ""}),
        ),
        (
            "POST",
            "/api/v1/listings",
            json!({"title": "x", "price": 1}),
        ),
        ("PATCH", "/api/v1/users/user-2", json!({"bio": "hi"})),
        ("DELETE", "/api/v1/me/api-key", json!({})),
    ] {
        let (status, v) = h.call(method, path, &read, body);
        assert_eq!(
            (status, v["error"].as_str()),
            (403, Some("read_only_key")),
            "{method} {path}"
        );
    }
    // A full key beside it; revoking one leaves the other.
    let full = bearer(&h.ok(
        "POST",
        "/api/v1/me/api-key",
        &alice,
        json!({"password": "1234"}),
    ));
    h.ok("DELETE", "/api/v1/me/api-key?scope=read", &alice, json!({}));
    assert_eq!(h.call("GET", "/api/v1/me", &read, json!({})).0, 401);
    h.get("/api/v1/me", &full);
    // Nana can't make a person's read key, nor a bot's.
    h.member(&nana, "bot", "bot");
    let (status, _) = h.call(
        "POST",
        "/api/v1/users/user-3/api-key?scope=read",
        &nana,
        json!({}),
    );
    assert_eq!(status, 403);
}

#[test]
fn the_activity_feed_says_what_happened_and_nothing_private() {
    let mut h = House::new("feed");
    let nana = common::login(h.s());
    h.member(&nana, "alice", "human");
    h.member(&nana, "trader", "bot");
    let alice = common::login_as(h.s(), "alice", "1234");
    let bot = bearer(&h.ok("POST", "/api/v1/users/user-3/api-key", &nana, json!({})));
    h.ok(
        "POST",
        "/api/v1/admin/issue-usd",
        &nana,
        json!({"to": "account-2", "cents": 5000, "reason": "x"}),
    );
    let start = h.get("/api/v1/activity?limit=1", &alice)["sequence"]
        .as_u64()
        .unwrap();

    h.ok(
        "POST",
        "/api/v1/listings",
        &alice,
        json!({"title": "Bike", "description": "SECRET-DESCRIPTION", "price": 50000}),
    );
    h.ok(
        "POST",
        "/api/v1/lottos",
        &nana,
        json!({"kind": "SIMPLE", "title": "Friday pot", "ticket_price": 10000,
               "closes_at": now() + 60, "rate_bps": 0}),
    );
    let lotto = h.get("/api/v1/lottos", &alice)["lottos"][0]["id"]
        .as_u64()
        .unwrap();
    h.ok(
        "POST",
        &format!("/api/v1/lottos/{lotto}/tickets"),
        &alice,
        json!({"count": 2}),
    );
    let quote = h.ok(
        "POST",
        "/api/v1/quotes",
        &bot,
        json!({"side": "ASK", "cents_per_coin": 12, "coins": 10000}),
    );
    h.ok(
        "POST",
        &format!("/api/v1/quotes/{}/take", quote["id"].as_str().unwrap()),
        &alice,
        json!({}),
    );
    h.ok(
        "POST",
        "/api/v1/loans/request",
        &alice,
        json!({"borrower": "account-2", "amount": 100000, "rate_bps": 500, "rate_days": 365,
               "payment_days": 30, "installment": 10000, "credit": false, "memo": "SECRET-NOTE"}),
    );
    h.ok(
        "POST",
        "/api/v1/transfers",
        &alice,
        json!({"to": "account-1", "amount": 0, "memo": "SECRET-MESSAGE"}),
    );
    NOW.with(|n| n.set(now() + 61));
    while h.s().tick().unwrap() > 0 {}

    let feed = h.get(&format!("/api/v1/activity?after={start}"), &alice);
    assert_eq!(feed["truncated"], false);
    assert_eq!(feed["decimals"], 4);
    let kinds: Vec<&str> = feed["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "listing_opened",
            "lotto_opened",
            "quote_posted",
            "quote_taken",
            "loan_requested",
            "lotto_drawn"
        ]
    );
    let events = &feed["events"];
    assert_eq!(events[0]["title"], "Bike");
    assert_eq!(events[0]["side"], "SELL");
    assert_eq!(events[0]["amount"], 50000);
    assert_eq!(events[0]["actor_name"], "alice");
    assert_eq!(events[1]["closes_at"], now() - 1);
    assert_eq!(events[1]["lotto_kind"], "SIMPLE");
    assert_eq!(events[2]["actor_bot"], true);
    assert_eq!(events[2]["rate"], 12);
    assert_eq!(events[3]["other_name"], "trader");
    assert_eq!(events[4]["apr_bps"], 500);
    assert_eq!(events[5]["other_name"], "alice", "the winner");
    assert_eq!(events[5]["amount"], 20000, "the pool");
    let text = feed.to_string();
    for secret in [
        "SECRET-DESCRIPTION",
        "SECRET-NOTE",
        "SECRET-MESSAGE",
        "password",
        "key_hash",
    ] {
        assert!(!text.contains(secret), "{secret} leaked: {text}");
    }

    // Paging: `after` the last seen event, at most `limit`.
    let first = events[0]["seq"].as_u64().unwrap();
    let page = h.get(&format!("/api/v1/activity?after={first}&limit=2"), &alice);
    assert_eq!(page["events"].as_array().unwrap().len(), 2);
    assert_eq!(page["events"][0]["kind"], "lotto_opened");
    assert_eq!(
        h.call("GET", "/api/v1/activity?limit=51", &alice, json!({}))
            .0,
        400
    );
    // Joins are news; other identity changes are not.
    let all = h.get("/api/v1/activity", &alice);
    let joined: Vec<&str> = all["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "member_joined")
        .map(|e| e["actor_name"].as_str().unwrap())
        .collect();
    assert_eq!(joined, vec!["Nana", "alice", "trader"]);
    let loans = h.get("/api/v1/loans", &alice);
    assert_eq!(loans["loans"][0]["apr_bps"], 500);
}

#[test]
fn one_member_cannot_fill_the_quote_book_or_lend_to_everyone() {
    let mut h = House::new("caps");
    let nana = common::login(h.s());
    h.member(&nana, "trader", "bot");
    let bot = bearer(&h.ok("POST", "/api/v1/users/user-2/api-key", &nana, json!({})));
    for rate in [10, 11] {
        h.ok(
            "POST",
            "/api/v1/quotes",
            &bot,
            json!({"side": "BID", "cents_per_coin": rate, "coins": 10000}),
        );
    }
    let (status, v) = h.call(
        "POST",
        "/api/v1/quotes",
        &bot,
        json!({"side": "BID", "cents_per_coin": 12, "coins": 10000}),
    );
    assert_eq!(
        (status, v["error"].as_str()),
        (507, Some("member_quote_limit"))
    );

    for n in 0..5 {
        h.member(&nana, &format!("b{n}"), "human");
    }
    let offer = |to: u8| {
        json!({"borrower": format!("account-{to}"), "amount": 10000, "rate_bps": 100,
               "rate_days": 365, "payment_days": 30, "installment": 10000,
               "credit": false, "memo": ""})
    };
    for to in 3..7 {
        h.ok("POST", "/api/v1/loans", &bot, offer(to));
    }
    let (status, v) = h.call("POST", "/api/v1/loans", &bot, offer(7));
    assert_eq!(
        (status, v["error"].as_str()),
        (507, Some("member_loan_limit"))
    );
}

#[test]
fn checkpoints_keep_bots_read_keys_and_members_past_sixteen() {
    let mut h = House::new("ckpt");
    let nana = common::login(h.s());
    for n in 2..=20 {
        let kind = if n == 19 { "bot" } else { "human" };
        h.member(&nana, &format!("m{n}"), kind);
    }
    assert_eq!(h.s().state().members.len(), 20);
    let m20 = common::login_as(h.s(), "m20", "1234");
    let read = bearer(&h.ok(
        "POST",
        "/api/v1/me/api-key",
        &m20,
        json!({"password": "1234", "scope": "read"}),
    ));
    let bot = bearer(&h.ok("POST", "/api/v1/users/user-19/api-key", &nana, json!({})));
    h.ok(
        "POST",
        "/api/v1/lottos",
        &nana,
        json!({"kind": "SIMPLE", "title": "Pot", "ticket_price": 10000,
               "closes_at": now() + 600, "rate_bps": 0}),
    );
    let lotto = h.get("/api/v1/lottos", &m20)["lottos"][0]["id"]
        .as_u64()
        .unwrap();
    h.ok(
        "POST",
        &format!("/api/v1/lottos/{lotto}/tickets"),
        &m20,
        json!({"count": 3}),
    );
    h.ok(
        "POST",
        &format!("/api/v1/lottos/{lotto}/tickets"),
        &bot,
        json!({"count": 1}),
    );

    h.s().checkpoint(MemberId(1)).unwrap();
    h.reopen();
    let state = h.s.as_ref().unwrap().state();
    assert_eq!(state.member(MemberId(19)).unwrap().kind, MemberKind::Bot);
    assert_eq!(state.member(MemberId(20)).unwrap().kind, MemberKind::Human);
    assert_eq!(
        state.lotto(lotto).unwrap().tickets[19],
        3,
        "member 20's tickets"
    );
    assert_eq!(state.lotto(lotto).unwrap().tickets[18], 1);
    state.check_invariants().unwrap();
    assert_eq!(h.get("/api/v1/me", &read)["username"], "m20");
    assert_eq!(h.get("/api/v1/me", &bot)["kind"], "bot");
    // And once more, from the checkpoint plus a journal tail. (Sessions
    // don't survive a restart; keys do.)
    let m20 = common::login_as(h.s(), "m20", "1234");
    h.ok(
        "POST",
        &format!("/api/v1/lottos/{lotto}/tickets"),
        &m20,
        json!({"count": 1}),
    );
    h.reopen();
    assert_eq!(h.s().state().lotto(lotto).unwrap().tickets[19], 4);
}

/// Current identity audit classification survives a checkpoint and journal tail.
#[test]
fn identity_audit_creation_survives_checkpoint_and_tail() {
    let mut h = House::new("identity-audit");
    let nana = common::login(h.s());
    h.member(&nana, "alice", "human");
    h.member(&nana, "bob", "human");
    h.ok(
        "PATCH",
        "/api/v1/users/user-2",
        &nana,
        json!({"display_name": "Alice Admin", "role": "nana"}),
    );
    h.ok(
        "PATCH",
        "/api/v1/users/user-2",
        &nana,
        json!({"display_name": "Alice Updated", "role": "user", "password": "9876"}),
    );
    h.s().checkpoint(MemberId(1)).unwrap();
    h.reopen();
    let nana = common::login(h.s());
    let alice = common::login_as(h.s(), "alice", "9876");
    let feed = h.get("/api/v1/activity", &alice);
    assert_eq!(feed["events"].as_array().unwrap().len(), 3);
    assert!(feed["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["kind"] == "member_joined"));
    let audit = h.get("/api/v1/audit", &nana);
    let updates: Vec<_> = audit["audit"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["sequence"].as_u64().unwrap() > 3)
        .collect();
    assert_eq!(updates.len(), 2);
    assert!(updates
        .iter()
        .all(|a| a["action"]["Identity"]["created"] == false));
    h.member(&nana, "trader", "bot");
    let bot = bearer(&h.ok("POST", "/api/v1/users/user-4/api-key", &nana, json!({})));
    let read = bearer(&h.ok(
        "POST",
        "/api/v1/me/api-key",
        &alice,
        json!({"password": "9876", "scope": "read"}),
    ));
    h.reopen();
    assert_eq!(h.get("/api/v1/me", &bot)["kind"], "bot");
    assert_eq!(h.get("/api/v1/me", &read)["display_name"], "Alice Updated");
    assert_eq!(
        h.get("/api/v1/activity", &read)["events"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    h.s().state().check_invariants().unwrap();
}
