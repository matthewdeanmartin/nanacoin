//! Build-time board profile. Each NanaCoin bank runs on its own board and
//! issues its own currency; see `spec/SECOND_BANK.md`.
//!
//! The default is the ESP32-S3-N16R8 bank at `nanacoin.local`. The `board-s2`
//! feature selects the ESP32-S2 Mini bank at `nanacoin-s2.local`: the same
//! application and features with smaller retention and connection bounds for
//! 2 MiB PSRAM, 4 MiB flash and one core. The S3 never uses these smaller bounds.

#[cfg(not(feature = "board-s2"))]
mod profile {
    pub const ID: &str = "s3";
    pub const MARKER: &str = "NANACOIN-BOARD:s3:nanacoin.local;";
    pub const HOSTNAME: &str = "nanacoin";
    pub const FQDN: &str = "nanacoin.local";
    pub const HTTPS_ORIGIN: &str = "https://nanacoin.local";
    pub const HTTP_ORIGIN: &str = "http://nanacoin.local";
    pub const DEFAULT_ORIGINS: &str =
        "http://localhost:4200,http://127.0.0.1:4200,http://nanacoin.local,https://nanacoin.local";
    pub const PLATFORM: &str = "ESP32-S3 / Rust";
    pub const HISTORY: usize = 3000;
    pub const MAX_RECORDS: usize = 4096;
    pub const CORRECTIONS: usize = 4096;
    pub const AUDIT_CACHE: usize = 1024;
    pub const FULFILLMENTS: usize = 128;
    pub const ARCHIVE_SLOTS: usize = 1024;
    pub const ARCHIVE_RETAIN_PAGES: u64 = 768;
    pub const RESPONSE_LIMIT: usize = 512 * 1024;
}

#[cfg(feature = "board-s2")]
mod profile {
    pub const ID: &str = "s2";
    pub const MARKER: &str = "NANACOIN-BOARD:s2:nanacoin-s2.local;";
    pub const HOSTNAME: &str = "nanacoin-s2";
    pub const FQDN: &str = "nanacoin-s2.local";
    pub const HTTPS_ORIGIN: &str = "https://nanacoin-s2.local";
    pub const HTTP_ORIGIN: &str = "http://nanacoin-s2.local";
    pub const DEFAULT_ORIGINS: &str =
        "http://localhost:4200,http://127.0.0.1:4200,http://nanacoin-s2.local,https://nanacoin-s2.local";
    pub const PLATFORM: &str = "ESP32-S2 / Rust";
    pub const HISTORY: usize = 300;
    pub const MAX_RECORDS: usize = 512;
    pub const CORRECTIONS: usize = 512;
    pub const AUDIT_CACHE: usize = 128;
    pub const FULFILLMENTS: usize = 32;
    pub const ARCHIVE_SLOTS: usize = 128;
    pub const ARCHIVE_RETAIN_PAGES: u64 = 96;
    pub const RESPONSE_LIMIT: usize = 192 * 1024;
}

pub use profile::*;

/// Rotate while at most half of the journal is used, as the S3 always has.
pub const CHECKPOINT_AFTER: usize = MAX_RECORDS / 2;
/// Archive-capable journals rotate after this many frames or transactions.
pub const ARCHIVE_AFTER: usize = MAX_RECORDS / 4;

// Every profile, including the firmware build, must keep rotation headroom.
const _: () = assert!(ARCHIVE_AFTER < CHECKPOINT_AFTER && CHECKPOINT_AFTER < MAX_RECORDS);
const _: () = assert!(ARCHIVE_RETAIN_PAGES < ARCHIVE_SLOTS as u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_strings_agree() {
        assert_eq!(MARKER, format!("NANACOIN-BOARD:{ID}:{FQDN};"));
        assert_eq!(FQDN, format!("{HOSTNAME}.local"));
        assert_eq!(HTTPS_ORIGIN, format!("https://{FQDN}"));
        assert_eq!(HTTP_ORIGIN, format!("http://{FQDN}"));
        assert!(DEFAULT_ORIGINS.split(',').any(|o| o == HTTPS_ORIGIN));
    }
}
