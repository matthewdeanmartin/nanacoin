//! Only successful, explicitly public read APIs may be stored. Revalidate on
//! every use: ledger retention, currency reform and configuration edits can
//! change even old-looking URLs. Hash the serialized representation, not time
//! or a sequence counter that can repeat after a development reset.
use sha2::{Digest, Sha256};

pub struct Policy {
    pub status: u16,
    pub control: &'static str,
    pub etag: Option<String>,
}

pub fn api(method: &str, uri: &str, status: u16, body: &[u8], validator: &str) -> Policy {
    let path = uri.split('?').next().unwrap_or(uri);
    let public = matches!(
        path,
        "/api/v1/configuration" | "/api/v1/transport" | "/api/v1/status" | "/api/v1/transactions"
    ) || path
        .strip_prefix("/api/v1/transactions/")
        .and_then(|id| id.strip_prefix("tx-"))
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
    if method != "GET" || status != 200 || !public {
        return Policy {
            status,
            control: "no-store",
            etag: None,
        };
    }
    let etag = format!("\"{:x}\"", Sha256::digest(body));
    Policy {
        status: if etag_matches(Some(validator), &etag) {
            304
        } else {
            status
        },
        control: "public, no-cache",
        etag: Some(etag),
    }
}

/// If-None-Match uses weak comparison, including lists and the wildcard.
/// Commas inside a quoted opaque tag are not list separators.
pub fn etag_matches(header: Option<&str>, etag: &str) -> bool {
    let Some(header) = header else {
        return false;
    };
    if header.trim() == "*" {
        return true;
    }
    let mut quoted = false;
    header
        .split(|ch| {
            if ch == '"' {
                quoted = !quoted;
            }
            ch == ',' && !quoted
        })
        .any(|candidate| {
            let candidate = candidate.trim();
            candidate.strip_prefix("W/").unwrap_or(candidate)
                == etag.strip_prefix("W/").unwrap_or(etag)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_reads_revalidate_and_changes_bust_the_tag() {
        for path in [
            "/api/v1/status",
            "/api/v1/configuration",
            "/api/v1/transport",
            "/api/v1/transactions?limit=10",
            "/api/v1/transactions/tx-42",
        ] {
            let first = api("GET", path, 200, b"old", "");
            assert_eq!(first.control, "public, no-cache");
            let tag = first.etag.unwrap();
            for validator in [tag.clone(), format!("\"other\", W/{tag}"), "*".into()] {
                assert_eq!(api("GET", path, 200, b"old", &validator).status, 304);
            }
            let changed = api("GET", path, 200, b"new", &tag);
            assert_eq!(changed.status, 200);
            assert_ne!(changed.etag.as_deref(), Some(tag.as_str()));
            assert_eq!(api("GET", path, 404, b"missing", &tag).control, "no-store");
        }
    }

    #[test]
    fn private_mutations_errors_and_diagnostics_never_validate() {
        for (method, path, status) in [
            ("GET", "/api/v1/me", 200),
            ("GET", "/api/v1/state", 200),
            ("GET", "/api/v1/users", 200),
            ("GET", "/api/v1/diag", 200),
            ("GET", "/api/v1/diag/database", 200),
            ("POST", "/api/v1/transactions", 200),
            ("GET", "/api/v1/configuration", 403),
            ("GET", "/api/v1/status", 503),
        ] {
            let policy = api(method, path, status, b"secret", "*");
            assert_eq!(policy.status, status);
            assert_eq!(policy.control, "no-store");
            assert!(policy.etag.is_none());
        }
        assert!(!etag_matches(Some("\"a,b\""), "\"b\""));
        assert!(etag_matches(Some("\"a,b\""), "W/\"a,b\""));
    }
}
