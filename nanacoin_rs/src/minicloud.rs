//! Instance trust: bank credentials stay server-side; users receive five-minute tokens.
use crate::domain::Error;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct Claims<'a> {
    bank: &'a str,
    username: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}
#[derive(Serialize)]
pub struct Session {
    pub url: String,
    pub token: String,
    pub owner: String,
    pub namespace: String,
    pub expires_at: u64,
}
fn setting(key: &str, compiled: Option<&str>) -> Option<String> {
    std::env::var(key)
        .ok()
        .or_else(|| compiled.map(str::to_owned))
}
fn origin_valid(s: &str) -> bool {
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .is_some_and(|host| {
            !host.is_empty()
                && host.len() <= 160
                && host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b))
        })
}
pub fn session(username: &str, now: u64) -> Result<Session, Error> {
    let url = setting(
        "NANACOIN_MINICLOUD_URL",
        option_env!("NANACOIN_MINICLOUD_URL"),
    )
    .ok_or(Error::NotFound)?;
    let bank = setting(
        "NANACOIN_MINICLOUD_BANK",
        option_env!("NANACOIN_MINICLOUD_BANK"),
    )
    .unwrap_or_else(|| crate::board::FQDN.into());
    let secret = setting(
        "NANACOIN_MINICLOUD_SECRET",
        option_env!("NANACOIN_MINICLOUD_SECRET"),
    )
    .ok_or(Error::NotFound)?;
    if !origin_valid(&url)
        || bank.is_empty()
        || bank.len() > 64
        || !bank
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || secret.len() < 32
        || secret.len() > 128
        || username.trim().is_empty()
        || username.contains('@')
        || username.len() > 64
        || username.chars().any(char::is_control)
        || now < 1_700_000_000
    {
        return Err(Error::Forbidden);
    }
    let mut json = [0; 512];
    let len = serde_json_core::to_slice(
        &Claims {
            bank: &bank,
            username,
            aud: &url,
            iat: now,
            exp: now + 300,
        },
        &mut json,
    )
    .map_err(|_| Error::Capacity)?;
    let mut encoded = [0; 768];
    let len = URL_SAFE_NO_PAD
        .encode_slice(&json[..len], &mut encoded)
        .map_err(|_| Error::Capacity)?;
    let payload = std::str::from_utf8(&encoded[..len]).map_err(|_| Error::Capacity)?;
    let message = format!("mc1.{payload}");
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| Error::Forbidden)?;
    mac.update(message.as_bytes());
    let mut signature = [0; 64];
    let len = URL_SAFE_NO_PAD
        .encode_slice(mac.finalize().into_bytes(), &mut signature)
        .map_err(|_| Error::Capacity)?;
    let token = format!(
        "{message}.{}",
        std::str::from_utf8(&signature[..len]).map_err(|_| Error::Capacity)?
    );
    let owner = format!("{username}@{bank}");
    let namespace = format!("{:x}", Sha256::digest(owner.as_bytes()))[..16].to_owned();
    Ok(Session {
        url,
        token,
        owner,
        namespace,
        expires_at: now + 300,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origins_are_bounded_and_cannot_contain_paths_or_credentials() {
        assert!(origin_valid("https://minicloud.local:443"));
        for bad in [
            "https://",
            "https://evil@host",
            "https://host/path",
            "https://host\r\nX: y",
        ] {
            assert!(!origin_valid(bad));
        }
    }
}
