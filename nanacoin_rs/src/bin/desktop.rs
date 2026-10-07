//! The desktop server: the same framework connection loop the boards run,
//! over loopback HTTP, with the journal in a file.
use miniframework::desktop::{serve, DesktopPlatform};
use miniframework::Site;
use nanacoin::{
    api,
    journal::{file::FileJournal, Service},
    server::{self, Bank},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::args().any(|arg| arg == "--bank-engine") {
        println!(
            "{}",
            if cfg!(feature = "cobol-core") {
                "cobol"
            } else {
                "rust"
            }
        );
        return Ok(());
    }
    #[cfg(feature = "cobol-core")]
    nanacoin::cobol::initialize().map_err(|e| format!("COBOL banking kernel: {e}"))?;
    let path = std::env::var("NANACOIN_JOURNAL").unwrap_or_else(|_| "nanacoin.journal".into());
    let journal = FileJournal::open(path)?;
    #[cfg(feature = "conformance-clock")]
    let clock = fixture_clock::configure()?;
    #[cfg(not(feature = "conformance-clock"))]
    let clock = {
        if std::env::args().any(|arg| arg == "--conformance-clock-file") {
            return Err("This build does not enable conformance-clock".into());
        }
        None
    };
    let mut service = match clock {
        Some(clock) => Service::open_with_clock(journal, clock),
        None => Service::open(journal),
    }
    .map_err(|e| format!("journal: {e:?}"))?;
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
    let origins = format!(
        "{},http://{address},http://localhost:{port}",
        std::env::var("NANACOIN_ORIGINS").unwrap_or_else(|_| api::DEFAULT_ORIGINS.into())
    );
    miniframework::desktop::init_logging();
    println!("NanaCoin: http://{address} (loopback development server)");
    let ledger = Arc::new(Mutex::new(service));
    let site = Site::new(
        server::config(&address, &origins),
        Bank::new(Arc::clone(&ledger)),
        DesktopPlatform,
    );
    let mut screen = nanacoin::screen::Worker::spawn()?;
    let mut last = Instant::now() - Duration::from_secs(1);
    serve(&site, &address, || {
        // Scheduled payments and screen delivery, once a second.
        if last.elapsed() < Duration::from_secs(1) {
            return;
        }
        last = Instant::now();
        let mut service = ledger.lock().unwrap_or_else(|e| e.into_inner());
        screen.pump(&mut service);
        if let Err(error) = service.tick() {
            eprintln!("scheduled payments: {error:?}");
        }
    })?;
    Ok(())
}

#[cfg(feature = "conformance-clock")]
mod fixture_clock {
    type Clock = fn() -> u64;
    type Failure = Box<dyn std::error::Error + Send + Sync>;
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    };
    static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    static LAST: AtomicU64 = AtomicU64::new(0);

    pub fn configure() -> Result<Option<Clock>, Failure> {
        let mut args = std::env::args();
        if !args.any(|arg| arg == "--conformance-clock-file") {
            return Ok(None);
        }
        let path = args
            .next()
            .ok_or("--conformance-clock-file requires a path")?;
        let initial = std::fs::read_to_string(&path)?.trim().parse::<u64>()?;
        if !(nanacoin::offers::MIN_CLOCK..=nanacoin::domain::MAX_SEQUENCE).contains(&initial) {
            return Err("Conformance clock is outside the valid bank clock range".into());
        }
        PATH.set(path.into())
            .map_err(|_| "Clock already configured")?;
        LAST.store(initial, Ordering::Relaxed);
        Ok(Some(now))
    }

    fn now() -> u64 {
        // Atomic rename by the launcher avoids partial inputs. An invalid read
        // or backwards input holds the last valid instant; no public clock route.
        let candidate = std::fs::read_to_string(PATH.get().expect("configured clock"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|v| *v <= nanacoin::domain::MAX_SEQUENCE)
            .unwrap_or(0);
        LAST.fetch_max(candidate, Ordering::Relaxed).max(candidate)
    }
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
