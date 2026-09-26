use std::path::{Path, PathBuf};

/// Credentials the firmware embeds. The environment always wins; these files
/// are the fallback so an ordinary `make firmware` works without exporting
/// anything by hand.
///
/// Both forms are already gitignored repo-wide (`config.py` by name, `.env`
/// under this crate), so nothing read here is in version control. Searched
/// nearest-first, and the first file that defines a key supplies it.
const CREDENTIAL_FILES: [&str; 4] = [".env", "config.py", "../.env", "../nanacoin_web/config.py"];

/// Reads `KEY = "value"` / `KEY=value` pairs. Covers both the Python config
/// files and dotenv syntax: `#` comments, optional `export`, single or double
/// quotes. Values are not unescaped, matching what the Python files contain.
fn parse(text: &str, want: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != want {
            continue;
        }
        // Strip a trailing comment only when the value is unquoted; a quoted
        // password may legitimately contain '#'.
        let value = value.trim();
        let unquoted = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')));
        let value = match unquoted {
            Some(v) => v.to_string(),
            None => value.split('#').next().unwrap_or("").trim().to_string(),
        };
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

/// Maps a firmware variable to the names the config files use for it.
fn aliases(key: &str) -> &'static [&'static str] {
    match key {
        "NANACOIN_WIFI_SSID" => &["NANACOIN_WIFI_SSID", "WIFI_SSID"],
        "NANACOIN_WIFI_PASSWORD" => &["NANACOIN_WIFI_PASSWORD", "WIFI_PASSWORD"],
        "NANACOIN_ORIGINS" => &["NANACOIN_ORIGINS"],
        "NANACOIN_NTP_SERVER" => &["NANACOIN_NTP_SERVER", "NTP_SERVER"],
        _ => &[],
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=NANACOIN_RECOVER_HTTP");
    let recovery = std::env::var("NANACOIN_RECOVER_HTTP").unwrap_or_else(|_| "0".into());
    assert!(matches!(recovery.as_str(), "0" | "1"));
    println!("cargo:rustc-env=NANACOIN_RECOVER_HTTP={recovery}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_BUNDLED_WEB").is_some() {
        let assets =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join(".embuild/web");
        assert!(
            assets.join("assets.rs").exists(),
            "Run bash scripts/build-web.sh before building with bundled-web"
        );
        println!("cargo:rerun-if-changed={}", assets.display());
        std::fs::copy(
            assets.join("assets.rs"),
            PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("assets.rs"),
        )
        .unwrap();
    }
    #[cfg(feature = "esp32")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("espidf") {
        embuild::espidf::sysenv::output();
    }

    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    let sources: Vec<(PathBuf, String)> = CREDENTIAL_FILES
        .iter()
        .map(|relative| root.join(relative))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            // Rebuild when a file we actually read changes. Declaring a path
            // that does not exist would make Cargo rebuild every time.
            println!("cargo:rerun-if-changed={}", path.display());
            Some((path, text))
        })
        .collect();

    for key in [
        "NANACOIN_WIFI_SSID",
        "NANACOIN_WIFI_PASSWORD",
        "NANACOIN_ORIGINS",
        "NANACOIN_NTP_SERVER",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        if std::env::var_os(key).is_some() {
            continue;
        }
        let found = sources.iter().find_map(|(path, text)| {
            aliases(key)
                .iter()
                .find_map(|name| parse(text, name))
                .map(|value| (path.as_path(), value))
        });
        if let Some((path, value)) = found {
            // The value itself is never printed: build logs are shared more
            // freely than the files they came from.
            let shown: &Path = path.strip_prefix(&root).unwrap_or(path);
            println!("cargo:warning={key} taken from {}", shown.display());
            println!("cargo:rustc-env={key}={value}");
        }
    }

    println!("cargo:rerun-if-changed=certs/nanacoin-ca-signed.crt");
    println!("cargo:rerun-if-changed=certs/nanacoin-ca-signed.key");
}
