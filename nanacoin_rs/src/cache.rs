//! Only successful, explicitly public read APIs may be stored. Revalidate on
//! every use: ledger retention, currency reform and configuration edits can
//! change even old-looking URLs. The ETag hashes the serialized
//! representation, not a time or a sequence counter that can repeat after a
//! development reset (miniframework's `Reply::revalidate` does the hashing).

/// What a public read may be cached as: stored, but checked every time.
pub const PUBLIC: &str = "public, no-cache";

/// True for a successful GET of a public read. Everything else (private
/// reads, mutations, errors, diagnostics) stays `no-store`.
pub fn public(method: &str, uri: &str, status: u16) -> bool {
    let path = uri.split('?').next().unwrap_or(uri);
    let public = matches!(
        path,
        "/api/v1/configuration" | "/api/v1/transport" | "/api/v1/status" | "/api/v1/transactions"
    ) || path
        .strip_prefix("/api/v1/transactions/")
        .and_then(|id| id.strip_prefix("tx-"))
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
    method == "GET" && status == 200 && public
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_reads() {
        for path in [
            "/api/v1/status",
            "/api/v1/configuration",
            "/api/v1/transport",
            "/api/v1/transactions?limit=10",
            "/api/v1/transactions/tx-42",
        ] {
            assert!(public("GET", path, 200), "{path}");
            assert!(!public("GET", path, 404), "{path}");
            assert!(!public("GET", path, 503), "{path}");
        }
    }

    #[test]
    fn private_mutations_errors_and_diagnostics_are_never_public() {
        for (method, path, status) in [
            ("GET", "/api/v1/me", 200),
            ("GET", "/api/v1/state", 200),
            ("GET", "/api/v1/users", 200),
            ("GET", "/api/v1/diag", 200),
            ("GET", "/api/v1/diag/database", 200),
            ("GET", "/api/v1/transactions/tx-", 200),
            ("GET", "/api/v1/transactions/tx-4a", 200),
            ("POST", "/api/v1/transactions", 200),
            ("GET", "/api/v1/configuration", 403),
            ("GET", "/api/v1/status", 503),
        ] {
            assert!(!public(method, path, status), "{method} {path} {status}");
        }
    }
}
