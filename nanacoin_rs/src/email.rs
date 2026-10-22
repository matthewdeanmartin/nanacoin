//! Explicit household mail. No financial event automatically sends email.
//! Providers own transport; the worker never holds the bank lock during I/O.
use crate::{
    api, auth,
    domain::{Command, Error, MemberId},
    journal::{Journal, Service},
};
use miniframework::{
    fetch::{Fetch, Outgoing},
    kv::Kv,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use subtle::ConstantTimeEq;

pub const DAILY_LIMIT: u32 = 100;
/// Unauthenticated recovery requests share this part of the daily allowance,
/// so strangers cannot exhaust the allowance members use for each other.
pub const RECOVERY_DAILY_LIMIT: u32 = 20;
/// Public recovery requests are throttled per target account, not globally.
pub const RECOVERY_COOLDOWN: u64 = 300;
pub const TTL: u64 = 900;
pub const QUEUE: usize = 16;
/// TLS handshakes need far more than a default small thread: a 16 KiB desktop
/// thread overflowed (aborting the process) in debug builds. Boards keep this
/// worker small in scarce internal RAM (it writes NVS, so its stack cannot be
/// in PSRAM); their provider runs each send on a PSRAM-stack helper instead.
const WORKER_STACK: usize = if cfg!(target_os = "espidf") {
    8 * 1024
} else {
    256 * 1024
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub to: String,
    pub subject: String,
    pub text: String,
}

/// SMTP can implement this interface without changing account or UI behavior.
pub trait Provider: Send + 'static {
    fn name(&self) -> &'static str;
    fn send(&mut self, message: &Message) -> Result<(), String>;
}

pub struct Brevo {
    fetch: Arc<dyn Fetch>,
    key: String,
    sender: String,
}
impl Brevo {
    pub fn new(fetch: Arc<dyn Fetch>, key: String, sender: String) -> Self {
        Self { fetch, key, sender }
    }
    pub fn configured(fetch: Arc<dyn Fetch>) -> Option<Box<dyn Provider>> {
        let key = std::env::var("NANACOIN_BREVO_API_KEY")
            .ok()
            .or_else(|| option_env!("NANACOIN_BREVO_API_KEY").map(str::to_owned))?;
        if key.trim().is_empty() || key.contains(char::is_whitespace) {
            return None;
        }
        let sender = std::env::var("NANACOIN_MAIL_FROM")
            .ok()
            .or_else(|| option_env!("NANACOIN_MAIL_FROM").map(str::to_owned))
            .unwrap_or_default();
        Some(Box::new(Self::new(fetch, key, sender)))
    }
}
impl Provider for Brevo {
    fn name(&self) -> &'static str {
        "Brevo"
    }
    fn send(&mut self, message: &Message) -> Result<(), String> {
        // Resolve the first verified sender like GameMatcher, or check the configured sender.
        let response = self
            .fetch
            .send(&Outgoing {
                method: "GET",
                url: "https://api.brevo.com/v3/senders",
                headers: &[("api-key", &self.key)],
                body: &[],
                limit: 16384,
            })
            .map_err(|_| "Could not reach email provider".to_string())?;
        if response.status != 200 {
            return Err(format!("Sender lookup HTTP {}", response.status));
        }
        #[derive(Deserialize)]
        struct Sender {
            email: String,
            active: bool,
        }
        #[derive(Deserialize)]
        struct Senders {
            senders: Vec<Sender>,
        }
        let senders: Senders =
            serde_json::from_slice(&response.body).map_err(|_| "Invalid sender response")?;
        let sender = senders
            .senders
            .iter()
            .find(|s| {
                s.active && (self.sender.is_empty() || s.email.eq_ignore_ascii_case(&self.sender))
            })
            .ok_or("No matching verified email sender")?;
        let body = serde_json::to_vec(
            &serde_json::json!({"sender":{"name":"NanaCoin", "email":sender.email},
            "to":[{"email":message.to}], "subject":message.subject, "textContent":message.text}),
        )
        .map_err(|_| "Email encoding failed")?;
        let response = self
            .fetch
            .send(&Outgoing {
                method: "POST",
                url: "https://api.brevo.com/v3/smtp/email",
                headers: &[("api-key", &self.key), ("Content-Type", "application/json")],
                body: &body,
                limit: 2048,
            })
            .map_err(|_| "Delivery unknown: provider connection interrupted".to_string())?;
        if response.status == 201 {
            Ok(())
        } else {
            Err(format!("Email provider HTTP {}", response.status))
        }
    }
}

pub fn valid_address(a: &str) -> bool {
    let Some((user, host)) = a.split_once('@') else {
        return false;
    };
    a.len() <= 254
        && !user.is_empty()
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains('@')
        && a.bytes()
            .all(|c| c.is_ascii_graphic() && !b",<>\";\\".contains(&c))
}

#[derive(Clone, Serialize, Deserialize)]
struct Contact {
    member: u8,
    username: String,
    created_at: u64,
    email: String,
    notifications: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Job {
    message: Message,
    expires: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Saved {
    contacts: Vec<Contact>,
    jobs: Vec<Job>,
    day: u64,
    count: u32,
    recoveries: u32,
}
struct Challenge {
    member: MemberId,
    address: String,
    hash: [u8; 32],
    expires: u64,
    recovery: bool,
    password: [u8; 32],
}
struct Inner {
    saved: Saved,
    challenges: Vec<Challenge>,
    last_request: Vec<(u8, u64)>,
    last_recovery: Vec<(u8, u64)>,
    error: String,
}

pub struct Mailer {
    inner: Mutex<Inner>,
    kv: Arc<dyn Kv>,
    provider: &'static str,
    origin: String,
}
impl Mailer {
    pub fn open(kv: Arc<dyn Kv>, provider: &'static str, origin: String) -> Result<Self, Error> {
        let saved = match kv.get("email") {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| Error::Storage)?,
            None => Saved::default(),
        };
        Ok(Self {
            inner: Mutex::new(Inner {
                saved,
                challenges: vec![],
                last_request: vec![],
                last_recovery: vec![],
                error: String::new(),
            }),
            kv,
            provider,
            origin,
        })
    }
    fn save(&self, inner: &mut Inner, next: Saved) -> Result<(), Error> {
        let bytes = serde_json::to_vec(&next).map_err(|_| Error::Storage)?;
        self.kv.set("email", &bytes).map_err(|_| Error::Storage)?;
        inner.saved = next;
        Ok(())
    }
    fn enqueue(
        &self,
        inner: &mut Inner,
        message: Message,
        now: u64,
        recovery: bool,
    ) -> Result<(), Error> {
        if now < 1_700_000_000 {
            return Err(Error::InvalidInput);
        }
        let mut next = inner.saved.clone();
        next.jobs.retain(|j| j.expires > now);
        if now / 86400 > next.day {
            next.day = now / 86400;
            next.count = 0;
            next.recoveries = 0;
        }
        if next.count >= DAILY_LIMIT || (recovery && next.recoveries >= RECOVERY_DAILY_LIMIT) {
            return Err(Error::RateLimited);
        }
        if next.jobs.len() >= QUEUE {
            return Err(Error::Capacity);
        }
        next.jobs.push(Job {
            message,
            expires: now + TTL,
        });
        next.count += 1;
        if recovery {
            next.recoveries += 1;
        }
        self.save(inner, next)
    }
    pub fn queued(&self) -> usize {
        self.inner.lock().unwrap().saved.jobs.len()
    }
    /// Claim durably before network I/O. Ambiguous sends are never automatically
    /// retried: duplicate notifications are worse than asking the user to resend.
    pub fn run_once(&self, provider: &mut dyn Provider, now: u64) {
        let job = {
            let mut inner = self.inner.lock().unwrap();
            let mut next = inner.saved.clone();
            next.jobs.retain(|j| j.expires > now);
            if next.jobs.is_empty() {
                return;
            }
            let job = next.jobs.remove(0);
            if self.save(&mut inner, next).is_err() {
                inner.error = "Email storage failed".into();
                return;
            }
            job
        };
        let result = provider.send(&job.message);
        self.inner.lock().unwrap().error = result.err().unwrap_or_default();
    }
    pub fn start(
        kv: Arc<dyn Kv>,
        mut provider: Box<dyn Provider>,
        origin: String,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mail = Arc::new(
            Self::open(kv, provider.name(), origin)
                .map_err(|_| "Email storage could not be read")?,
        );
        let worker = mail.clone();
        std::thread::Builder::new()
            .name("nanacoin-mail".into())
            .stack_size(WORKER_STACK)
            .spawn(move || loop {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                worker.run_once(provider.as_mut(), now);
                std::thread::sleep(std::time::Duration::from_secs(1));
            })?;
        Ok(mail)
    }

    pub fn handle<J: Journal>(
        &self,
        service: &mut Service<J>,
        method: &str,
        path: &str,
        authorization: &str,
        body: &[u8],
        out: &mut [u8],
    ) -> (u16, usize) {
        let result = self.route(service, method, path, authorization, body, out);
        match result {
            Ok(n) => (200, n),
            Err(e) => api::error_response(e, out),
        }
    }
    fn route<J: Journal>(
        &self,
        service: &mut Service<J>,
        method: &str,
        path: &str,
        authorization: &str,
        body: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Error> {
        if service.storage_failed() {
            return Err(Error::Storage);
        }
        // Mail lifetimes use wall-clock time; sessions and password throttling use the auth clock.
        let now = service.now();
        let auth_now = service.auth.now();
        let public = matches!(path, "/api/v1/email/recover" | "/api/v1/email/reset");
        let actor = if public {
            MemberId(0)
        } else {
            let token = authorization
                .strip_prefix("Bearer ")
                .ok_or(Error::Unauthorized)?;
            // Recovery settings and email are human-session capabilities, never API keys.
            service.auth.lookup(service.state(), token, auth_now)?
        };
        let mut inner = self.inner.lock().unwrap();
        inner.challenges.retain(|c| c.expires > now);
        if method == "GET" && path == "/api/v1/email" {
            let member = service.state().member(actor)?;
            let contact = inner.saved.contacts.iter().find(|c| {
                c.member == actor.0
                    && c.username == member.username.as_str()
                    && c.created_at == member.created_at
            });
            #[derive(Serialize)]
            struct View<'a> {
                provider: &'a str,
                address: &'a str,
                notifications: bool,
                queued: usize,
                last_error: &'a str,
            }
            return api::serialize(
                &View {
                    provider: self.provider,
                    address: contact.map_or("", |c| &c.email),
                    notifications: contact.is_some_and(|c| c.notifications),
                    queued: inner.saved.jobs.len(),
                    last_error: if member.role == crate::domain::Role::Nana {
                        &inner.error
                    } else {
                        ""
                    },
                },
                out,
            );
        }
        if method != "POST" {
            return Err(Error::NotFound);
        }
        if !matches!(
            path,
            "/api/v1/email/verify"
                | "/api/v1/email/reset"
                | "/api/v1/email/preferences"
                | "/api/v1/email/recover"
        ) {
            inner
                .last_request
                .retain(|(_, t)| now < t.saturating_add(60));
            if inner.last_request.iter().any(|(id, _)| *id == actor.0) {
                return Err(Error::RateLimited);
            }
            inner.last_request.push((actor.0, now));
        }
        match path {
            "/api/v1/email/address" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    address: heapless::String<254>,
                    password: heapless::String<128>,
                }
                let input: Input = api::parse(body)?;
                let member = service.state().member(actor)?.clone();
                service
                    .auth
                    .confirm_password(&service.state, actor, &input.password, auth_now)?;
                let address = input.address.trim().to_lowercase();
                if !valid_address(&address) {
                    return Err(Error::InvalidInput);
                }
                self.challenge(&mut inner, &member, address, false, now)?;
            }
            "/api/v1/email/verify" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    code: heapless::String<64>,
                }
                let input: Input = api::parse(body)?;
                let index = challenge_index(&inner, &input.code, false, now)?;
                if inner.challenges[index].member != actor {
                    return Err(Error::Unauthorized);
                }
                let member = service.state().member(actor)?;
                if inner.challenges[index].password != password_fingerprint(member) {
                    return Err(Error::Unauthorized);
                }
                let mut next = inner.saved.clone();
                next.contacts.retain(|c| c.member != actor.0);
                next.contacts.push(Contact {
                    member: actor.0,
                    username: member.username.to_string(),
                    created_at: member.created_at,
                    email: inner.challenges[index].address.clone(),
                    notifications: true,
                });
                self.save(&mut inner, next)?;
                inner.challenges.remove(index);
            }
            "/api/v1/email/preferences" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    notifications: bool,
                    #[serde(default)]
                    remove: bool,
                }
                let input: Input = api::parse(body)?;
                let mut next = inner.saved.clone();
                if input.remove || !input.notifications {
                    if let Some(contact) = next.contacts.iter().find(|c| c.member == actor.0) {
                        let address = contact.email.clone();
                        next.jobs.retain(|j| j.message.to != address);
                    }
                }
                if input.remove {
                    next.contacts.retain(|c| c.member != actor.0);
                    inner.challenges.retain(|c| c.member != actor);
                } else if let Some(c) = next.contacts.iter_mut().find(|c| c.member == actor.0) {
                    c.notifications = input.notifications;
                }
                self.save(&mut inner, next)?;
            }
            "/api/v1/email/recover" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    username: heapless::String<32>,
                }
                let input: Input = api::parse(body)?;
                if let Some(member) = service.state().members.iter().find(|m| {
                    m.username.eq_ignore_ascii_case(input.username.trim())
                        && !m.disabled
                        && m.password.is_some()
                }) {
                    inner
                        .last_recovery
                        .retain(|(_, t)| now < t.saturating_add(RECOVERY_COOLDOWN));
                    let cooling = inner.last_recovery.iter().any(|(id, _)| *id == member.id.0);
                    if let Some(c) = inner
                        .saved
                        .contacts
                        .iter()
                        .find(|c| {
                            c.member == member.id.0
                                && c.username == member.username.as_str()
                                && c.created_at == member.created_at
                        })
                        .cloned()
                        .filter(|_| !cooling)
                    {
                        // Same public response for absent, unverified, cooling, limited, full queue, and accepted accounts.
                        if self
                            .challenge(&mut inner, member, c.email, true, now)
                            .is_ok()
                        {
                            inner.last_recovery.push((member.id.0, now));
                        }
                    }
                }
            }
            "/api/v1/email/reset" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    code: heapless::String<64>,
                    password: heapless::String<128>,
                }
                let input: Input = api::parse(body)?;
                let index = challenge_index(&inner, &input.code, true, now)?;
                let challenge = &inner.challenges[index];
                let member = service.state().member(challenge.member)?;
                if member.disabled
                    || challenge.password != password_fingerprint(member)
                    || !inner.saved.contacts.iter().any(|c| {
                        c.member == member.id.0
                            && c.username == member.username.as_str()
                            && c.created_at == member.created_at
                            && c.email == challenge.address
                    })
                {
                    return Err(Error::Unauthorized);
                }
                let id = member.id;
                let nonce = member.last_request + 1;
                let password = auth::PasswordVerifier::hash(&input.password)?;
                service.execute(
                    id,
                    nonce,
                    Command::UpdateMember {
                        member: id,
                        display_name: None,
                        password: Some(password),
                        role: None,
                        disabled: None,
                        mastodon_id: None,
                        bio: None,
                    },
                )?;
                inner.challenges.retain(|c| c.member != id);
            }
            "/api/v1/email/invite" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    address: heapless::String<254>,
                }
                let input: Input = api::parse(body)?;
                let address = input.address.trim().to_lowercase();
                if !valid_address(&address) {
                    return Err(Error::InvalidInput);
                }
                let member = service.state().member(actor)?;
                self.enqueue(&mut inner, Message { to:address, subject:format!("{} invited you to NanaCoin",member.name),
                    text:format!("{} invited you to the household bank.\n\nVisit {} on the household network. Ask Nana to create your account if you do not have one.\n\nThis invitation does not create an account or grant access.",member.name,self.origin) },now,false)?;
            }
            "/api/v1/email/notify" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Input {
                    member: u8,
                    subject: heapless::String<80>,
                    text: heapless::String<512>,
                }
                let input: Input = api::parse(body)?;
                if input.subject.trim().is_empty()
                    || input.subject.chars().any(char::is_control)
                    || input.text.trim().is_empty()
                {
                    return Err(Error::InvalidInput);
                }
                let target = service.state().member(MemberId(input.member))?;
                if target.disabled {
                    return Err(Error::NotFound);
                }
                let contact = inner
                    .saved
                    .contacts
                    .iter()
                    .find(|c| {
                        c.member == input.member
                            && c.username == target.username.as_str()
                            && c.created_at == target.created_at
                            && c.notifications
                    })
                    .ok_or(Error::NotFound)?;
                let message = Message { to:contact.email.clone(), subject:format!("NanaCoin: {}",input.subject),
                    text:format!("From {} on NanaCoin:\n\n{}\n\n{}\nTurn off member emails on the Email page.",service.state().member(actor)?.name,input.text,self.origin) };
                self.enqueue(&mut inner, message, now, false)?;
            }
            _ => return Err(Error::NotFound),
        }
        api::serialize(&true, out)
    }
    fn challenge(
        &self,
        inner: &mut Inner,
        member: &crate::domain::Member,
        address: String,
        recovery: bool,
        now: u64,
    ) -> Result<(), Error> {
        let mut random = [0u8; 16];
        getrandom::getrandom(&mut random).map_err(|_| Error::Capacity)?;
        let code: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let action = if recovery {
            "reset your password"
        } else {
            "verify your email address"
        };
        self.enqueue(inner, Message { to:address.clone(), subject:format!("NanaCoin: {action}"),
            text:format!("Enter this code on the NanaCoin Email page to {action}:\n\n{code}\n\nIt expires in 15 minutes.\n{}\nIf you did not ask for this, ignore this email.",self.origin) },now,recovery)?;
        inner
            .challenges
            .retain(|c| c.member != member.id || c.recovery != recovery);
        inner.challenges.push(Challenge {
            member: member.id,
            address,
            hash: auth::digest(&code),
            expires: now + TTL,
            recovery,
            password: password_fingerprint(member),
        });
        Ok(())
    }
}
fn password_fingerprint(member: &crate::domain::Member) -> [u8; 32] {
    auth::digest(&serde_json::to_string(&member.password).expect("password serializes"))
}
fn challenge_index(inner: &Inner, code: &str, recovery: bool, now: u64) -> Result<usize, Error> {
    let hash = auth::digest(code.trim());
    inner
        .challenges
        .iter()
        .position(|c| c.recovery == recovery && c.expires > now && bool::from(c.hash.ct_eq(&hash)))
        .ok_or(Error::Unauthorized)
}
