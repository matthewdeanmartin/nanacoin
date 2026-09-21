//! Password/PIN provisioning and login preserve Nanacoin's original security
//! model. The financial API remains a typed Rust command endpoint.
use crate::{
    auth::{PasswordVerifier, SESSION_TTL},
    domain::*,
    journal::{Journal, Service},
};
use heapless::String;
use serde::{Deserialize, Serialize};

pub const BODY_LIMIT: usize = 1024;
pub const RESPONSE_LIMIT: usize = 512 * 1024;
pub const DEFAULT_ORIGINS: &str =
    "http://localhost:4200,http://127.0.0.1:4200,http://nanacoin.local,https://nanacoin.local";
pub fn origin_allowed(origin: &str, allowed: &str) -> bool {
    origin.is_empty()
        || allowed
            .split(',')
            .map(str::trim)
            .any(|entry| entry == origin)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    request_id: u64,
    command: Command,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provision {
    household_name: Name,
    username: Name,
    display_name: Name,
    password: String<128>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authorize {
    username: Name,
    password: String<128>,
    code_challenge: String<128>,
    code_challenge_method: String<8>,
    redirect_uri: String<256>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Redeem {
    code: String<128>,
    code_verifier: String<128>,
    redirect_uri: String<256>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMember {
    username: Name,
    display_name: Name,
    password: String<128>,
    #[serde(default = "default_role")]
    role: Role,
    #[serde(default = "default_grant")]
    grant: bool,
    #[serde(default)]
    mastodon_id: MastodonId,
}
fn default_role() -> Role {
    Role::User
}
fn default_grant() -> bool {
    true
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMember {
    member: MemberId,
    display_name: Option<Name>,
    password: Option<String<128>>,
    role: Option<Role>,
    disabled: Option<bool>,
    mastodon_id: Option<MastodonId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageAction {
    expected_generation: u64,
    expected_sequence: u64,
    confirmation: Option<String<32>>,
}

#[derive(Serialize)]
struct StorageStatus {
    generation: u64,
    sequence: u64,
    journal_records: usize,
    checkpoint_after: usize,
    checkpoint_supported: bool,
}

fn storage_status<J: Journal>(service: &Service<J>) -> StorageStatus {
    StorageStatus {
        generation: service.generation(),
        sequence: service.state.sequence,
        journal_records: service.journal_records(),
        checkpoint_after: 2048,
        checkpoint_supported: service.checkpoint_supported(),
    }
}
#[derive(Serialize)]
struct View<'a> {
    member: MemberId,
    state: &'a State,
    storage_failed: bool,
}

pub fn handle<J: Journal>(
    service: &mut Service<J>,
    method: &str,
    path: &str,
    authorization: &str,
    body: &[u8],
    output: &mut [u8],
) -> (u16, usize) {
    handle_keyed(service, method, path, authorization, "", body, output)
}
#[allow(clippy::too_many_arguments)]
pub fn handle_keyed_on<J: Journal>(
    service: &mut Service<J>,
    method: &str,
    path: &str,
    authorization: &str,
    key: &str,
    body: &[u8],
    output: &mut [u8],
    tls: bool,
) -> (u16, usize) {
    if !tls && (service.https_only() || (method == "POST" && path == "/api/v1/admin/transport")) {
        return error_response(Error::Forbidden, output);
    }
    handle_keyed(service, method, path, authorization, key, body, output)
}

#[allow(clippy::too_many_arguments)]
pub fn handle_keyed<J: Journal>(
    service: &mut Service<J>,
    method: &str,
    path: &str,
    authorization: &str,
    idempotency_key: &str,
    body: &[u8],
    output: &mut [u8],
) -> (u16, usize) {
    let result = (|| {
        if !path.starts_with("/api/v1/") {
            return Err(Error::NotFound);
        }
        if body.len() > BODY_LIMIT {
            return Err(Error::Capacity);
        }
        if service.storage_failed() {
            return Err(Error::Storage);
        }
        let (route_path, query) = path.split_once('?').unwrap_or((path, ""));
        // Like a Bitcoin explorer or Nana's paper notebook, the ledger is
        // readable without a session. Mutations and account administration
        // remain authenticated below.
        if method == "GET" && route_path == "/api/v1/transactions" {
            let limit = query
                .split('&')
                .find_map(|p| p.strip_prefix("limit="))
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(100)
                .min(100);
            return crate::client::public_ledger(service.state(), limit, output);
        }
        if method == "GET" {
            if let Some(id) = route_path.strip_prefix("/api/v1/transactions/") {
                return crate::client::public_transaction(service.state(), id, output);
            }
        }
        let now = service.auth.now();
        match (method, path) {
            ("GET", "/api/v1/transport") => {
                #[derive(Serialize)]
                struct Policy {
                    https_only: bool,
                    supported: bool,
                }
                return serialize(
                    &Policy {
                        https_only: service.https_only(),
                        supported: service.supports_transport(),
                    },
                    output,
                );
            }
            ("GET", "/api/v1/status") => {
                return crate::client::status(
                    &service.state,
                    service.journal_records(),
                    service.generation(),
                    service.checkpoint_supported(),
                    output,
                );
            }
            ("POST", "/api/v1/provision") => {
                if !service.state.members.is_empty() {
                    return Err(Error::Forbidden);
                }
                let req: Provision = parse(body)?;
                let display_name = if req.display_name.trim().is_empty() {
                    req.username.clone()
                } else {
                    req.display_name
                };
                service.execute(
                    MemberId(1),
                    1,
                    Command::Provision {
                        household_name: req.household_name,
                        username: req.username,
                        display_name,
                        password: PasswordVerifier::hash(&req.password)?,
                    },
                )?;
                return serialize(
                    &crate::client::user(service.state.member(MemberId(1))?),
                    output,
                );
            }
            ("POST", "/api/v1/auth/authorize") => {
                let req: Authorize = parse(body)?;
                let code = service.auth.authorize(
                    &service.state,
                    &req.username,
                    &req.password,
                    &req.code_challenge,
                    &req.code_challenge_method,
                    &req.redirect_uri,
                    now,
                )?;
                #[derive(Serialize)]
                struct Response {
                    code: crate::auth::Secret,
                }
                return serialize(&Response { code }, output);
            }
            ("POST", "/api/v1/auth/token") => {
                let req: Redeem = parse(body)?;
                let (token, member) = service.auth.redeem(
                    &service.state,
                    &req.code,
                    &req.code_verifier,
                    &req.redirect_uri,
                    now,
                )?;
                #[derive(Serialize)]
                struct Response<'a> {
                    access_token: crate::auth::Secret,
                    token_type: &'static str,
                    expires_in: u64,
                    user: crate::client::User<'a>,
                }
                return serialize(
                    &Response {
                        access_token: token,
                        token_type: "Bearer",
                        expires_in: SESSION_TTL,
                        user: crate::client::user(service.state.member(member)?),
                    },
                    output,
                );
            }
            ("POST", "/api/v1/auth/logout") => {
                if let Some(token) = authorization.strip_prefix("Bearer ") {
                    service.auth.revoke(token);
                }
                return serialize(&true, output);
            }
            _ => {}
        }
        let token = authorization
            .strip_prefix("Bearer ")
            .ok_or(Error::Unauthorized)?;
        let actor = service.auth.lookup(&service.state, token, now)?;
        // After a reform an old screen cannot submit amounts in the old unit.
        // Keyed commands also check this after durable retry receipt lookup.
        if method != "GET" && service.state.money_epoch > 0 && path != "/api/v1/admin/reform" {
            let epoch = idempotency_key
                .split(':')
                .nth(1)
                .and_then(|s| s.strip_prefix('m'))
                .and_then(|s| s.parse::<u64>().ok());
            if epoch != Some(service.state.money_epoch) {
                return Err(Error::StaleRequest);
            }
        }
        if path == "/api/v1/admin/transport" && method == "POST" {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Action {
                confirmation: heapless::String<32>,
            }
            let action: Action = parse(body)?;
            if action.confirmation.as_str() != "REQUIRE HTTPS" {
                return Err(Error::InvalidInput);
            }
            service.require_https(actor)?;
            return serialize(&true, output);
        }
        if path == "/api/v1/admin/storage" && method == "GET" {
            service.state.admin(actor)?;
            return serialize(&storage_status(service), output);
        }
        if method == "POST" && matches!(path, "/api/v1/admin/checkpoint" | "/api/v1/admin/reset") {
            service.state.admin(actor)?;
            let action: StorageAction = parse(body)?;
            if action.expected_generation != service.generation()
                || action.expected_sequence != service.state.sequence
            {
                return Err(Error::Conflict);
            }
            if path.ends_with("/reset") {
                if action.confirmation.as_deref() != Some("RESET ECONOMY") {
                    return Err(Error::InvalidInput);
                }
                service.reset_economy(actor)?;
            } else {
                service.checkpoint(actor)?;
            }
            return serialize(&storage_status(service), output);
        }
        match (method, path) {
            ("GET", "/api/v1/state") => {
                service.state.admin(actor)?;
                serialize(
                    &View {
                        member: actor,
                        state: service.state(),
                        storage_failed: false,
                    },
                    output,
                )
            }
            ("POST", "/api/v1/users") => {
                if service.state.member(actor)?.role != Role::Nana {
                    return Err(Error::Forbidden);
                }
                let req: CreateMember = parse(body)?;
                let display_name = if req.display_name.trim().is_empty() {
                    req.username.clone()
                } else {
                    req.display_name
                };
                let nonce = service.state.member(actor)?.last_request + 1;
                service.execute(
                    actor,
                    nonce,
                    Command::CreateMember {
                        username: req.username,
                        display_name,
                        password: PasswordVerifier::hash(&req.password)?,
                        role: req.role,
                        grant: if req.grant {
                            service.state.initial_grant
                        } else {
                            0
                        },
                        mastodon_id: req.mastodon_id,
                    },
                )?;
                serialize(
                    &crate::client::user(service.state.members.last().unwrap()),
                    output,
                )
            }
            ("POST", "/api/v1/users/update") => {
                let req: UpdateMember = parse(body)?;
                if service.state.member(actor)?.role != Role::Nana
                    && (actor != req.member || req.role.is_some() || req.disabled.is_some())
                {
                    return Err(Error::Forbidden);
                }
                let nonce = service.state.member(actor)?.last_request + 1;
                let receipt = service.execute(
                    actor,
                    nonce,
                    Command::UpdateMember {
                        member: req.member,
                        display_name: req.display_name,
                        password: req
                            .password
                            .as_ref()
                            .map(|p| PasswordVerifier::hash(p))
                            .transpose()?,
                        role: req.role,
                        disabled: req.disabled,
                        mastodon_id: req.mastodon_id,
                    },
                )?;
                serialize(&receipt, output)
            }
            ("POST", "/api/v1/commands") => {
                let request: Request = parse(body)?;
                // Credential/verifier-bearing variants are internal journal
                // events, not public commands. The server salts all passwords.
                if matches!(
                    request.command,
                    Command::AddMember { .. }
                        | Command::Provision { .. }
                        | Command::CreateMember { .. }
                        | Command::UpdateMember { .. }
                        | Command::MigrateMember { .. }
                ) {
                    return Err(Error::Forbidden);
                }
                let receipt = service.execute(actor, request.request_id, request.command)?;
                serialize(&receipt, output)
            }
            _ => crate::client::route(service, actor, method, path, idempotency_key, body, output),
        }
    })();
    match result {
        Ok(len) => {
            let path = path.split('?').next().unwrap_or(path);
            if method == "POST" && path == "/api/v1/auth/logout" {
                return (204, 0);
            }
            let created = method == "POST"
                && (matches!(
                    path,
                    "/api/v1/provision"
                        | "/api/v1/users"
                        | "/api/v1/listings"
                        | "/api/v1/quotes"
                        | "/api/v1/transfers"
                        | "/api/v1/admin/issue"
                        | "/api/v1/admin/retire"
                        | "/api/v1/admin/issue-usd"
                ) || (path.starts_with("/api/v1/transactions/") && path.ends_with("/reverse"))
                    || (path.starts_with("/api/v1/listings/") && path.ends_with("/purchase"))
                    || (path.starts_with("/api/v1/quotes/") && path.ends_with("/take"))
                    || (path.starts_with("/api/v1/listings/") && path.ends_with("/offers"))
                    || (path.starts_with("/api/v1/offers/") && path.ends_with("/accept")));
            (if created { 201 } else { 200 }, len)
        }
        Err(error) => error_response(error, output),
    }
}
pub(crate) fn parse<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    crate::json::decode(body).map_err(|_| Error::InvalidInput)
}
pub(crate) fn serialize<T: Serialize>(value: &T, output: &mut [u8]) -> Result<usize, Error> {
    serde_json_core::to_slice(value, output).map_err(|_| Error::Capacity)
}

pub fn error_response(error: Error, output: &mut [u8]) -> (u16, usize) {
    let status = match error {
        Error::Unauthorized | Error::InvalidCredentials => 401,
        Error::Forbidden | Error::Disabled => 403,
        Error::NotFound => 404,
        Error::Conflict
        | Error::StaleRequest
        | Error::InsufficientFunds
        | Error::OfferClosed
        | Error::OfferSettled
        | Error::ListingClosed => 409,
        Error::Capacity => 507,
        Error::Storage | Error::CorruptJournal | Error::Unavailable => 503,
        Error::InvalidInput | Error::Overflow | Error::SelfDeal => 400,
        Error::RateLimited => 429,
    };
    #[derive(Serialize)]
    struct Failure {
        error: Error,
        message: &'static str,
    }
    (
        status,
        serde_json_core::to_slice(
            &Failure {
                error,
                message: error_message(error),
            },
            output,
        )
        .unwrap_or(0),
    )
}

fn error_message(error: Error) -> &'static str {
    match error {
        Error::InvalidCredentials => "Username or password is incorrect",
        Error::Unauthorized => "Please log in again",
        Error::Disabled => "This account is disabled",
        Error::RateLimited => "Too many failed attempts. Try again in five minutes",
        Error::Forbidden => "This action is not permitted",
        Error::NotFound => "This resource or API operation is not available",
        Error::InvalidInput => "Invalid or unsupported input",
        Error::InsufficientFunds => "Insufficient funds",
        Error::Capacity => "The configured capacity has been reached",
        Error::StaleRequest => {
            "This retry is older than the retained response; it was not repeated"
        }
        Error::Conflict => "This request conflicts with an existing operation",
        Error::Overflow => "The amount exceeds the supported range",
        Error::OfferClosed => "This offer is no longer open for that action",
        Error::OfferSettled => "This offer has settled and can no longer be undone",
        Error::ListingClosed => "This listing is no longer active",
        Error::SelfDeal => "You cannot offer on your own listing",
        _ => "The service is unavailable; check the server log",
    }
}
