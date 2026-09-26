//! The existing Nanacoin security model: salted passwords, S256 PKCE and
//! expiring RAM-only sessions. No permanent access-token account credentials.
use crate::domain::{Error, MemberId, State};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use heapless::String;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

// Preserve the existing Go application's work factor and household PIN rule.
pub const PASSWORD_ROUNDS: u32 = 1000;
pub const SESSION_TTL: u64 = 8 * 60 * 60;
pub const CODE_TTL: u64 = 60;
pub const LOCKOUT: u64 = 5 * 60;
pub const MAX_SESSIONS: usize = 64;
pub const MAX_CODES: usize = 16;
pub type Secret = String<43>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordVerifier {
    salt: [u8; 16],
    key: [u8; 32],
    rounds: u32,
}

impl PasswordVerifier {
    pub fn hash(password: &str) -> Result<Self, Error> {
        if !(4..=128).contains(&password.len()) {
            return Err(Error::InvalidInput);
        }
        let mut salt = [0; 16];
        getrandom::getrandom(&mut salt).map_err(|_| Error::Unavailable)?;
        Ok(Self::derive(password, salt, PASSWORD_ROUNDS))
    }
    fn derive(password: &str, salt: [u8; 16], rounds: u32) -> Self {
        let mut key = [0; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, rounds, &mut key);
        Self { salt, key, rounds }
    }
    pub fn verify(&self, password: &str) -> bool {
        self.valid()
            && bool::from(
                self.key
                    .ct_eq(&Self::derive(password, self.salt, self.rounds).key),
            )
    }
    pub fn valid(&self) -> bool {
        self.rounds == PASSWORD_ROUNDS
    }
}

#[derive(Clone, Copy)]
struct Session {
    hash: [u8; 32],
    member: MemberId,
    expires: u64,
}
#[derive(Clone, Copy)]
struct Code {
    hash: [u8; 32],
    challenge: [u8; 32],
    redirect: [u8; 32],
    member: MemberId,
    expires: u64,
}
#[derive(Clone, Copy)]
struct Failure {
    username: [u8; 32],
    count: u8,
    until: u64,
}

pub struct Auth {
    started: std::time::Instant,
    sessions: [Option<Session>; MAX_SESSIONS],
    codes: [Option<Code>; MAX_CODES],
    failures: [Option<Failure>; 64],
}
impl Default for Auth {
    fn default() -> Self {
        Self {
            started: std::time::Instant::now(),
            sessions: [None; MAX_SESSIONS],
            codes: [None; MAX_CODES],
            failures: [None; 64],
        }
    }
}

impl Auth {
    /// Counts only; never expose credential hashes or identities in diagnostics.
    pub(crate) fn inventory(&self) -> [(usize, usize, usize, usize); 3] {
        let now = self.now();
        [
            (
                self.sessions.iter().flatten().count(),
                self.sessions
                    .iter()
                    .flatten()
                    .filter(|s| s.expires > now)
                    .count(),
                MAX_SESSIONS,
                core::mem::size_of_val(&self.sessions),
            ),
            (
                self.codes.iter().flatten().count(),
                self.codes
                    .iter()
                    .flatten()
                    .filter(|s| s.expires > now)
                    .count(),
                MAX_CODES,
                core::mem::size_of_val(&self.codes),
            ),
            (
                self.failures.iter().flatten().count(),
                self.failures
                    .iter()
                    .flatten()
                    .filter(|s| s.until > now)
                    .count(),
                64,
                core::mem::size_of_val(&self.failures),
            ),
        ]
    }

    pub(crate) fn clear(&mut self) {
        self.sessions.fill(None);
        self.codes.fill(None);
        self.failures.fill(None);
    }
}

pub fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
pub fn challenge(verifier: &str) -> Result<Secret, Error> {
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._~".contains(&c))
    {
        return Err(Error::InvalidInput);
    }
    encode(&digest(verifier))
}
fn encode(bytes: &[u8; 32]) -> Result<Secret, Error> {
    let mut encoded = [0; 43];
    URL_SAFE_NO_PAD
        .encode_slice(bytes, &mut encoded)
        .map_err(|_| Error::Unavailable)?;
    Secret::try_from(core::str::from_utf8(&encoded).map_err(|_| Error::Unavailable)?)
        .map_err(|_| Error::Unavailable)
}
fn random_secret() -> Result<Secret, Error> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| Error::Unavailable)?;
    encode(&bytes)
}

impl Auth {
    pub fn now(&self) -> u64 {
        self.started.elapsed().as_secs()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn authorize(
        &mut self,
        state: &State,
        username: &str,
        password: &str,
        pkce: &str,
        method: &str,
        redirect: &str,
        now: u64,
    ) -> Result<Secret, Error> {
        if method != "S256"
            || pkce.len() != 43
            || !pkce
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            || redirect.is_empty()
            || redirect.len() > 256
        {
            return Err(Error::InvalidInput);
        }
        let username_hash = digest(username);
        if self
            .failures
            .iter()
            .flatten()
            .any(|f| f.username == username_hash && f.count >= 5 && now < f.until)
        {
            return Err(Error::RateLimited);
        }
        let user = state
            .members
            .iter()
            .find(|m| m.username.as_str() == username && m.password.is_some());
        let valid = if let Some(member) = user {
            member.password.as_ref().unwrap().verify(password)
        } else {
            // Unknown usernames pay the same KDF cost and get the same error.
            let dummy = PasswordVerifier::derive(password, [0; 16], PASSWORD_ROUNDS);
            std::hint::black_box(dummy);
            false
        };
        if !valid {
            self.record_failure(username_hash, now);
            return Err(Error::InvalidCredentials);
        }
        let member = user.unwrap();
        if member.disabled {
            return Err(Error::Disabled);
        }
        for failure in &mut self.failures {
            if failure.is_some_and(|f| f.username == username_hash) {
                *failure = None;
            }
        }
        let slot = self
            .codes
            .iter_mut()
            .find(|c| c.is_none_or(|c| now >= c.expires))
            .ok_or(Error::Capacity)?;
        let secret = random_secret()?;
        *slot = Some(Code {
            hash: digest(&secret),
            challenge: digest(pkce),
            redirect: digest(redirect),
            member: member.id,
            expires: now.saturating_add(CODE_TTL),
        });
        Ok(secret)
    }

    fn record_failure(&mut self, username: [u8; 32], now: u64) {
        let matching = self
            .failures
            .iter()
            .position(|f| f.is_some_and(|f| f.username == username));
        let index = matching.unwrap_or_else(|| {
            self.failures
                .iter()
                .enumerate()
                .min_by_key(|(_, f)| f.map_or(0, |f| f.until))
                .unwrap()
                .0
        });
        let count = self.failures[index]
            .filter(|f| f.username == username && now < f.until)
            .map_or(1, |f| f.count.saturating_add(1).min(5));
        self.failures[index] = Some(Failure {
            username,
            count,
            until: now.saturating_add(LOCKOUT),
        });
    }

    pub fn redeem(
        &mut self,
        state: &State,
        code: &str,
        verifier: &str,
        redirect: &str,
        now: u64,
    ) -> Result<(Secret, MemberId), Error> {
        let hash = digest(code);
        let index = self
            .codes
            .iter()
            .position(|c| c.is_some_and(|c| bool::from(c.hash.ct_eq(&hash))))
            .ok_or(Error::Unauthorized)?;
        // Consume before verification, including failed PKCE/redirect attempts.
        let code = self.codes[index].take().unwrap();
        let computed = challenge(verifier).map_err(|_| Error::Unauthorized)?;
        if now >= code.expires
            || !bool::from(code.challenge.ct_eq(&digest(&computed)))
            || !bool::from(code.redirect.ct_eq(&digest(redirect)))
        {
            return Err(Error::Unauthorized);
        }
        let member = state.member(code.member)?;
        if member.disabled || member.password.is_none() {
            return Err(Error::Unauthorized);
        }
        let slot = self
            .sessions
            .iter_mut()
            .find(|s| s.is_none_or(|s| now >= s.expires))
            .ok_or(Error::Capacity)?;
        let secret = random_secret()?;
        *slot = Some(Session {
            hash: digest(&secret),
            member: code.member,
            expires: now.saturating_add(SESSION_TTL),
        });
        Ok((secret, code.member))
    }

    pub fn lookup(&self, state: &State, token: &str, now: u64) -> Result<MemberId, Error> {
        let hash = digest(token);
        let session = self
            .sessions
            .iter()
            .flatten()
            .find(|s| bool::from(s.hash.ct_eq(&hash)) && now < s.expires)
            .ok_or(Error::Unauthorized)?;
        let member = state.member(session.member)?;
        if member.disabled || member.password.is_none() {
            return Err(Error::Unauthorized);
        }
        Ok(member.id)
    }
    pub fn revoke(&mut self, token: &str) {
        let hash = digest(token);
        for session in &mut self.sessions {
            if session.is_some_and(|s| bool::from(s.hash.ct_eq(&hash))) {
                *session = None;
            }
        }
    }
    pub fn revoke_member(&mut self, member: MemberId) {
        for session in &mut self.sessions {
            if session.is_some_and(|s| s.member == member) {
                *session = None;
            }
        }
        for code in &mut self.codes {
            if code.is_some_and(|c| c.member == member) {
                *code = None;
            }
        }
    }
}
