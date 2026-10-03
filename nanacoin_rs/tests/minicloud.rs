mod common;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use nanacoin::{api, domain::*, journal::*};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[derive(Default)]
struct Memory(Vec<[u8; FRAME_SIZE]>);
impl Journal for Memory {
    fn read(&mut self, i: usize, out: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(frame) = self.0.get(i) {
            *out = *frame;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, _: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        self.0.push(*frame);
        Ok(())
    }
}
fn get(s: &mut Service<Memory>, auth: &str) -> (u16, Value) {
    let mut output = vec![0; api::RESPONSE_LIMIT];
    let (status, len) = api::handle(
        s,
        "GET",
        "/api/v1/minicloud/session",
        auth,
        &[],
        &mut output,
    );
    (status, serde_json::from_slice(&output[..len]).unwrap())
}
#[test]
fn authenticated_bank_session_issues_minicloud_wire_token_and_never_upgrades_api_keys() {
    // This integration test runs in its own process; restore caller configuration.
    struct Env(Vec<(&'static str, Option<String>)>);
    impl Drop for Env {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                if let Some(v) = value {
                    std::env::set_var(key, v);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
    let settings = [
        ("NANACOIN_MINICLOUD_URL", "https://cloud.local"),
        ("NANACOIN_MINICLOUD_BANK", "bank"),
        (
            "NANACOIN_MINICLOUD_SECRET",
            "test-bank-secret-at-least-32-bytes-long",
        ),
    ];
    let _restore = Env(settings
        .iter()
        .map(|(k, _)| (*k, std::env::var(k).ok()))
        .collect());
    for (key, value) in settings {
        std::env::set_var(key, value);
    }
    let mut s = Service::open(Memory::default()).unwrap();
    common::provision(&mut s);
    assert_eq!(get(&mut s, "").0, 401);
    let auth = common::login(&mut s);
    let (status, response) = get(&mut s, &auth);
    assert_eq!(status, 200, "{response}");
    assert_eq!(response["owner"], "nana@bank");
    assert!(!response.to_string().contains(settings[2].1));
    let token = response["token"].as_str().unwrap();
    let (message, signature) = token.rsplit_once('.').unwrap();
    let mut decoded = [0; 512];
    let n = URL_SAFE_NO_PAD
        .decode_slice(message.strip_prefix("mc1.").unwrap(), &mut decoded)
        .unwrap();
    let claims: Value = serde_json::from_slice(&decoded[..n]).unwrap();
    assert_eq!(claims["bank"], "bank");
    assert_eq!(claims["username"], "nana");
    assert_eq!(claims["aud"], "https://cloud.local");
    assert_eq!(
        claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(),
        300
    );
    let n = URL_SAFE_NO_PAD
        .decode_slice(signature, &mut decoded)
        .unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(settings[2].1.as_bytes()).unwrap();
    mac.update(message.as_bytes());
    mac.verify_slice(&decoded[..n]).unwrap();
    let namespace = format!("{:x}", Sha256::digest(b"nana@bank"));
    assert_eq!(response["namespace"], namespace[..16]);
    for scope in ["full", "read"] {
        let (status, key) = common::call(
            &mut s,
            "/api/v1/me/api-key",
            &auth,
            json!({"password":"1234","scope":scope}),
        );
        assert_eq!(status, 200, "{key}");
        assert_eq!(
            get(
                &mut s,
                &format!("Bearer {}", key["api_key"].as_str().unwrap())
            )
            .0,
            403
        );
    }
}
