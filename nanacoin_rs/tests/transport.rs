mod common;
use nanacoin::{
    api,
    domain::MemberId,
    journal::{file::FileJournal, Journal, Service},
};

#[test]
fn policy_requires_tls_and_nana_revokes_sessions_and_survives_reset_and_restart() {
    let dir = std::env::temp_dir().join(format!("nanacoin-transport-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.journal");
    let mut s = Service::open(FileJournal::open(&path).unwrap()).unwrap();
    common::provision(&mut s);
    let auth = common::login(&mut s);
    assert_eq!(
        common::call(
            &mut s,
            "/api/v1/users",
            &auth,
            serde_json::json!({
                "username": "alice", "display_name": "Alice", "password": "1234", "grant": false
            })
        )
        .0,
        201
    );
    let member_auth = common::login_as(&mut s, "alice", "1234");
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let body = br#"{"confirmation":"REQUIRE HTTPS"}"#;
    assert_eq!(
        api::handle_keyed_on(
            &mut s,
            "POST",
            "/api/v1/admin/transport",
            &member_auth,
            "",
            body,
            &mut out,
            true
        )
        .0,
        403
    );
    assert!(!s.https_only());
    assert_eq!(
        api::handle_keyed_on(
            &mut s,
            "POST",
            "/api/v1/admin/transport",
            &auth,
            "",
            body,
            &mut out,
            false
        )
        .0,
        403
    );
    assert!(!s.https_only());
    assert_eq!(
        api::handle_keyed_on(
            &mut s,
            "POST",
            "/api/v1/admin/transport",
            "",
            "",
            body,
            &mut out,
            true
        )
        .0,
        401
    );
    assert!(s.require_https(MemberId(99)).is_err());
    assert!(!s.https_only());
    assert_eq!(
        api::handle_keyed_on(
            &mut s,
            "POST",
            "/api/v1/admin/transport",
            &auth,
            "",
            br#"{"confirmation":"NO"}"#,
            &mut out,
            true
        )
        .0,
        400
    );
    assert_eq!(
        api::handle_keyed_on(
            &mut s,
            "POST",
            "/api/v1/admin/transport",
            &auth,
            "",
            body,
            &mut out,
            true
        )
        .0,
        200
    );
    assert!(s.https_only());
    for endpoint in [
        "/api/v1/status",
        "/api/v1/transport",
        "/api/v1/auth/authorize",
    ] {
        assert_eq!(
            api::handle_keyed_on(&mut s, "GET", endpoint, &auth, "", b"", &mut out, false).0,
            403
        );
    }
    assert_eq!(
        api::handle_keyed_on(&mut s, "GET", "/api/v1/status", "", "", b"", &mut out, true).0,
        200
    );
    assert_eq!(
        api::handle_keyed_on(
            &mut s,
            "POST",
            "/api/v1/admin/transport",
            &auth,
            "",
            body,
            &mut out,
            true
        )
        .0,
        401
    );
    s.reset_economy(MemberId(1)).unwrap();
    assert!(s.https_only());
    drop(s);
    let mut journal = FileJournal::open(&path).unwrap();
    assert!(journal.https_only());
    // Physical recovery changes the policy only, not the journal generation.
    let generation = journal.generation();
    journal.set_https_only(false).unwrap();
    assert_eq!(generation, journal.generation());
    drop(journal);
    assert!(!FileJournal::open(&path).unwrap().https_only());
    std::fs::write(dir.join("test.journal.transport"), [9]).unwrap();
    assert!(FileJournal::open(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
