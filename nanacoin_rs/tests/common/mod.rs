#![allow(dead_code)]
use nanacoin::{api, auth::PasswordVerifier, domain::*, journal::*};

pub fn provision<J: Journal>(s: &mut Service<J>) {
    s.execute(
        MemberId(1),
        1,
        Command::Provision {
            household_name: Name::try_from("Our house").unwrap(),
            username: Name::try_from("nana").unwrap(),
            display_name: Name::try_from("Nana").unwrap(),
            password: PasswordVerifier::hash("1234").unwrap(),
        },
    )
    .unwrap();
}
pub fn call<J: Journal>(
    s: &mut Service<J>,
    path: &str,
    auth: &str,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    let mut output = vec![0; api::RESPONSE_LIMIT];
    let body = serde_json::to_vec(&body).unwrap();
    let (status, len) = api::handle(s, "POST", path, auth, &body, &mut output);
    (
        status,
        if len == 0 {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&output[..len]).unwrap()
        },
    )
}
pub fn login<J: Journal>(s: &mut Service<J>) -> std::string::String {
    login_as(s, "nana", "1234")
}
pub fn login_as<J: Journal>(
    s: &mut Service<J>,
    username: &str,
    password: &str,
) -> std::string::String {
    let verifier = "a".repeat(43);
    let challenge = nanacoin::auth::challenge(&verifier).unwrap();
    let (status, response) = call(
        s,
        "/api/v1/auth/authorize",
        "",
        serde_json::json!({"username":username,"password":password,"code_challenge":challenge.as_str(),"code_challenge_method":"S256","redirect_uri":"http://localhost/"}),
    );
    assert_eq!(status, 200, "{response}");
    let (status, response) = call(
        s,
        "/api/v1/auth/token",
        "",
        serde_json::json!({"code":response["code"],"code_verifier":verifier,"redirect_uri":"http://localhost/"}),
    );
    assert_eq!(status, 200, "{response}");
    format!("Bearer {}", response["access_token"].as_str().unwrap())
}
