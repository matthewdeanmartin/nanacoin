mod common;
use nanacoin::{
    api,
    auth::{self, Auth, PasswordVerifier},
    domain::*,
    journal::*,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>);
impl Journal for Memory {
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(value) = self.0.borrow().get(index) {
            *frame = *value;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, _: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        self.0.borrow_mut().push(*frame);
        Ok(())
    }
}
fn setup() -> (Service<Memory>, Memory) {
    let memory = Memory::default();
    let mut s = Service::open(memory.clone()).unwrap();
    common::provision(&mut s);
    s.execute(
        MemberId(1),
        2,
        Command::CreateMember {
            username: Name::try_from("alice").unwrap(),
            display_name: Name::try_from("Alice").unwrap(),
            password: PasswordVerifier::hash("5678").unwrap(),
            role: Role::User,
            grant: 0,
            mastodon_id: MastodonId::new(),
        },
    )
    .unwrap();
    (s, memory)
}
fn get<J: Journal>(s: &mut Service<J>, auth: &str, path: &str) -> (u16, serde_json::Value) {
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let (status, len) = api::handle(s, "GET", path, auth, &[], &mut out);
    (status, serde_json::from_slice(&out[..len]).unwrap())
}

fn patch<J: Journal>(
    s: &mut Service<J>,
    auth: &str,
    path: &str,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let body = serde_json::to_vec(&body).unwrap();
    let (status, len) = api::handle(s, "PATCH", path, auth, &body, &mut out);
    (status, serde_json::from_slice(&out[..len]).unwrap())
}

#[test]
fn provisions_once_and_uses_the_existing_angular_login_shapes() {
    let mut s = Service::open(Memory::default()).unwrap();
    assert_eq!(get(&mut s, "", "/api/v1/status").1["provisioned"], false);
    let body = json!({"household_name":"Home", "username":"nana", "display_name":"Nana", "password":"1234"});
    let (status, user) = common::call(&mut s, "/api/v1/provision", "", body.clone());
    assert_eq!(status, 201);
    assert_eq!(user["id"], "user-1");
    assert_eq!(user["role"], "nana");
    assert_eq!(common::call(&mut s, "/api/v1/provision", "", body).0, 403);
    let header = common::login(&mut s);
    let (status, me) = get(&mut s, &header, "/api/v1/me");
    assert_eq!(status, 200);
    assert_eq!(me["username"], "nana");
    assert_eq!(me["account"], "account-1");
    assert!(get(&mut s, &header, "/api/v1/users").1["users"].is_array());
    assert!(get(&mut s, &header, "/api/v1/listings").1["listings"].is_array());
    assert_eq!(get(&mut s, "", "/").0, 404);
}

#[test]
fn mastodon_ids_are_bounded_persistent_member_metadata() {
    let (mut s, memory) = setup();
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "alice", "5678");
    let (status, user) = patch(
        &mut s,
        &alice,
        "/api/v1/users/user-2",
        json!({"mastodon_id":"@alice@example.social"}),
    );
    assert_eq!(status, 200, "{user}");
    assert_eq!(user["mastodon_id"], "@alice@example.social");
    assert_eq!(
        patch(
            &mut s,
            &alice,
            "/api/v1/users/user-1",
            json!({"mastodon_id":"@nana@example.social"})
        )
        .0,
        403
    );
    assert_eq!(
        patch(
            &mut s,
            &nana,
            "/api/v1/users/user-2",
            json!({"mastodon_id":"not-a-handle"})
        )
        .0,
        400
    );
    drop(s);
    let mut replay = Service::open(memory).unwrap();
    let nana = common::login(&mut replay);
    assert_eq!(
        get(&mut replay, &nana, "/api/v1/users").1["users"][1]["mastodon_id"],
        "@alice@example.social"
    );
}

#[test]
fn pin_rule_salted_hashes_and_persistent_credentials() {
    assert_eq!(PasswordVerifier::hash("123"), Err(Error::InvalidInput));
    let first = PasswordVerifier::hash("1234").unwrap();
    let second = PasswordVerifier::hash("1234").unwrap();
    assert_ne!(first, second);
    assert!(first.verify("1234"));
    assert!(!first.verify("4321"));
    let (mut s, memory) = setup();
    let token = common::login(&mut s);
    drop(s);
    let mut s = Service::open(memory).unwrap();
    assert_eq!(get(&mut s, &token, "/api/v1/me").0, 401);
    assert!(!common::login(&mut s).is_empty());
    assert!(!common::login_as(&mut s, "alice", "5678").is_empty());
}

#[test]
fn rejects_plain_wrong_verifier_redirect_replay_and_expired_codes() {
    let (s, _) = setup();
    let mut a = Auth::default();
    let verifier = "a".repeat(43);
    let challenge = auth::challenge(&verifier).unwrap();
    assert_eq!(
        a.authorize(
            s.state(),
            "nana",
            "1234",
            &challenge,
            "plain",
            "http://localhost/",
            0
        ),
        Err(Error::InvalidInput)
    );
    for (bad_verifier, redirect) in [
        ("b".repeat(43), "http://localhost/"),
        (verifier.clone(), "http://elsewhere/"),
    ] {
        let code = a
            .authorize(
                s.state(),
                "nana",
                "1234",
                &challenge,
                "S256",
                "http://localhost/",
                0,
            )
            .unwrap();
        assert_eq!(
            a.redeem(s.state(), &code, &bad_verifier, redirect, 1),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            a.redeem(s.state(), &code, &verifier, "http://localhost/", 2),
            Err(Error::Unauthorized)
        );
    }
    let code = a
        .authorize(
            s.state(),
            "nana",
            "1234",
            &challenge,
            "S256",
            "http://localhost/",
            0,
        )
        .unwrap();
    assert_eq!(
        a.redeem(s.state(), &code, &verifier, "http://localhost/", 60),
        Err(Error::Unauthorized)
    );
    let code = a
        .authorize(
            s.state(),
            "nana",
            "1234",
            &challenge,
            "S256",
            "http://localhost/",
            60,
        )
        .unwrap();
    let (token, _) = a
        .redeem(s.state(), &code, &verifier, "http://localhost/", 61)
        .unwrap();
    assert_eq!(a.lookup(s.state(), &token, 61), Ok(MemberId(1)));
    assert_eq!(
        a.lookup(s.state(), &token, 61 + auth::SESSION_TTL),
        Err(Error::Unauthorized)
    );
}

#[test]
fn failed_logins_are_rate_limited_and_do_not_enumerate_usernames() {
    let (s, _) = setup();
    let mut a = Auth::default();
    let challenge = auth::challenge(&"a".repeat(43)).unwrap();
    for username in ["nana", "unknown"] {
        for _ in 0..5 {
            assert_eq!(
                a.authorize(
                    s.state(),
                    username,
                    "wrong",
                    &challenge,
                    "S256",
                    "http://localhost/",
                    0
                ),
                Err(Error::InvalidCredentials)
            );
        }
        assert_eq!(
            a.authorize(
                s.state(),
                username,
                "1234",
                &challenge,
                "S256",
                "http://localhost/",
                1
            ),
            Err(Error::RateLimited)
        );
    }
    assert!(a
        .authorize(
            s.state(),
            "nana",
            "1234",
            &challenge,
            "S256",
            "http://localhost/",
            auth::LOCKOUT
        )
        .is_ok());
}

#[test]
fn logout_password_reset_and_disable_revoke_sessions_and_pending_codes() {
    let (mut s, _) = setup();
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "alice", "5678");
    assert_eq!(
        common::call(&mut s, "/api/v1/auth/logout", &alice, json!({})).0,
        204
    );
    assert_eq!(get(&mut s, &alice, "/api/v1/me").0, 401);
    let alice = common::login_as(&mut s, "alice", "5678");
    let verifier = "a".repeat(43);
    let challenge = auth::challenge(&verifier).unwrap();
    let (_, pending) = common::call(
        &mut s,
        "/api/v1/auth/authorize",
        "",
        json!({"username":"alice","password":"5678","code_challenge":challenge.as_str(),"code_challenge_method":"S256","redirect_uri":"http://localhost/"}),
    );
    assert_eq!(
        common::call(
            &mut s,
            "/api/v1/users/update",
            &nana,
            json!({"member":2,"password":"9999"})
        )
        .0,
        200
    );
    assert_eq!(get(&mut s, &alice, "/api/v1/me").0, 401);
    assert_eq!(common::call(&mut s,"/api/v1/auth/token","",json!({"code":pending["code"],"code_verifier":verifier,"redirect_uri":"http://localhost/"})).0,401);
    let alice = common::login_as(&mut s, "alice", "9999");
    assert_eq!(
        common::call(
            &mut s,
            "/api/v1/users/update",
            &nana,
            json!({"member":2,"disabled":true})
        )
        .0,
        200
    );
    assert_eq!(get(&mut s, &alice, "/api/v1/me").0, 401);
}

#[test]
fn member_cannot_promote_self_and_last_nana_cannot_be_disabled() {
    let (mut s, _) = setup();
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "alice", "5678");
    for body in [
        json!({"member":2,"role":"nana"}),
        json!({"member":1,"password":"9999"}),
    ] {
        assert_eq!(
            common::call(&mut s, "/api/v1/users/update", &alice, body).0,
            403
        );
    }
    for body in [
        json!({"member":1,"role":"user"}),
        json!({"member":1,"disabled":true}),
    ] {
        assert_eq!(
            common::call(&mut s, "/api/v1/users/update", &nana, body).0,
            403
        );
    }
    assert_eq!(
        common::call(
            &mut s,
            "/api/v1/users/update",
            &alice,
            json!({"member":2,"password":"9999"})
        )
        .0,
        200
    );
    assert_eq!(get(&mut s, &alice, "/api/v1/me").0, 401);
}

#[test]
fn legacy_tokens_are_migration_only_and_the_ledger_is_preserved() {
    let memory = Memory::default();
    let mut s = Service::open(memory.clone()).unwrap();
    let old = "0123456789abcdef0123456789abcdef";
    s.execute(
        MemberId(1),
        1,
        Command::AddMember {
            name: Name::try_from("Nana").unwrap(),
            token_hash: token_hash(old).unwrap(),
        },
    )
    .unwrap();
    s.execute(
        MemberId(1),
        2,
        Command::Issue {
            to: MemberId(1),
            amount: 25,
            memo: Memo::new(),
        },
    )
    .unwrap();
    assert_eq!(get(&mut s, &format!("Bearer {old}"), "/api/v1/me").0, 401);
    s.execute(
        MemberId(1),
        3,
        Command::MigrateMember {
            member: MemberId(1),
            username: Name::try_from("nana").unwrap(),
            password: PasswordVerifier::hash("1234").unwrap(),
        },
    )
    .unwrap();
    let mut s = Service::open(memory).unwrap();
    let auth = common::login(&mut s);
    assert_eq!(get(&mut s, &auth, "/api/v1/me").1["balance"], 25);
    assert_eq!(s.state().authenticate_legacy(old), Err(Error::Unauthorized));
}

#[test]
fn angular_idempotency_keys_survive_intervening_writes_and_restart() {
    let (mut s, memory) = setup();
    let cmd = Command::Issue {
        to: MemberId(2),
        amount: 10,
        memo: Memo::new(),
    };
    let original = s
        .execute_keyed(MemberId(1), "random-key", cmd.clone())
        .unwrap();
    for i in 0..80 {
        s.execute_keyed(MemberId(1), &format!("another-{i}"), cmd.clone())
            .unwrap();
    }
    let mut s = Service::open(memory).unwrap();
    assert_eq!(
        s.execute_keyed(MemberId(1), "random-key", cmd),
        Ok(Receipt {
            replayed: true,
            ..original
        })
    );
    assert_eq!(s.state().member(MemberId(2)).unwrap().balance, 810);
    assert_eq!(
        s.execute_keyed(
            MemberId(1),
            "random-key",
            Command::Issue {
                to: MemberId(2),
                amount: 11,
                memo: Memo::new()
            }
        ),
        Err(Error::Conflict)
    );
}

#[test]
fn request_cannot_bypass_server_side_password_hashing() {
    let (mut s, _) = setup();
    let nana = common::login(&mut s);
    let body = json!({"request_id":3,"command":{"add_member":{"name":"Backdoor","token_hash":vec![1;32]}}});
    assert_eq!(common::call(&mut s, "/api/v1/commands", &nana, body).0, 403);
}
