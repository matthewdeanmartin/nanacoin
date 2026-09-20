use nanacoin::{
    api,
    journal::{file::FileJournal, Service},
};
use std::time::Duration;
use tiny_http::{Header, Response, Server};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = std::env::var("NANACOIN_JOURNAL").unwrap_or_else(|_| "nanacoin.journal".into());
    let mut service =
        Service::open(FileJournal::open(path)?).map_err(|e| format!("journal: {e:?}"))?;
    if std::env::args().any(|arg| arg == "--migrate-login") {
        migrate_login(&mut service)?;
        return Ok(());
    }
    if service.state().needs_login_migration() {
        eprintln!("Existing token-era accounts preserved. Stop the server and run make migrate-login to choose a username and PIN/password.");
    }
    let port: u16 = std::env::var("NANACOIN_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;
    let address = format!("127.0.0.1:{port}");
    let server = Server::http(&address)?;
    let mut output = vec![0u8; api::RESPONSE_LIMIT].into_boxed_slice();
    println!("NanaCoin: http://{address} (loopback development server)");
    let origins = format!(
        "{},http://{address},http://localhost:{port}",
        std::env::var("NANACOIN_ORIGINS").unwrap_or_else(|_| api::DEFAULT_ORIGINS.into())
    );
    loop {
        let Some(request) = server.recv_timeout(Duration::from_secs(1))? else {
            continue;
        };
        if let Err(error) = serve(request, &mut service, &mut output, &origins) {
            eprintln!("request disconnected: {error}");
        }
    }
}

fn serve(
    mut request: tiny_http::Request,
    service: &mut Service<FileJournal>,
    output: &mut [u8],
    origins: &str,
) -> std::io::Result<()> {
    let reply = if service.https_only() {
        nanacoin::web::onboarding(request.method().as_str(), request.url(), true)
    } else {
        nanacoin::web::respond(
            request.method().as_str(),
            request.url(),
            request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Accept-Encoding"))
                .map(|h| h.value.as_str())
                .unwrap_or(""),
            request
                .headers()
                .iter()
                .find(|h| h.field.equiv("If-None-Match"))
                .map(|h| h.value.as_str())
                .unwrap_or(""),
        )
    };
    if let Some(reply) = reply {
        return request.respond(Response::new(
            reply.status.into(),
            reply.headers.iter().map(|(k, v)| header(k, v)).collect(),
            reply.bytes,
            Some(reply.bytes.len()),
            None,
        ));
    }
    let origin = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Origin"))
        .map(|h| h.value.as_str())
        .unwrap_or("")
        .to_owned();
    let allowed = api::origin_allowed(&origin, origins);
    let mut headers = vec![
        header("Content-Type", "application/json"),
        header("Cache-Control", "no-store"),
        header("Vary", "Origin"),
    ];
    if allowed && !origin.is_empty() {
        headers.push(header("Access-Control-Allow-Origin", &origin));
        headers.push(header(
            "Access-Control-Allow-Methods",
            "GET, POST, PATCH, OPTIONS",
        ));
        headers.push(header(
            "Access-Control-Allow-Headers",
            "Authorization, Content-Type, Idempotency-Key",
        ));
    }
    let method = request.method().as_str().to_owned();
    let (status, len) = if !allowed {
        api::error_response(nanacoin::domain::Error::Forbidden, output)
    } else if method == "OPTIONS" {
        output[..2].copy_from_slice(b"{}");
        (200, 2)
    } else {
        let length = if method == "POST" || method == "PATCH" {
            request.body_length().unwrap_or(usize::MAX)
        } else {
            0
        };
        if length > api::BODY_LIMIT {
            let (_, len) = api::error_response(nanacoin::domain::Error::Capacity, output);
            (413, len)
        } else {
            let mut body = [0; api::BODY_LIMIT];
            if request.as_reader().read_exact(&mut body[..length]).is_err() {
                api::error_response(nanacoin::domain::Error::InvalidInput, output)
            } else {
                let auth = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Authorization"))
                    .map(|h| h.value.as_str())
                    .unwrap_or("");
                let key = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Idempotency-Key"))
                    .map(|h| h.value.as_str())
                    .unwrap_or("");
                api::handle_keyed_on(
                    service,
                    &method,
                    request.url(),
                    auth,
                    key,
                    &body[..length],
                    output,
                    false,
                )
            }
        }
    };
    headers.push(header(
        "X-Nanacoin-Generation",
        &service.generation().to_string(),
    ));
    headers.push(header(
        "Access-Control-Expose-Headers",
        "X-Nanacoin-Generation",
    ));
    request.respond(Response::new(
        status.into(),
        headers,
        &output[..len],
        Some(len),
        None,
    ))
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).unwrap()
}

fn migrate_login(
    service: &mut Service<FileJournal>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use nanacoin::{
        auth::PasswordVerifier,
        domain::{Command, Name},
    };
    use std::io::Read;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Migration {
        token: heapless::String<128>,
        username: Name,
        password: heapless::String<128>,
    }
    let mut input = [0u8; 1025];
    let mut len = 0;
    let mut stdin = std::io::stdin().lock();
    while len < input.len() {
        let n = stdin.read(&mut input[len..])?;
        if n == 0 {
            break;
        }
        len += n;
    }
    if len > 1024 {
        return Err("migration input too large".into());
    }
    let mut scratch = [0u8; 1024];
    let (req, used): (Migration, _) =
        serde_json_core::from_slice_escaped(&input[..len], &mut scratch)
            .map_err(|_| "invalid migration input")?;
    if !input[used..len].iter().all(u8::is_ascii_whitespace) {
        return Err("trailing migration input".into());
    }
    let member = service
        .state()
        .authenticate_legacy(&req.token)
        .map_err(|_| "legacy credential is invalid or already migrated")?;
    let nonce = service
        .state()
        .member(member)
        .map_err(|_| "member not found")?
        .last_request
        + 1;
    let password = PasswordVerifier::hash(&req.password).map_err(|e| format!("password: {e:?}"))?;
    service
        .execute(
            member,
            nonce,
            Command::MigrateMember {
                member,
                username: req.username,
                password,
            },
        )
        .map_err(|e| format!("migration: {e:?}"))?;
    println!("Login migrated. Ledger and member identity preserved; the old access token no longer authenticates.");
    Ok(())
}
