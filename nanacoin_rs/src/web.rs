//! The bundled Angular site, trust page and household CA. miniframework
//! serves them (`crate::server::config`); this module only says what they are.
pub use miniframework::web::Asset;

#[cfg(feature = "bundled-web")]
include!(concat!(env!("OUT_DIR"), "/assets.rs"));
#[cfg(not(feature = "bundled-web"))]
static ASSETS: &[Asset] = &[];
#[cfg(not(feature = "bundled-web"))]
static TRUST_HTML: &[u8] = b"";
#[cfg(not(feature = "bundled-web"))]
static CA_CERT: &[u8] = b"";

/// The Angular routes. Only these get `/index.html`; any other path without
/// a bundled file is a 404 rather than the app shell.
pub const SPA_ROUTES: &[&str] = &[
    "",
    "/market",
    "/send",
    "/offers",
    "/forex",
    "/lotto",
    "/messages",
    "/loans",
    "/history",
    "/economy",
    "/clientlog",
    "/logs",
    "/nana",
    "/diagnostics",
    "/error-log",
    "/database",
    "/configuration",
    "/about",
    "/recipes",
    "/ledger",
    "/nickles",
];

pub fn assets() -> &'static [Asset] {
    ASSETS
}

/// The `/trust` page, when a bundle is built in.
pub fn trust_html() -> Option<&'static [u8]> {
    (!TRUST_HTML.is_empty()).then_some(TRUST_HTML)
}

/// The household CA (DER) offered at `/ca`, when a bundle is built in.
pub fn ca_cert() -> Option<&'static [u8]> {
    (!CA_CERT.is_empty()).then_some(CA_CERT)
}
