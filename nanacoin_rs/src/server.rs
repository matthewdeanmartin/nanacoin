//! NanaCoin as a miniframework app. The framework owns the connection loop,
//! HTTPS, static files, CORS, `/trust`, `/ca` and `/metrics`; this adapter
//! hands API requests to the ledger under its lock and adds NanaCoin's
//! headers (`X-Nanacoin-Generation`, revalidation of public reads).
//!
//! The same adapter runs on the desktop and on both banks. The ledger mutex
//! covers API work including JSON encoding, never socket I/O: the framework
//! writes the reply after `handle` returns.
use crate::{
    api, board, cache,
    journal::{Journal, Service},
    web,
};
use miniframework::ops::{Check, Status as CheckStatus};
use miniframework::{web::Spa, Config, Reply, Request};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub type Ledger<J> = Arc<Mutex<Service<J>>>;

/// Read-only board diagnostics answered without the ledger lock. Gets the
/// path and an output buffer; returns `(status, length)`.
pub type Diagnostics = Box<dyn Fn(&str, &mut [u8]) -> (u16, usize) + Send + Sync>;

/// The diagnostics routes the firmware answers outside the ledger lock.
pub const DIAGNOSTICS: &[&str] = &["/api/v1/diag", "/api/v1/diag/static", "/api/v1/diag/events"];

pub struct Bank<J> {
    pub ledger: Ledger<J>,
    diagnostics: Option<Diagnostics>,
    email: Option<Arc<crate::email::Mailer>>,
    /// Called once when a request leaves storage failed (read-only).
    storage_failed: fn(),
}

impl<J> Bank<J> {
    pub fn new(ledger: Ledger<J>) -> Self {
        Self {
            ledger,
            diagnostics: None,
            email: None,
            storage_failed: || {},
        }
    }

    pub fn with_email(mut self, email: Arc<crate::email::Mailer>) -> Self {
        self.email = Some(email);
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: Diagnostics) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub fn on_storage_failed(mut self, report: fn()) -> Self {
        self.storage_failed = report;
        self
    }
}

impl<J: Journal + Send + 'static> miniframework::Service for Bank<J> {
    fn handle(&self, req: &Request<'_>, reply: &mut Reply<'_>) {
        if let Some(diagnostics) = &self.diagnostics {
            if req.method == "GET" && DIAGNOSTICS.contains(&req.path) {
                reply.fill("application/json", |out| diagnostics(req.path, out));
                return;
            }
        }
        let waiting = Instant::now();
        // A poisoned lock means a handler panicked mid-request; with
        // panic=abort on the boards this only happens on the desktop.
        let mut ledger = self.ledger.lock().unwrap_or_else(|e| e.into_inner());
        let lock_ms = waiting.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let failed_before = ledger.storage_failed();
        if req.path == "/api/v1/email" || req.path.starts_with("/api/v1/email/") {
            reply.fill("application/json", |out| match &self.email {
                Some(mail) => mail.handle(
                    &mut ledger,
                    req.method,
                    req.path,
                    req.header("Authorization"),
                    req.body,
                    out,
                ),
                None => api::error_response(crate::domain::Error::NotFound, out),
            });
            // A password reset commits to the journal like any other account change.
            if !failed_before && ledger.storage_failed() {
                (self.storage_failed)();
            }
            return;
        }
        reply.fill("application/json", |out| {
            api::handle_keyed_on(
                &mut ledger,
                req.method,
                req.uri,
                req.header("Authorization"),
                req.header("Idempotency-Key"),
                req.body,
                out,
                req.secure,
            )
        });
        let generation = ledger.generation();
        if !failed_before && ledger.storage_failed() {
            (self.storage_failed)();
        }
        drop(ledger);
        let app_ms = started.elapsed().as_secs_f64() * 1000.0;
        if cache::public(req.method, req.uri, reply.status()) {
            reply.revalidate(req, cache::PUBLIC);
        }
        reply.header("X-Nanacoin-Generation", generation.to_string());
        reply.header(
            "Server-Timing",
            format!("app;dur={app_ms:.3}, lock;dur={lock_ms:.3}"),
        );
    }

    fn https_required(&self) -> bool {
        // Fail closed: an unreadable setting keeps plain HTTP locked.
        self.ledger
            .lock()
            .map_or(true, |ledger| ledger.https_only())
    }

    fn checks(&self) -> Vec<Check> {
        let ledger = self.ledger.lock().unwrap_or_else(|e| e.into_inner());
        let storage = if ledger.storage_failed() {
            Check::new(
                "ledger.storage",
                CheckStatus::Fail,
                "A journal write failed: the ledger refuses changes",
            )
        } else {
            Check::new(
                "ledger.storage",
                CheckStatus::Ok,
                "Journal writes succeeding",
            )
        };
        // The journal rotates at CHECKPOINT_AFTER (half); well past it, rotation is not happening.
        let rotate = board::CHECKPOINT_AFTER * 100 / board::MAX_RECORDS;
        let journal = Check::usage(
            "ledger.journal",
            "Journal records",
            ledger.journal_records() as f64,
            board::MAX_RECORDS as f64,
            (rotate + 10) as f64,
            90.0,
        );
        vec![storage.runbook("storage"), journal.runbook("checkpoint")]
    }
}

/// The site configuration for this bank. `host` is `nanacoin.local` on a
/// board, the listening address on the desktop; `origins` is the
/// comma-separated CORS allowlist (`NANACOIN_ORIGINS`), to which the bank's
/// own HTTP and HTTPS origins are always added.
pub fn config(host: &str, origins: &str) -> Config {
    let mut config = Config::new("nanacoin", host);
    config.version = env!("CARGO_PKG_VERSION");
    config.build = env!("NANACOIN_BUILD_FINGERPRINT");
    config.body_limit = api::BODY_LIMIT;
    config.response_limit = api::RESPONSE_LIMIT;
    config.assets = web::assets();
    config.spa = Spa::Routes(web::SPA_ROUTES);
    config.ca_der = web::ca_cert();
    config.ca_filename = "NanaCoin-Home-CA.crt";
    config.trust_html = web::trust_html();
    config.cors_methods = "GET, POST, PATCH, OPTIONS";
    config.cors_headers =
        "Authorization, Content-Type, Idempotency-Key, If-None-Match, Cache-Control";
    config.expose_headers = "X-Nanacoin-Generation, Server-Timing, ETag";
    config.influx_tags = vec![("bank", board::ID.into())];
    config.origins = origins
        .split(',')
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .chain([board::HTTPS_ORIGIN, board::HTTP_ORIGIN])
        .map(String::from)
        .collect();
    config
}
