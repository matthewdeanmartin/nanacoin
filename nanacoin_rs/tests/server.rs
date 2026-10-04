//! NanaCoin through miniframework's request pipeline: the same `Site` the
//! desktop server and both banks run, without sockets.
mod common;
use miniframework::{desktop::DesktopPlatform, http, site::Scratch, Site};
use nanacoin::{
    api,
    journal::{file::FileJournal, Service},
    server::{self, Bank},
};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

struct Wire {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Wire {
    fn header(&self, name: &str) -> &str {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map_or("", |(_, v)| v.as_str())
    }
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

type Bank0 = Bank<FileJournal>;

fn bank(name: &str) -> (Site<Bank0>, Arc<Mutex<Service<FileJournal>>>, String) {
    let dir = std::env::temp_dir().join(format!("nanacoin-server-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut service = Service::open(FileJournal::open(dir.join("test.journal")).unwrap()).unwrap();
    common::provision(&mut service);
    let auth = common::login(&mut service);
    let ledger = Arc::new(Mutex::new(service));
    let site = Site::new(
        server::config("nanacoin.local", "http://localhost:4200"),
        Bank::new(Arc::clone(&ledger)),
        DesktopPlatform,
    );
    (site, ledger, auth)
}

fn send(site: &Site<Bank0>, raw: &str, secure: bool) -> Wire {
    let request = http::parse(raw.as_bytes(), api::BODY_LIMIT)
        .unwrap()
        .unwrap();
    let mut scratch = Scratch::new(site.config.response_limit);
    let mut response = site.respond(&request, secure, &mut scratch);
    let mut out = Vec::new();
    while !response.next().is_empty() {
        let n = response.next().len();
        out.extend_from_slice(response.next());
        response.advance(n);
    }
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let head = std::str::from_utf8(&out[..split]).unwrap();
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Wire {
        status,
        headers,
        body: out[split..].to_vec(),
    }
}

fn get(site: &Site<Bank0>, path: &str, extra: &str, secure: bool) -> Wire {
    send(
        site,
        &format!("GET {path} HTTP/1.1\r\nHost: nanacoin.local\r\n{extra}\r\n"),
        secure,
    )
}

fn post(site: &Site<Bank0>, path: &str, auth: &str, body: &str, secure: bool) -> Wire {
    send(
        site,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: nanacoin.local\r\nAuthorization: {auth}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
        secure,
    )
}

#[test]
fn public_reads_revalidate_and_carry_the_generation() {
    let (site, ledger, _) = bank("public");
    let first = get(&site, "/api/v1/status", "", true);
    assert_eq!(first.status, 200);
    assert_eq!(first.header("Content-Type"), "application/json");
    assert_eq!(first.header("Cache-Control"), "public, no-cache");
    let generation = ledger.lock().unwrap().generation().to_string();
    assert_eq!(first.header("X-Nanacoin-Generation"), generation);
    assert!(first.header("Server-Timing").contains("lock;dur="));
    let etag = first.header("ETag").to_string();
    assert!(etag.starts_with('"'));
    let again = get(
        &site,
        "/api/v1/status",
        &format!("If-None-Match: {etag}\r\n"),
        true,
    );
    assert_eq!(again.status, 304);
    assert!(again.body.is_empty());
    assert_eq!(again.header("ETag"), etag);
}

#[test]
fn private_reads_and_errors_are_never_stored() {
    let (site, _, auth) = bank("private");
    let me = get(
        &site,
        "/api/v1/me",
        &format!("Authorization: {auth}\r\n"),
        true,
    );
    assert_eq!(me.status, 200, "{}", String::from_utf8_lossy(&me.body));
    assert_eq!(me.header("Cache-Control"), "no-store");
    assert_eq!(me.header("ETag"), "");
    let anonymous = get(&site, "/api/v1/me", "If-None-Match: *\r\n", true);
    assert_eq!(anonymous.status, 401);
    assert_eq!(anonymous.header("Cache-Control"), "no-store");
    // Unknown API paths are the ledger's JSON 404, not the framework's.
    let missing = get(
        &site,
        "/api/v1/nothing",
        &format!("Authorization: {auth}\r\n"),
        true,
    );
    assert_eq!(missing.status, 404);
    assert!(missing.json().get("error").is_some());
}

#[test]
fn cors_allows_the_client_and_nanacoin_headers_only() {
    let (site, _, _) = bank("cors");
    let w = get(
        &site,
        "/api/v1/status",
        "Origin: http://localhost:4200\r\n",
        true,
    );
    assert_eq!(
        w.header("Access-Control-Allow-Origin"),
        "http://localhost:4200"
    );
    assert_eq!(
        w.header("Access-Control-Allow-Methods"),
        "GET, POST, PATCH, OPTIONS"
    );
    assert!(w
        .header("Access-Control-Allow-Headers")
        .contains("Idempotency-Key"));
    assert!(w
        .header("Access-Control-Expose-Headers")
        .contains("X-Nanacoin-Generation"));
    // The bank's own origins always work, whatever the allowlist says.
    for origin in ["https://nanacoin.local", "http://nanacoin.local"] {
        let w = get(
            &site,
            "/api/v1/status",
            &format!("Origin: {origin}\r\n"),
            true,
        );
        assert_eq!(w.status, 200, "{origin}");
    }
    let stranger = get(
        &site,
        "/api/v1/status",
        "Origin: http://evil.example\r\n",
        true,
    );
    assert_eq!(stranger.status, 403);
    let preflight = send(
        &site,
        "OPTIONS /api/v1/transactions HTTP/1.1\r\nHost: nanacoin.local\r\nOrigin: http://localhost:4200\r\n\r\n",
        true,
    );
    assert!(preflight.status == 204 || preflight.status == 200);
}

#[test]
fn oversized_bodies_are_refused_before_the_ledger() {
    let (site, _, auth) = bank("body");
    let raw = format!(
        "POST /api/v1/transactions HTTP/1.1\r\nHost: nanacoin.local\r\nAuthorization: {auth}\r\nContent-Length: {}\r\n\r\n",
        api::BODY_LIMIT + 1
    );
    assert_eq!(
        http::parse(raw.as_bytes(), api::BODY_LIMIT).unwrap_err(),
        413
    );
    let _ = site;
}

#[test]
fn requiring_https_locks_plain_http_to_trust_and_metrics() {
    let (site, ledger, auth) = bank("https");
    assert_eq!(get(&site, "/api/v1/status", "", false).status, 200);
    // Switching needs TLS and Nana.
    let body = r#"{"confirmation":"REQUIRE HTTPS"}"#;
    assert_eq!(
        post(&site, "/api/v1/admin/transport", &auth, body, false).status,
        403
    );
    let switched = post(&site, "/api/v1/admin/transport", &auth, body, true);
    assert_eq!(
        switched.status,
        200,
        "{}",
        String::from_utf8_lossy(&switched.body)
    );
    assert!(ledger.lock().unwrap().https_only());
    for path in ["/api/v1/status", "/api/v1/me", "/market", "/api/v1/sys"] {
        assert_eq!(get(&site, path, "", false).status, 403, "{path}");
    }
    let metrics = get(&site, "/metrics", "", false);
    assert_eq!(metrics.status, 200);
    let line = String::from_utf8(metrics.body).unwrap();
    assert!(
        line.starts_with("board,host=nanacoin.local,app=nanacoin,bank="),
        "{line}"
    );
    assert_eq!(get(&site, "/api/v1/status", "", true).status, 200);
}

#[test]
fn diagnostics_are_answered_without_the_ledger_lock() {
    let dir = std::env::temp_dir().join(format!("nanacoin-server-diag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let service = Service::open(FileJournal::open(dir.join("test.journal")).unwrap()).unwrap();
    let ledger = Arc::new(Mutex::new(service));
    let site = Site::new(
        server::config("nanacoin.local", ""),
        Bank::new(Arc::clone(&ledger)).with_diagnostics(Box::new(|path, out| {
            let body = format!("{{\"path\":\"{path}\"}}");
            out[..body.len()].copy_from_slice(body.as_bytes());
            (200, body.len())
        })),
        DesktopPlatform,
    );
    // Hold the ledger for the whole request: a handler that tried to take
    // it would never answer.
    let _held = ledger.lock().unwrap();
    let (done, finished) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let w = get(&site, "/api/v1/diag/events", "", true);
            done.send((w.status, w.body)).unwrap();
        });
        let (status, body) = finished
            .recv_timeout(Duration::from_secs(5))
            .expect("diagnostics waited for the ledger lock");
        assert_eq!(status, 200);
        assert_eq!(body, br#"{"path":"/api/v1/diag/events"}"#);
    });
}

#[cfg(feature = "bundled-web")]
#[test]
fn the_bundled_site_serves_routes_assets_and_trust() {
    let (site, _, _) = bank("bundle");
    let market = get(&site, "/market?test=1", "Accept-Encoding: gzip\r\n", true);
    assert_eq!(market.status, 200);
    assert_eq!(market.header("Content-Encoding"), "gzip");
    let etag = market.header("ETag").to_string();
    let again = get(
        &site,
        "/",
        &format!("Accept-Encoding: gzip\r\nIf-None-Match: {etag}\r\n"),
        true,
    );
    assert_eq!(again.status, 304, "/ and /market are the same shell");
    for path in ["/missing.js", "/secrets", "/apiary", "/%2e%2e/index.html"] {
        assert_eq!(get(&site, path, "", true).status, 404, "{path}");
    }
    let trust = get(&site, "/trust", "", false);
    assert_eq!(trust.status, 200);
    assert_eq!(trust.header("Content-Type"), "text/html; charset=utf-8");
    let ca = get(&site, "/ca", "", false);
    assert_eq!(
        ca.header("Content-Disposition"),
        "attachment; filename=\"NanaCoin-Home-CA.crt\""
    );
    assert!(!ca.body.is_empty());
}
