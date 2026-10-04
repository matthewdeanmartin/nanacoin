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
