//! Read-only flash assets. Routing and negotiation allocate no request storage.
pub struct Asset {
    pub path: &'static str,
    pub mime: &'static str,
    pub raw: &'static [u8],
    pub gzip: &'static [u8],
    pub etag: &'static str,
    pub immutable: bool,
}

#[cfg(feature = "bundled-web")]
include!(concat!(env!("OUT_DIR"), "/assets.rs"));
#[cfg(not(feature = "bundled-web"))]
static ASSETS: &[Asset] = &[];

pub struct Reply {
    pub status: u16,
    pub bytes: &'static [u8],
    pub headers: heapless::Vec<(&'static str, &'static str), 8>,
}

pub fn is_api(uri: &str) -> bool {
    let path = uri.split('?').next().unwrap_or(uri);
    path == "/api" || path.starts_with("/api/")
}

fn quality(header: &str, wanted: &str, default: f32) -> f32 {
    let mut wildcard = None;
    for part in header.split(',') {
        let mut fields = part.trim().split(';');
        let name = fields.next().unwrap_or("").trim();
        let mut q = 1.0;
        for field in fields {
            if let Some(value) = field.trim().strip_prefix("q=") {
                q = value
                    .parse::<f32>()
                    .ok()
                    .filter(|v| (0.0..=1.0).contains(v))
                    .unwrap_or(0.0);
            }
        }
        if name.eq_ignore_ascii_case(wanted) {
            return q;
        }
        if name == "*" {
            wildcard = Some(q);
        }
    }
    // Identity is acceptable unless explicitly excluded (or wildcard q=0).
    if wanted == "identity" {
        if wildcard == Some(0.0) {
            0.0
        } else {
            default
        }
    } else {
        wildcard.unwrap_or(default)
    }
}

pub fn respond(method: &str, uri: &str, accept: &str, etag: &str) -> Option<Reply> {
    if let Some(reply) = onboarding(method, uri, false) {
        return Some(reply);
    }
    route(ASSETS, method, uri, accept, etag)
}

pub fn onboarding(method: &str, uri: &str, locked_http: bool) -> Option<Reply> {
    #[cfg(feature = "bundled-web")]
    {
        trust_route(method, uri, locked_http, TRUST_HTML, CA_CERT)
    }
    #[cfg(not(feature = "bundled-web"))]
    {
        let _ = (method, uri, locked_http);
        None
    }
}

#[cfg(any(feature = "bundled-web", test))]
fn trust_route(
    method: &str,
    uri: &str,
    locked_http: bool,
    html: &'static [u8],
    ca: &'static [u8],
) -> Option<Reply> {
    let path = uri.split('?').next().unwrap_or(uri);
    if !locked_http && !matches!(path, "/ca" | "/trust") {
        return None;
    }
    let mut reply = Reply {
        status: 200,
        bytes: b"",
        headers: heapless::Vec::new(),
    };
    reply.headers.push(("Cache-Control", "no-store")).unwrap();
    reply
        .headers
        .push(("X-Content-Type-Options", "nosniff"))
        .unwrap();
    reply
        .headers
        .push(("Referrer-Policy", "no-referrer"))
        .unwrap();
    if method != "GET" {
        reply.status = 405;
        reply.headers.push(("Allow", "GET")).unwrap();
    } else if path == "/ca" {
        reply.bytes = ca;
        reply
            .headers
            .push(("Content-Type", "application/x-x509-ca-cert"))
            .unwrap();
        reply
            .headers
            .push((
                "Content-Disposition",
                "attachment; filename=\"NanaCoin-Home-CA.crt\"",
            ))
            .unwrap();
    } else if path == "/trust" || (locked_http && path == "/") {
        reply.bytes = html;
        reply
            .headers
            .push(("Content-Type", "text/html; charset=utf-8"))
            .unwrap();
        reply.headers.push(("Content-Security-Policy", "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")).unwrap();
    } else {
        reply.status = 403;
    }
    Some(reply)
}

fn route(
    assets: &'static [Asset],
    method: &str,
    uri: &str,
    accept: &str,
    etag: &str,
) -> Option<Reply> {
    if is_api(uri) || assets.is_empty() {
        return None;
    }
    let mut reply = Reply {
        status: 404,
        bytes: b"Not found",
        headers: heapless::Vec::new(),
    };
    reply
        .headers
        .push(("X-Content-Type-Options", "nosniff"))
        .unwrap();
    reply.headers.push(("Vary", "Accept-Encoding")).unwrap();
    if method != "GET" {
        reply.status = 405;
        reply.bytes = b"Method not allowed";
        reply.headers.push(("Allow", "GET")).unwrap();
    } else {
        let path = uri.split('?').next().unwrap_or(uri);
        // Exact manifest lookup: no filesystem traversal/percent decoding.
        // Only actual Angular routes get SPA fallback; missing assets stay 404.
        let spa = matches!(
            path.trim_end_matches('/'),
            "" | "/market"
                | "/send"
                | "/offers"
                | "/forex"
                | "/history"
                | "/economy"
                | "/clientlog"
                | "/logs"
                | "/nana"
                | "/diagnostics"
                | "/about"
                | "/recipes"
                | "/ledger"
                | "/nickles"
        );
        let path = if spa { "/index.html" } else { path };
        if let Some(asset) = assets.iter().find(|a| a.path == path) {
            let gz = quality(accept, "gzip", 0.0);
            let identity = quality(accept, "identity", 1.0);
            if gz == 0.0 && identity == 0.0 {
                reply.status = 406;
                reply.bytes = b"No acceptable representation";
            } else {
                reply.status = 200;
                reply.bytes = if gz > 0.0 && gz >= identity {
                    reply.headers.push(("Content-Encoding", "gzip")).unwrap();
                    asset.gzip
                } else {
                    asset.raw
                };
                reply.headers.push(("Content-Type", asset.mime)).unwrap();
                reply.headers.push(("ETag", asset.etag)).unwrap();
                reply
                    .headers
                    .push((
                        "Cache-Control",
                        if asset.immutable {
                            "public, max-age=31536000, immutable"
                        } else {
                            "no-cache"
                        },
                    ))
                    .unwrap();
                if etag.split(',').any(|tag| {
                    tag.trim() == "*"
                        || tag.trim().trim_start_matches("W/")
                            == asset.etag.trim_start_matches("W/")
                }) {
                    reply.status = 304;
                    reply.bytes = b"";
                }
                return Some(reply);
            }
        }
    }
    reply
        .headers
        .push(("Content-Type", "text/plain; charset=utf-8"))
        .unwrap();
    reply.headers.push(("Cache-Control", "no-store")).unwrap();
    Some(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trust_and_locked_http_are_public_only() {
        let ca = trust_route("GET", "/ca", true, b"help", b"certificate").unwrap();
        assert_eq!(ca.bytes, b"certificate");
        assert!(ca
            .headers
            .contains(&("Content-Type", "application/x-x509-ca-cert")));
        for path in ["/", "/trust?x=1"] {
            assert_eq!(
                trust_route("GET", path, true, b"help", b"cert")
                    .unwrap()
                    .bytes,
                b"help"
            );
        }
        for path in [
            "/api/v1/status",
            "/api/v1/auth/token",
            "/index.html",
            "/main.js",
            "/rootCA-key.pem",
            "/ca/../config.py",
        ] {
            assert_eq!(
                trust_route("GET", path, true, b"help", b"cert")
                    .unwrap()
                    .status,
                403
            );
            assert_eq!(
                trust_route("POST", path, true, b"help", b"cert")
                    .unwrap()
                    .status,
                405
            );
        }
        assert!(trust_route("GET", "/api/v1/status", false, b"help", b"cert").is_none());
    }
    static FIXTURE: &[Asset] = &[
        Asset {
            path: "/index.html",
            mime: "text/html",
            raw: b"html",
            gzip: b"gz",
            etag: "W/\"index\"",
            immutable: false,
        },
        Asset {
            path: "/main-12345678.js",
            mime: "text/javascript",
            raw: b"js",
            gzip: b"gzjs",
            etag: "W/\"js\"",
            immutable: true,
        },
    ];
    #[test]
    fn paths_and_methods() {
        for path in ["/", "/nana", "/market?x=1", "/diagnostics/"] {
            assert_eq!(route(FIXTURE, "GET", path, "", "").unwrap().bytes, b"html");
        }
        for path in [
            "/missing.js",
            "/../index.html",
            "/%2e%2e/index.html",
            "/secrets",
            "/apiary",
        ] {
            assert_eq!(route(FIXTURE, "GET", path, "", "").unwrap().status, 404);
        }
        assert!(route(FIXTURE, "GET", "/api/v1/missing", "", "").is_none());
        assert_eq!(route(FIXTURE, "POST", "/", "", "").unwrap().status, 405);
        assert_eq!(route(FIXTURE, "HEAD", "/", "", "").unwrap().status, 405);
    }
    #[test]
    fn negotiation_and_cache() {
        let get = |accept, etag| route(FIXTURE, "GET", "/main-12345678.js", accept, etag).unwrap();
        assert_eq!(get("gzip, br", "").bytes, b"gzjs");
        assert_eq!(get("gzip;q=0", "").bytes, b"js");
        assert_eq!(get("gzip;q=0.5, identity;q=0.1", "").bytes, b"gzjs");
        assert_eq!(get("*;q=0", "").status, 406);
        assert_eq!(get("gzip", "\"js\"").status, 304);
        assert!(get("", "")
            .headers
            .contains(&("Cache-Control", "public, max-age=31536000, immutable")));
    }
}
