mod common;
use nanacoin::{
    api,
    auth::{Auth, PasswordVerifier, CODE_TTL, MAX_CODES, MAX_SESSIONS, SESSION_TTL},
    domain::*,
    journal::*,
};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const NOW: u64 = 1_800_000_000;
thread_local! { static CLOCK: Cell<u64> = const { Cell::new(NOW) }; }
fn now() -> u64 {
    CLOCK.with(Cell::get)
}
#[derive(Clone, Default)]
struct Memory {
    frames: Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>,
    failure: Rc<Cell<u8>>,
}
impl Journal for Memory {
    fn read(&mut self, i: usize, out: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(frame) = self.frames.borrow().get(i) {
            *out = *frame;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, i: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        if self.failure.get() != 1 {
            assert_eq!(i, self.frames.borrow().len());
            self.frames.borrow_mut().push(*frame);
        }
        if self.failure.get() != 0 {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}
fn exec(s: &mut Service<Memory>, actor: u8, c: Command) -> Result<Receipt, Error> {
    let request = s.state().member(MemberId(actor))?.last_request + 1;
    s.execute(MemberId(actor), request, c)
}
fn house() -> (Service<Memory>, Memory) {
    CLOCK.with(|clock| clock.set(NOW));
    let disk = Memory::default();
    let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
    common::provision(&mut s);
    for name in ["alice", "bob", "outsider"] {
        exec(
            &mut s,
            1,
            Command::CreateMember {
                username: name.try_into().unwrap(),
                display_name: name.try_into().unwrap(),
                password: PasswordVerifier::hash("1234").unwrap(),
                role: Role::User,
                grant: 1000,
                mastodon_id: MastodonId::new(),
            },
        )
        .unwrap();
    }
    (s, disk)
}
fn request(
    s: &mut Service<Memory>,
    method: &str,
    path: &str,
    auth: &str,
    key: &str,
    body: &[u8],
) -> (u16, Value) {
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let (status, len) = api::handle_keyed_on(s, method, path, auth, key, body, &mut out, true);
    (
        status,
        if len == 0 {
            Value::Null
        } else {
            serde_json::from_slice(&out[..len]).unwrap()
        },
    )
}
fn unchanged(s: &Service<Memory>, disk: &Memory, state: &Value, writes: usize) {
    assert_eq!(serde_json::to_value(s.state()).unwrap(), *state);
    assert_eq!(disk.frames.borrow().len(), writes);
    s.state().check_invariants().unwrap();
}

#[test]
fn malformed_mutations_and_unknown_routes_never_write_or_publish() {
    let (mut s, disk) = house();
    let nana = common::login(&mut s);
    let before = serde_json::to_value(s.state()).unwrap();
    let writes = disk.frames.borrow().len();
    let routes = [
        ("POST", "/api/v1/users"),
        ("POST", "/api/v1/users/update"),
        ("PATCH", "/api/v1/users/user-2"),
        ("PATCH", "/api/v1/admin/config"),
        ("POST", "/api/v1/admin/issue"),
        ("POST", "/api/v1/admin/retire"),
        ("POST", "/api/v1/admin/issue-usd"),
        ("POST", "/api/v1/transfers"),
        ("POST", "/api/v1/listings"),
        ("POST", "/api/v1/quotes"),
        ("POST", "/api/v1/loans"),
        ("POST", "/api/v1/lottos"),
        ("POST", "/api/v1/commands"),
        ("POST", "/api/v1/admin/reset"),
        ("POST", "/api/v1/admin/checkpoint"),
        ("POST", "/api/v1/admin/light"),
    ];
    for (method, path) in routes {
        for body in [
            b"".as_slice(),
            b"null",
            b"[]",
            b"{",
            b"{\"unexpected\":true}",
            b"{}{}",
            b"\xff",
            b"{\"to\":\"account-2\",\"amount\":1.5}",
        ] {
            let (status, _) = request(&mut s, method, path, &nana, "g0:hostile", body);
            assert!(status >= 400, "{method} {path} {body:?}: {status}");
            unchanged(&s, &disk, &before, writes);
        }
    }
    for path in [
        "/api/v1/does-not-exist",
        "/api/v1/users/user-0",
        "/api/v1/listings/listing-18446744073709551616",
        "/api/v1/accounts/account-256",
        "/api/v1/transactions/tx--1",
        "/api/v1/offers/offer-0",
        "/api/v1/loans/loan-x",
        "/api/v1/lottos/lotto-x",
        "/api/v1/quotes/quote-x",
    ] {
        for method in ["GET", "DELETE", "PUT"] {
            assert!(
                request(&mut s, method, path, &nana, "", b"{}").0 >= 400,
                "{method} {path}"
            );
            unchanged(&s, &disk, &before, writes);
        }
    }
}

#[test]
fn admin_routes_reject_members_and_all_mutations_reject_anonymous_requests() {
    let (mut s, disk) = house();
    let alice = common::login_as(&mut s, "alice", "1234");
    let before = serde_json::to_value(s.state()).unwrap();
    let writes = disk.frames.borrow().len();
    for (method, path, body) in [
        ("GET", "/api/v1/state", json!({})),
        ("GET", "/api/v1/admin/storage", json!({})),
        ("GET", "/api/v1/audit", json!({})),
        ("GET", "/api/v1/admin/light", json!({})),
        (
            "PATCH",
            "/api/v1/admin/config",
            json!({"household_name":"stolen"}),
        ),
        (
            "POST",
            "/api/v1/admin/reset",
            json!({"expected_generation":0,"expected_sequence":s.state().sequence,"confirmation":"RESET ECONOMY"}),
        ),
        (
            "POST",
            "/api/v1/admin/issue",
            json!({"to":"account-2","amount":1}),
        ),
        (
            "PATCH",
            "/api/v1/users/user-3",
            json!({"password":"hacked"}),
        ),
        ("PATCH", "/api/v1/users/user-2", json!({"role":"nana"})),
        (
            "POST",
            "/api/v1/users",
            json!({"username":"intruder","display_name":"Intruder","password":"1234"}),
        ),
    ] {
        let bytes = serde_json::to_vec(&body).unwrap();
        assert_eq!(
            request(&mut s, method, path, &alice, "g0:hostile", &bytes).0,
            403,
            "{path}"
        );
        unchanged(&s, &disk, &before, writes);
    }
    for path in [
        "/api/v1/transfers",
        "/api/v1/listings",
        "/api/v1/quotes",
        "/api/v1/users",
        "/api/v1/admin/issue",
        "/api/v1/commands",
        "/api/v1/me/api-key",
    ] {
        assert_eq!(
            request(&mut s, "POST", path, "", "g0:hostile", b"{}").0,
            401,
            "{path}"
        );
        unchanged(&s, &disk, &before, writes);
    }
}

#[test]
fn hostile_paging_parameters_fail_without_writes_and_public_reads_remain_available() {
    let (mut s, disk) = house();
    let nana = common::login(&mut s);
    let before = serde_json::to_value(s.state()).unwrap();
    let writes = disk.frames.borrow().len();
    for query in [
        "cursor=",
        "cursor=1:2:3",
        "cursor=1:2:3:4:5",
        "cursor=-1:0:0:0",
        "cursor=18446744073709551616:0:0:0",
        "cursor=999:0:0:0",
        "cursor=0:999999:0:0",
        "cursor=0:0:1:0",
        "cursor=0:0:0:999",
        "limit=abc",
        "before=abc",
    ] {
        assert!(
            request(
                &mut s,
                "GET",
                &format!("/api/v1/transactions?{query}"),
                "",
                "",
                b""
            )
            .0 >= 400,
            "{query}"
        );
        unchanged(&s, &disk, &before, writes);
    }
    for path in [
        "/api/v1/audit?before=bad",
        "/api/v1/audit?page=bad",
        "/api/v1/audit?page=999",
        "/api/v1/audit?incarnation=bad",
        "/api/v1/activity?after=bad",
        "/api/v1/activity?limit=bad",
        "/api/v1/listings?status=BOGUS",
        "/api/v1/me/api-key?scope=bogus",
    ] {
        assert!(
            request(&mut s, "GET", path, &nana, "", b"").0 >= 400,
            "{path}"
        );
        unchanged(&s, &disk, &before, writes);
    }
    for path in [
        "/api/v1/status",
        "/api/v1/transport",
        "/api/v1/configuration",
        "/api/v1/diag/database",
        "/api/v1/diag/database/benchmark",
        "/api/v1/transactions?limit=0",
        "/api/v1/transactions?limit=999",
    ] {
        assert_eq!(request(&mut s, "GET", path, "", "", b"").0, 200, "{path}");
        unchanged(&s, &disk, &before, writes);
    }
}

#[test]
fn every_small_output_buffer_returns_a_bounded_result() {
    let (mut s, disk) = house();
    let nana = common::login(&mut s);
    let before = serde_json::to_value(s.state()).unwrap();
    let writes = disk.frames.borrow().len();
    for path in [
        "/api/v1/state",
        "/api/v1/me",
        "/api/v1/users",
        "/api/v1/status",
        "/api/v1/audit",
        "/api/v1/activity",
        "/api/v1/transactions",
        "/api/v1/accounts/account-2",
        "/api/v1/loans",
        "/api/v1/listings",
        "/api/v1/me/api-key",
        "/api/v1/not-found",
    ] {
        for size in [0, 1, 2, 8, 32, 64, 128] {
            let mut output = vec![0xA5; size];
            let (status, len) = api::handle(&mut s, "GET", path, &nana, b"", &mut output);
            assert!(len <= size, "{path} {size}: {len}");
            assert!(status == 200 || status >= 400);
            if len != 0 {
                serde_json::from_slice::<Value>(&output[..len]).unwrap();
            }
            unchanged(&s, &disk, &before, writes);
        }
    }
}

#[test]
fn session_and_code_capacity_expire_at_the_exact_boundary() {
    let (s, _) = house();
    let mut auth = Auth::default();
    let verifier = "a".repeat(43);
    let challenge = nanacoin::auth::challenge(&verifier).unwrap();
    let redirect = "https://client.local/";
    let mut codes = Vec::new();
    for _ in 0..MAX_CODES {
        codes.push(
            auth.authorize(s.state(), "alice", "1234", &challenge, "S256", redirect, 0)
                .unwrap(),
        );
    }
    assert_eq!(
        auth.authorize(
            s.state(),
            "alice",
            "1234",
            &challenge,
            "S256",
            redirect,
            CODE_TTL - 1
        ),
        Err(Error::Capacity)
    );
    assert_eq!(
        auth.redeem(s.state(), &codes[0], &verifier, redirect, CODE_TTL),
        Err(Error::Unauthorized)
    );
    let mut sessions = Vec::new();
    for _ in 0..MAX_SESSIONS {
        let code = auth
            .authorize(
                s.state(),
                "alice",
                "1234",
                &challenge,
                "S256",
                redirect,
                CODE_TTL,
            )
            .unwrap();
        sessions.push(
            auth.redeem(s.state(), &code, &verifier, redirect, CODE_TTL)
                .unwrap()
                .0,
        );
    }
    let code = auth
        .authorize(
            s.state(),
            "alice",
            "1234",
            &challenge,
            "S256",
            redirect,
            CODE_TTL,
        )
        .unwrap();
    assert_eq!(
        auth.redeem(s.state(), &code, &verifier, redirect, CODE_TTL),
        Err(Error::Capacity)
    );
    assert_eq!(
        auth.redeem(s.state(), &code, &verifier, redirect, CODE_TTL),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        auth.lookup(s.state(), &sessions[0], CODE_TTL + SESSION_TTL - 1),
        Ok(MemberId(2))
    );
    assert_eq!(
        auth.lookup(s.state(), &sessions[0], CODE_TTL + SESSION_TTL),
        Err(Error::Unauthorized)
    );
    let at = CODE_TTL + SESSION_TTL;
    let code = auth
        .authorize(s.state(), "alice", "1234", &challenge, "S256", redirect, at)
        .unwrap();
    let token = auth
        .redeem(s.state(), &code, &verifier, redirect, at)
        .unwrap()
        .0;
    auth.revoke_member(MemberId(2));
    assert_eq!(auth.lookup(s.state(), &token, at), Err(Error::Unauthorized));
}

#[test]
fn generated_payments_replay_against_an_independent_balance_model() {
    let (mut s, disk) = house();
    let mut expected = [1000i64; 3];
    let mut seed = 0x1234_5678u32;
    for turn in 0..300 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let from = (seed as usize) % 3;
        let to = (from + 1 + ((seed >> 8) as usize % 2)) % 3;
        let amount = ((seed >> 16) % 1500 + 1) as i64;
        let command = Command::Transfer {
            to: MemberId((to + 2) as u8),
            amount,
            memo: Memo::new(),
        };
        let key = format!("g0:model-{turn}");
        let writes = disk.frames.borrow().len();
        let result = s.execute_keyed(MemberId((from + 2) as u8), &key, command.clone());
        if expected[from] >= amount {
            let receipt = result.unwrap();
            expected[from] -= amount;
            expected[to] += amount;
            assert_eq!(
                s.execute_keyed(MemberId((from + 2) as u8), &key, command)
                    .unwrap(),
                Receipt {
                    replayed: true,
                    ..receipt
                }
            );
            assert_eq!(disk.frames.borrow().len(), writes + 1);
        } else {
            assert!(result.is_err());
            assert_eq!(disk.frames.borrow().len(), writes);
        }
        if turn % 13 == 0 {
            s = Service::open_with_clock(disk.clone(), now).unwrap();
        }
        for (i, balance) in expected.iter().enumerate() {
            assert_eq!(
                s.state().member(MemberId((i + 2) as u8)).unwrap().balance,
                *balance,
                "turn {turn}"
            );
        }
        assert_eq!(expected.iter().sum::<i64>(), 3000);
        s.state().check_invariants().unwrap();
    }
}

#[test]
fn failed_and_ambiguous_keyed_writes_latch_and_recover_exactly_once() {
    for mode in [1, 2] {
        for command in [
            Command::Transfer {
                to: MemberId(3),
                amount: 7,
                memo: Memo::new(),
            },
            Command::Transfer {
                to: MemberId(3),
                amount: 0,
                memo: "private mail".try_into().unwrap(),
            },
            Command::List {
                side: Side::Sell,
                title: "sale".try_into().unwrap(),
                description: Memo::new(),
                price: 7,
                details: Default::default(),
            },
        ] {
            let (mut s, disk) = house();
            let before = serde_json::to_value(s.state()).unwrap();
            let writes = disk.frames.borrow().len();
            disk.failure.set(mode);
            assert_eq!(
                s.execute_keyed(MemberId(2), "g0:lost", command.clone()),
                Err(Error::Storage)
            );
            assert!(s.storage_failed());
            assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
            assert_eq!(
                s.execute_keyed(MemberId(2), "g0:lost", command.clone()),
                Err(Error::Storage)
            );
            assert_eq!(disk.frames.borrow().len(), writes + usize::from(mode == 2));
            disk.failure.set(0);
            let mut restored = Service::open_with_clock(disk.clone(), now).unwrap();
            let result = restored
                .execute_keyed(MemberId(2), "g0:lost", command)
                .unwrap();
            assert_eq!(result.replayed, mode == 2);
            assert_eq!(disk.frames.borrow().len(), writes + 1);
            restored.state().check_invariants().unwrap();
        }
    }
}

#[test]
fn a_failed_response_after_commit_can_be_retried_after_restart_without_paying_twice() {
    for size in [0, 1, 8, 32] {
        let (mut s, disk) = house();
        let alice = common::login_as(&mut s, "alice", "1234");
        let body = br#"{"to":"account-3","amount":7}"#;
        let mut output = vec![0; size];
        let (status, len) = api::handle_keyed(
            &mut s,
            "POST",
            "/api/v1/transfers",
            &alice,
            "g0:lost-response",
            body,
            &mut output,
        );
        assert_eq!(status, 507);
        assert!(len <= size);
        assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 993);
        let writes = disk.frames.borrow().len();
        drop(s);
        let mut s = Service::open_with_clock(disk.clone(), now).unwrap();
        let alice = common::login_as(&mut s, "alice", "1234");
        assert_eq!(
            request(
                &mut s,
                "POST",
                "/api/v1/transfers",
                &alice,
                "g0:lost-response",
                body
            )
            .0,
            201
        );
        assert_eq!(disk.frames.borrow().len(), writes);
        let before = serde_json::to_value(s.state()).unwrap();
        assert_eq!(
            request(
                &mut s,
                "POST",
                "/api/v1/transfers",
                &alice,
                "g0:lost-response",
                br#"{"to":"account-3","amount":8}"#
            )
            .0,
            409
        );
        unchanged(&s, &disk, &before, writes);
    }
}

#[test]
fn reform_preview_and_stale_admin_actions_never_mutate_the_economy() {
    let (mut s, disk) = house();
    let nana = common::login(&mut s);
    let before = serde_json::to_value(s.state()).unwrap();
    let writes = disk.frames.borrow().len();
    let sequence = s.state().sequence;
    for (decimals, power, epoch, seq, expected) in [
        (5, 0, 0, sequence, 200),
        (9, 0, 0, sequence, 400),
        (5, 13, 0, sequence, 400),
        (5, 0, 1, sequence, 409),
        (5, 0, 0, sequence + 1, 409),
    ] {
        let body = serde_json::to_vec(&json!({"decimals":decimals,"power":power,"expected_epoch":epoch,"expected_sequence":seq,"preview":true})).unwrap();
        let (status, response) = request(&mut s, "POST", "/api/v1/admin/reform", &nana, "", &body);
        assert_eq!(status, expected, "{response}");
        if status == 200 {
            assert_eq!(response["circulation"], 30000);
            assert_eq!(response["preview"], true);
        }
        unchanged(&s, &disk, &before, writes);
    }
    for path in ["/api/v1/admin/reset", "/api/v1/admin/checkpoint"] {
        for body in [
            json!({"expected_generation":1,"expected_sequence":sequence,"confirmation":"RESET ECONOMY"}),
            json!({"expected_generation":0,"expected_sequence":sequence + 1,"confirmation":"RESET ECONOMY"}),
        ] {
            assert_eq!(
                request(
                    &mut s,
                    "POST",
                    path,
                    &nana,
                    "",
                    &serde_json::to_vec(&body).unwrap()
                )
                .0,
                409
            );
            unchanged(&s, &disk, &before, writes);
        }
    }
}

#[test]
fn listing_edits_and_lifecycle_reject_other_owners_stale_actions_and_bad_metadata() {
    let (mut s, disk) = house();
    let alice = common::login_as(&mut s, "alice", "1234");
    let bob = common::login_as(&mut s, "bob", "1234");
    let created = request(
        &mut s,
        "POST",
        "/api/v1/listings",
        &alice,
        "",
        br#"{"title":"work","price":7,"side":"SELL"}"#,
    );
    assert_eq!(created.0, 201, "{:?}", created.1);
    let path = format!("/api/v1/listings/{}", created.1["id"].as_str().unwrap());
    let before = serde_json::to_value(s.state()).unwrap();
    let writes = disk.frames.borrow().len();
    for (auth, body, expected) in [
        (&bob, br#"{"price":8}"#.as_slice(), 403),
        (&alice, br#"{"price":-1}"#, 400),
        (&alice, br#"{"title":" "}"#, 400),
        (&alice, br#"{"title":"one","title":"two"}"#, 400),
    ] {
        assert_eq!(request(&mut s, "PATCH", &path, auth, "", body).0, expected);
        unchanged(&s, &disk, &before, writes);
    }
    let edited = request(
        &mut s,
        "PATCH",
        &path,
        &alice,
        "",
        r#"{"title":"new 🎁","description":"updated","price":9}"#.as_bytes(),
    );
    assert_eq!(edited.0, 200);
    assert_eq!(edited.1["title"], "new 🎁");
    for status in ["ACTIVE", "SOLD", "CANCELLED"] {
        let (code, result) = request(
            &mut s,
            "GET",
            &format!("/api/v1/listings?status={status}"),
            &bob,
            "",
            b"",
        );
        assert_eq!(code, 200);
        assert_eq!(
            result["listings"].as_array().unwrap().len(),
            usize::from(status == "ACTIVE")
        );
    }
    let bought = request(
        &mut s,
        "POST",
        &format!("{path}/purchase"),
        &bob,
        "g0:buy",
        b"{}",
    );
    assert_eq!(bought.0, 201, "{:?}", bought.1);
    let writes = disk.frames.borrow().len();
    assert_eq!(
        request(
            &mut s,
            "POST",
            &format!("{path}/purchase"),
            &bob,
            "g0:buy",
            b"{}"
        )
        .1,
        bought.1
    );
    assert_eq!(disk.frames.borrow().len(), writes);
    let before = serde_json::to_value(s.state()).unwrap();
    assert!(
        request(
            &mut s,
            "POST",
            &format!("{path}/purchase"),
            &bob,
            "g0:second-buy",
            b"{}"
        )
        .0 >= 400
    );
    assert!(request(&mut s, "POST", &format!("{path}/cancel"), &alice, "", b"{}").0 >= 400);
    unchanged(&s, &disk, &before, writes);
}

#[test]
fn api_key_scopes_cannot_escape_through_alternate_credential_routes() {
    let (mut s, disk) = house();
    let alice = common::login_as(&mut s, "alice", "1234");
    for scope in ["full", "read"] {
        let body = serde_json::to_vec(&json!({"password":"1234","scope":scope})).unwrap();
        let (code, response) = request(&mut s, "POST", "/api/v1/me/api-key", &alice, "", &body);
        assert_eq!(code, 200, "{response}");
        let key = format!("Bearer {}", response["api_key"].as_str().unwrap());
        assert_eq!(request(&mut s, "GET", "/api/v1/me", &key, "", b"").0, 200);
        let before = serde_json::to_value(s.state()).unwrap();
        let writes = disk.frames.borrow().len();
        for (method, path, body) in [
            (
                "POST",
                "/api/v1/me/api-key",
                br#"{"password":"1234"}"#.as_slice(),
            ),
            ("DELETE", "/api/v1/me/api-key", b""),
            ("PATCH", "/api/v1/users/user-2", br#"{"password":"5678"}"#),
            (
                "POST",
                "/api/v1/users/update",
                br#"{"member":2,"password":"5678"}"#,
            ),
            ("PATCH", "/api/v1/users/user-2", b"{"),
            ("POST", "/api/v1/users/user-3/api-key", b"{}"),
        ] {
            assert_eq!(
                request(&mut s, method, path, &key, "g0:key-escape", body).0,
                403,
                "{scope} {method} {path}"
            );
            unchanged(&s, &disk, &before, writes);
        }
        assert_eq!(
            request(&mut s, "GET", "/api/v1/minicloud/session", &key, "", b"").0,
            403
        );
        let path = format!("/api/v1/me/api-key?scope={scope}");
        assert_eq!(request(&mut s, "DELETE", &path, &alice, "", b"").0, 200);
        assert_eq!(request(&mut s, "GET", "/api/v1/me", &key, "", b"").0, 401);
    }
}

#[test]
fn credit_waiting_states_and_clock_rollback_are_read_only_and_private() {
    let (mut s, disk) = house();
    let alice = common::login_as(&mut s, "alice", "1234");
    let bob = common::login_as(&mut s, "bob", "1234");
    let outsider = common::login_as(&mut s, "outsider", "1234");
    let terms = json!({"borrower":"account-3","amount":2000,"rate_bps":100,"rate_days":365,"payment_days":7,"installment":500,"credit":true,"memo":"SECRET loan note"});
    let (status, loan) = request(
        &mut s,
        "POST",
        "/api/v1/loans",
        &alice,
        "g0:credit",
        &serde_json::to_vec(&terms).unwrap(),
    );
    assert_eq!(status, 200, "{loan}");
    let id = loan["id"].as_u64().unwrap();
    assert_eq!(
        request(
            &mut s,
            "POST",
            &format!("/api/v1/loans/{id}/accept"),
            &bob,
            "g0:accept-credit",
            b"{}"
        )
        .0,
        200
    );
    let waiting = |s: &mut Service<Memory>, disk: &Memory, expected: &str| {
        let before = serde_json::to_value(s.state()).unwrap();
        let writes = disk.frames.borrow().len();
        let (status, book) = request(s, "GET", "/api/v1/loans", &bob, "", b"");
        assert_eq!(status, 200);
        assert_eq!(book["loans"][0]["waiting_reason"], expected);
        unchanged(s, disk, &before, writes);
    };
    waiting(&mut s, &disk, "Waiting for a zero balance");
    exec(
        &mut s,
        3,
        Command::Transfer {
            to: MemberId(4),
            amount: 1000,
            memo: Memo::new(),
        },
    )
    .unwrap();
    waiting(&mut s, &disk, "Waiting for lender funds");
    exec(
        &mut s,
        1,
        Command::Issue {
            to: MemberId(2),
            amount: 1000,
            memo: Memo::new(),
        },
    )
    .unwrap();
    waiting(&mut s, &disk, "Ready for automatic funding");
    assert_eq!(s.tick().unwrap(), 1);
    assert_eq!(s.state().member(MemberId(3)).unwrap().balance, 2000);
    assert_eq!(s.tick().unwrap(), 0);
    let (_, book) = request(
        &mut s,
        "GET",
        "/api/v1/loans?member=user-3",
        &outsider,
        "",
        b"",
    );
    assert_eq!(book["loans"][0]["memo"], "");
    assert!(!book.to_string().contains("SECRET"));
    CLOCK.with(|clock| clock.set(NOW - 1));
    waiting(
        &mut s,
        &disk,
        "Clock or arithmetic limit; payment is paused",
    );
    CLOCK.with(|clock| clock.set(NOW));
    exec(
        &mut s,
        1,
        Command::UpdateMember {
            member: MemberId(2),
            display_name: None,
            password: None,
            role: None,
            disabled: Some(true),
            mastodon_id: None,
            bio: None,
        },
    )
    .unwrap();
    waiting(&mut s, &disk, "An account is disabled");
}
