//! Household email: verification, recovery, member mail, limits, delivery
//! semantics, and the Brevo transport, without real network or provider.
mod common;
use miniframework::{
    fetch::{Fetch, Fetched, Outgoing},
    kv::{Kv, MemKv},
};
use nanacoin::{
    api,
    auth::PasswordVerifier,
    domain::*,
    email::{self, Brevo, Mailer, Message, Provider},
    journal::*,
};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Midnight UTC plus an hour, so a test can run many cooldowns in one day.
const START: u64 = 20_834 * 86_400 + 3_600;
thread_local! { static NOW: Cell<u64> = const { Cell::new(START) }; }
fn clock() -> u64 {
    NOW.with(Cell::get)
}
fn advance(seconds: u64) {
    NOW.with(|n| n.set(n.get() + seconds));
}

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

/// Records deliveries; fails every send while `fail` is set.
#[derive(Clone, Default)]
struct Outbox {
    sent: Arc<Mutex<Vec<Message>>>,
    fail: Option<String>,
}
impl Provider for Outbox {
    fn name(&self) -> &'static str {
        "Test"
    }
    fn send(&mut self, message: &Message) -> Result<(), String> {
        self.sent.lock().unwrap().push(message.clone());
        self.fail.clone().map_or(Ok(()), Err)
    }
}

struct Bank {
    s: Service<Memory>,
    kv: Arc<MemKv>,
    mail: Mailer,
    outbox: Outbox,
}
impl Bank {
    fn new() -> Self {
        let mut s = Service::open_with_clock(Memory::default(), clock).unwrap();
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
        let kv = Arc::new(MemKv::default());
        let mail = Self::open(&kv);
        Self {
            s,
            kv,
            mail,
            outbox: Outbox::default(),
        }
    }
    fn open(kv: &Arc<MemKv>) -> Mailer {
        Mailer::open(kv.clone(), "Test", "https://bank.example/email".into())
            .unwrap_or_else(|_| panic!("open"))
    }
    /// Simulates a server restart: durable preferences and queue, fresh RAM.
    fn restart(&mut self) {
        self.mail = Self::open(&self.kv);
    }
    fn call(&mut self, method: &str, path: &str, auth: &str, body: Value) -> (u16, Value) {
        let mut out = vec![0; api::RESPONSE_LIMIT];
        let body = if method == "GET" {
            vec![]
        } else {
            serde_json::to_vec(&body).unwrap()
        };
        let (status, len) = self
            .mail
            .handle(&mut self.s, method, path, auth, &body, &mut out);
        (
            status,
            serde_json::from_slice(&out[..len]).unwrap_or(Value::Null),
        )
    }
    fn post(&mut self, action: &str, auth: &str, body: Value) -> u16 {
        self.call("POST", &format!("/api/v1/email/{action}"), auth, body)
            .0
    }
    fn settings(&mut self, auth: &str) -> (u16, Value) {
        self.call("GET", "/api/v1/email", auth, Value::Null)
    }
    /// Runs the worker until the queue is empty; returns what was handed to the provider.
    fn deliver(&mut self) -> Vec<Message> {
        let before = self.outbox.sent.lock().unwrap().len();
        for _ in 0..email::QUEUE + 1 {
            self.mail.run_once(&mut self.outbox, clock());
        }
        self.outbox.sent.lock().unwrap()[before..].to_vec()
    }
    /// Alice verifies alice@example.com; returns her session.
    fn verified_alice(&mut self) -> String {
        let alice = common::login_as(&mut self.s, "alice", "5678");
        assert_eq!(
            self.post(
                "address",
                &alice,
                json!({"address":" Alice@Example.com ","password":"5678"})
            ),
            200
        );
        let sent = self.deliver();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, "alice@example.com");
        assert_eq!(
            self.post("verify", &alice, json!({"code": code(&sent[0])})),
            200
        );
        alice
    }
}

fn code(message: &Message) -> String {
    message
        .text
        .split_whitespace()
        .find(|w| w.len() == 32 && w.bytes().all(|b| b.is_ascii_hexdigit()))
        .expect("message carries a code")
        .to_owned()
}

#[test]
fn verify_then_recover_resets_password_once_and_revokes_sessions() {
    let mut b = Bank::new();
    let alice = common::login_as(&mut b.s, "alice", "5678");
    assert_eq!(
        b.settings(&alice).1,
        json!({"provider":"Test","address":"","notifications":false,"queued":0,"last_error":""})
    );

    // The current password is required, and a wrong code does not verify.
    assert_eq!(
        b.post(
            "address",
            &alice,
            json!({"address":"alice@example.com","password":"0000"})
        ),
        401
    );
    advance(61);
    assert_eq!(
        b.post(
            "address",
            &alice,
            json!({"address":"alice@example.com","password":"5678"})
        ),
        200
    );
    let sent = b.deliver();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        b.post("verify", &alice, json!({"code": "0".repeat(32)})),
        401
    );
    assert_eq!(
        b.post("verify", &alice, json!({"code": code(&sent[0])})),
        200
    );
    assert_eq!(b.settings(&alice).1["address"], "alice@example.com");
    assert_eq!(b.settings(&alice).1["notifications"], true);

    // Username lookup is case-insensitive; the email goes only to the verified address.
    assert_eq!(b.post("recover", "", json!({"username":"ALICE"})), 200);
    let sent = b.deliver();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "alice@example.com");
    let recovery = code(&sent[0]);
    // A verification code is not a recovery code.
    assert_eq!(
        b.post(
            "reset",
            "",
            json!({"code": "f".repeat(32), "password":"9999"})
        ),
        401
    );
    assert_eq!(
        b.post("reset", "", json!({"code": recovery, "password":"9999"})),
        200
    );

    // Old sessions are revoked, the new password works, and the code is single use.
    assert_eq!(b.settings(&alice).0, 401);
    common::login_as(&mut b.s, "alice", "9999");
    assert_eq!(
        b.post("reset", "", json!({"code": recovery, "password":"1111"})),
        401
    );
}

#[test]
fn recovery_answers_identically_and_sends_nothing_without_a_verified_address() {
    let mut b = Bank::new();
    for username in ["nobody", "nana", "alice"] {
        assert_eq!(
            b.call(
                "POST",
                "/api/v1/email/recover",
                "",
                json!({"username": username})
            ),
            (200, json!(true))
        );
    }
    assert!(b.deliver().is_empty());
}

#[test]
fn recovery_codes_die_with_a_password_change_expiry_or_restart() {
    let mut b = Bank::new();
    b.verified_alice();

    // A password changed elsewhere invalidates an outstanding code.
    b.post("recover", "", json!({"username":"alice"}));
    let stale = code(&b.deliver()[0]);
    let nonce = b.s.state().member(MemberId(2)).unwrap().last_request + 1;
    b.s.execute(
        MemberId(2),
        nonce,
        Command::UpdateMember {
            member: MemberId(2),
            display_name: None,
            password: Some(PasswordVerifier::hash("4321").unwrap()),
            role: None,
            disabled: None,
            mastodon_id: None,
            bio: None,
        },
    )
    .unwrap();
    assert_eq!(
        b.post("reset", "", json!({"code": stale, "password":"9999"})),
        401
    );

    // Expiry.
    advance(email::RECOVERY_COOLDOWN);
    b.post("recover", "", json!({"username":"alice"}));
    let expired = code(&b.deliver()[0]);
    advance(email::TTL);
    assert_eq!(
        b.post("reset", "", json!({"code": expired, "password":"9999"})),
        401
    );

    // Restart: challenges live only in RAM.
    advance(email::RECOVERY_COOLDOWN);
    b.post("recover", "", json!({"username":"alice"}));
    let lost = code(&b.deliver()[0]);
    b.restart();
    assert_eq!(
        b.post("reset", "", json!({"code": lost, "password":"9999"})),
        401
    );
    common::login_as(&mut b.s, "alice", "4321");
}

#[test]
fn verification_needs_the_same_member_and_unchanged_password() {
    let mut b = Bank::new();
    let alice = common::login_as(&mut b.s, "alice", "5678");
    let nana = common::login(&mut b.s);
    b.post(
        "address",
        &alice,
        json!({"address":"alice@example.com","password":"5678"}),
    );
    let sent = b.deliver();
    assert_eq!(
        b.post("verify", &nana, json!({"code": code(&sent[0])})),
        401
    );
    assert_eq!(b.settings(&nana).1["address"], "");
    assert_eq!(
        b.post("verify", &alice, json!({"code": code(&sent[0])})),
        200
    );
}

#[test]
fn api_keys_cannot_read_or_change_email() {
    let mut b = Bank::new();
    let alice = b.verified_alice();
    let (status, made) = common::call(
        &mut b.s,
        "/api/v1/me/api-key",
        &alice,
        json!({"password":"5678"}),
    );
    assert_eq!(status, 200, "{made}");
    let key = format!("Bearer {}", made["api_key"].as_str().unwrap());
    assert_eq!(b.settings(&key).0, 401);
    assert_eq!(
        b.post(
            "preferences",
            &key,
            json!({"notifications":false,"remove":true})
        ),
        401
    );
    assert_eq!(
        b.post("invite", &key, json!({"address":"x@example.com"})),
        401
    );
    assert_eq!(
        b.post(
            "address",
            "",
            json!({"address":"x@example.com","password":"5678"})
        ),
        401
    );
}

#[test]
fn member_email_respects_preferences_and_removal_drops_queued_mail() {
    let mut b = Bank::new();
    let alice = b.verified_alice();
    let nana = common::login(&mut b.s);

    assert_eq!(
        b.post(
            "notify",
            &nana,
            json!({"member":2,"subject":"Dinner","text":"Come at six."})
        ),
        200
    );
    let sent = b.deliver();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "alice@example.com");
    assert_eq!(sent[0].subject, "NanaCoin: Dinner");
    assert!(sent[0].text.contains("From Nana") && sent[0].text.contains("Come at six."));

    // No address, control characters, and empty bodies are refused.
    advance(61);
    assert_eq!(
        b.post(
            "notify",
            &alice,
            json!({"member":1,"subject":"Hi","text":"x"})
        ),
        404
    );
    advance(61);
    assert_eq!(
        b.post(
            "notify",
            &nana,
            json!({"member":2,"subject":"a\nBcc: x","text":"x"})
        ),
        400
    );
    advance(61);
    assert_eq!(
        b.post(
            "notify",
            &nana,
            json!({"member":2,"subject":"Hi","text":"  "})
        ),
        400
    );

    // Turning member email off refuses new mail but keeps recovery.
    assert_eq!(
        b.post("preferences", &alice, json!({"notifications":false})),
        200
    );
    advance(61);
    assert_eq!(
        b.post(
            "notify",
            &nana,
            json!({"member":2,"subject":"Hi","text":"x"})
        ),
        404
    );
    b.post("recover", "", json!({"username":"alice"}));
    assert_eq!(b.deliver().len(), 1);

    // Removal drops mail already queued to the address.
    assert_eq!(
        b.post("preferences", &alice, json!({"notifications":true})),
        200
    );
    advance(61);
    assert_eq!(
        b.post(
            "notify",
            &nana,
            json!({"member":2,"subject":"Hi","text":"x"})
        ),
        200
    );
    assert_eq!(b.settings(&nana).1["queued"], 1);
    assert_eq!(
        b.post(
            "preferences",
            &alice,
            json!({"notifications":false,"remove":true})
        ),
        200
    );
    assert_eq!(b.settings(&nana).1["queued"], 0);
    assert!(b.deliver().is_empty());
    assert_eq!(b.settings(&alice).1["address"], "");
}

#[test]
fn invitations_validate_the_address_and_grant_nothing() {
    let mut b = Bank::new();
    let nana = common::login(&mut b.s);
    assert_eq!(
        b.post("invite", &nana, json!({"address":"not-an-address"})),
        400
    );
    advance(61);
    assert_eq!(
        b.post("invite", &nana, json!({"address":"Cousin@Example.org"})),
        200
    );
    let sent = b.deliver();
    assert_eq!(sent[0].to, "cousin@example.org");
    assert!(sent[0].text.contains("https://bank.example/email"));
    assert!(sent[0].text.contains("does not create an account"));
}

#[test]
fn member_cooldown_and_daily_allowance_persist_across_restart() {
    let mut b = Bank::new();
    let nana = common::login(&mut b.s);
    assert_eq!(
        b.post("invite", &nana, json!({"address":"a@example.com"})),
        200
    );
    assert_eq!(
        b.post("invite", &nana, json!({"address":"a@example.com"})),
        429
    );
    for _ in 1..email::DAILY_LIMIT {
        advance(61);
        if b.mail.queued() >= email::QUEUE - 1 {
            b.deliver();
        }
        assert_eq!(
            b.post("invite", &nana, json!({"address":"a@example.com"})),
            200
        );
    }
    b.restart();
    advance(61);
    assert_eq!(
        b.post("invite", &nana, json!({"address":"a@example.com"})),
        429
    );
    advance(86_400);
    let nana = common::login(&mut b.s);
    assert_eq!(
        b.post("invite", &nana, json!({"address":"a@example.com"})),
        200
    );
}

#[test]
fn queue_is_bounded_and_expires() {
    let mut b = Bank::new();
    let alice = b.verified_alice();
    let nana = common::login(&mut b.s);
    // Nana and Alice alternate to fill the queue inside its lifetime.
    for i in 0..email::QUEUE {
        let who = if i % 2 == 0 { &nana } else { &alice };
        if i % 2 == 0 {
            advance(61);
        }
        assert_eq!(
            b.post("invite", who, json!({"address":"a@example.com"})),
            200,
            "message {i}"
        );
    }
    advance(61);
    assert_eq!(
        b.post("invite", &nana, json!({"address":"a@example.com"})),
        507
    );
    advance(email::TTL);
    assert!(
        b.deliver().is_empty(),
        "expired jobs are dropped, not sent late"
    );
}

#[test]
fn strangers_cannot_spend_the_members_allowance_or_block_other_accounts() {
    let mut b = Bank::new();
    b.verified_alice();
    // A stranger asks for Alice's recovery repeatedly; only one per cooldown is sent.
    assert_eq!(b.post("recover", "", json!({"username":"alice"})), 200);
    assert_eq!(b.post("recover", "", json!({"username":"alice"})), 200);
    assert_eq!(b.deliver().len(), 1);
    let mut sent = 1;
    for _ in 0..email::RECOVERY_DAILY_LIMIT * 2 {
        advance(email::RECOVERY_COOLDOWN);
        assert_eq!(b.post("recover", "", json!({"username":"alice"})), 200);
        sent += b.deliver().len() as u32;
    }
    assert_eq!(sent, email::RECOVERY_DAILY_LIMIT);
    // Recovery spam does not block members' own email.
    let nana = common::login(&mut b.s);
    assert_eq!(
        b.post(
            "notify",
            &nana,
            json!({"member":2,"subject":"Hi","text":"x"})
        ),
        200
    );
    assert_eq!(b.deliver().len(), 1);
}

#[test]
fn delivery_is_at_most_once_and_errors_are_shown_only_to_nana() {
    let mut b = Bank::new();
    let alice = b.verified_alice();
    let nana = common::login(&mut b.s);
    b.outbox.fail = Some("Email provider HTTP 500".into());
    b.post(
        "notify",
        &nana,
        json!({"member":2,"subject":"Hi","text":"x"}),
    );
    assert_eq!(b.deliver().len(), 1, "one attempt");
    assert!(b.deliver().is_empty(), "never retried");
    assert_eq!(b.settings(&nana).1["last_error"], "Email provider HTTP 500");
    assert_eq!(b.settings(&alice).1["last_error"], "");
    b.outbox.fail = None;
    advance(61);
    b.post(
        "notify",
        &nana,
        json!({"member":2,"subject":"Hi","text":"x"}),
    );
    b.deliver();
    assert_eq!(b.settings(&nana).1["last_error"], "");
}

#[test]
fn queued_mail_survives_restart_but_a_claimed_job_does_not_return() {
    let mut b = Bank::new();
    b.verified_alice();
    let nana = common::login(&mut b.s);
    b.post(
        "notify",
        &nana,
        json!({"member":2,"subject":"Hi","text":"x"}),
    );
    b.restart();
    assert_eq!(b.deliver().len(), 1);
    b.restart();
    assert!(b.deliver().is_empty());
    assert!(b.kv.get("email").is_some());
}

#[test]
fn addresses_are_validated() {
    for good in ["a@b.co", "first.last+tag@example.co.uk"] {
        assert!(email::valid_address(good), "{good}");
    }
    let long = format!("{}@example.com", "a".repeat(250));
    for bad in [
        "",
        "a",
        "@b.co",
        "a@b",
        "a@.b.co",
        "a@b.co.",
        "a@b@c.co",
        "a b@c.co",
        "a,b@c.co",
        "<a@b.co>",
        "a@b.co\r\nBcc: x@y.z",
        &long,
    ] {
        assert!(!email::valid_address(bad), "{bad:?}");
    }
}

/// Method, URL, headers and body of one outgoing request.
type Seen = (String, String, Vec<(String, String)>, Vec<u8>);
/// Answers Brevo's two endpoints and records each request.
struct FakeBrevo {
    senders: (u16, &'static str),
    send: io::Result<u16>,
    seen: Mutex<Vec<Seen>>,
}
impl FakeBrevo {
    fn new(senders: (u16, &'static str), send: io::Result<u16>) -> Arc<Self> {
        Arc::new(Self {
            senders,
            send,
            seen: Mutex::default(),
        })
    }
}
impl Fetch for FakeBrevo {
    fn get(&self, _: &str, _: &str, _: usize) -> io::Result<Fetched> {
        unreachable!("Brevo uses send")
    }
    fn send(&self, r: &Outgoing<'_>) -> io::Result<Fetched> {
        let headers = r
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.seen
            .lock()
            .unwrap()
            .push((r.method.into(), r.url.into(), headers, r.body.into()));
        let (status, body) = if r.url.ends_with("/senders") {
            (self.senders.0, self.senders.1.as_bytes().to_vec())
        } else {
            match &self.send {
                Ok(status) => (*status, br#"{"messageId":"x"}"#.to_vec()),
                Err(e) => return Err(io::Error::new(e.kind(), e.to_string())),
            }
        };
        Ok(Fetched {
            status,
            content_type: "application/json".into(),
            body,
        })
    }
}

const SENDERS: &str = r#"{"senders":[{"id":1,"name":"Old","email":"old@example.com","active":false},
    {"id":2,"name":"Bank","email":"bank@example.com","active":true},
    {"id":3,"name":"Other","email":"other@example.com","active":true}]}"#;
fn message() -> Message {
    Message {
        to: "alice@example.com".into(),
        subject: "NanaCoin: Hi".into(),
        text: "Hello \"there\"\n".into(),
    }
}

#[test]
fn brevo_uses_the_first_active_sender_and_posts_plain_text() {
    let fetch = FakeBrevo::new((200, SENDERS), Ok(201));
    let mut brevo = Brevo::new(fetch.clone(), "xkeysib-test".into(), String::new());
    assert_eq!(brevo.send(&message()), Ok(()));
    let seen = fetch.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        (seen[0].0.as_str(), seen[0].1.as_str()),
        ("GET", "https://api.brevo.com/v3/senders")
    );
    assert_eq!(
        (seen[1].0.as_str(), seen[1].1.as_str()),
        ("POST", "https://api.brevo.com/v3/smtp/email")
    );
    for (_, _, headers, _) in seen.iter() {
        assert!(headers.contains(&("api-key".into(), "xkeysib-test".into())));
    }
    let body: Value = serde_json::from_slice(&seen[1].3).unwrap();
    assert_eq!(
        body,
        json!({"sender":{"name":"NanaCoin","email":"bank@example.com"},
        "to":[{"email":"alice@example.com"}],"subject":"NanaCoin: Hi","textContent":"Hello \"there\"\n"})
    );
}

#[test]
fn brevo_matches_a_configured_sender_case_insensitively() {
    let fetch = FakeBrevo::new((200, SENDERS), Ok(201));
    let mut brevo = Brevo::new(fetch.clone(), "k".into(), "OTHER@example.com".into());
    assert_eq!(brevo.send(&message()), Ok(()));
    let body: Value = serde_json::from_slice(&fetch.seen.lock().unwrap()[1].3).unwrap();
    assert_eq!(body["sender"]["email"], "other@example.com");

    let fetch = FakeBrevo::new((200, SENDERS), Ok(201));
    let mut brevo = Brevo::new(fetch.clone(), "k".into(), "old@example.com".into());
    assert_eq!(
        brevo.send(&message()),
        Err("No matching verified email sender".into())
    );
    assert_eq!(fetch.seen.lock().unwrap().len(), 1, "nothing posted");
}

#[test]
fn brevo_reports_provider_failures() {
    let cases: [(FakeBrevo, &str); 4] = [
        (
            FakeBrevo {
                senders: (401, "{}"),
                send: Ok(201),
                seen: Mutex::default(),
            },
            "Sender lookup HTTP 401",
        ),
        (
            FakeBrevo {
                senders: (200, "nonsense"),
                send: Ok(201),
                seen: Mutex::default(),
            },
            "Invalid sender response",
        ),
        (
            FakeBrevo {
                senders: (200, SENDERS),
                send: Ok(400),
                seen: Mutex::default(),
            },
            "Email provider HTTP 400",
        ),
        (
            FakeBrevo {
                senders: (200, SENDERS),
                send: Err(io::ErrorKind::TimedOut.into()),
                seen: Mutex::default(),
            },
            "Delivery unknown: provider connection interrupted",
        ),
    ];
    for (fetch, expected) in cases {
        let mut brevo = Brevo::new(Arc::new(fetch), "k".into(), String::new());
        assert_eq!(brevo.send(&message()), Err(expected.to_string()));
    }
}
