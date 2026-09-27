mod common;
use nanacoin::{api, auth::PasswordVerifier, board_status::LightConfig, domain::*, journal::*};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Default)]
struct Memory {
    frames: Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>,
    light: Rc<RefCell<LightConfig>>,
    fail: Rc<Cell<bool>>,
}
impl Journal for Memory {
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(stored) = self.frames.borrow().get(index) {
            *frame = *stored;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, _: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        self.frames.borrow_mut().push(*frame);
        Ok(())
    }
    fn light_config(&self) -> Result<LightConfig, Error> {
        Ok(self.light.borrow().clone())
    }
    fn set_light_config(&mut self, config: &LightConfig) -> Result<(), Error> {
        if self.fail.get() {
            return Err(Error::Storage);
        }
        *self.light.borrow_mut() = config.clone();
        Ok(())
    }
}
#[test]
fn admin_only_atomic_settings_survive_restart_without_ledger_changes() {
    let disk = Memory::default();
    let mut s = Service::open(disk.clone()).unwrap();
    common::provision(&mut s);
    s.execute(
        MemberId(1),
        2,
        Command::CreateMember {
            username: "alice".try_into().unwrap(),
            display_name: "Alice".try_into().unwrap(),
            password: PasswordVerifier::hash("5678").unwrap(),
            role: Role::User,
            grant: 0,
            mastodon_id: MastodonId::new(),
        },
    )
    .unwrap();
    let nana = common::login(&mut s);
    let alice = common::login_as(&mut s, "alice", "5678");
    let path = "/api/v1/admin/light";
    let body = json!({"phrases": ["  First!  ", "Second?", "Third."]});
    let mut out = vec![0; api::RESPONSE_LIMIT];
    assert_eq!(api::handle(&mut s, "GET", path, "", &[], &mut out).0, 401);
    assert_eq!(
        api::handle(&mut s, "GET", path, &alice, &[], &mut out).0,
        403
    );
    assert_eq!(common::call(&mut s, path, &alice, body.clone()).0, 403);
    let before = (s.state().sequence, s.state().issuance_balance);
    let records = s.journal_records();
    assert_eq!(common::call(&mut s, path, &nana, body).0, 200);
    let saved = s.light_config.clone();
    assert_eq!(saved.phrases[0], "First!");
    for phrases in [
        json!(["one"]),
        json!(["a", "b", "c", "d"]),
        json!(["a", "b", ""]),
        json!(["a", "#", "c"]),
        json!(["A".repeat(81), "b", "c"]),
    ] {
        assert!(common::call(&mut s, path, &nana, json!({"phrases": phrases})).0 >= 400);
        assert_eq!(s.light_config, saved);
    }
    disk.fail.set(true);
    assert_eq!(
        common::call(
            &mut s,
            path,
            &nana,
            json!({"phrases": ["new", "two", "three"]})
        )
        .0,
        503
    );
    assert_eq!(s.light_config, saved);
    assert_eq!(s.journal_records(), records);
    assert_eq!(s.state().sequence, before.0);
    assert_eq!(s.state().issuance_balance, before.1);
    let reopened = Service::open(disk).unwrap();
    assert_eq!(reopened.light_config, saved);
}

#[cfg(feature = "desktop")]
#[test]
fn desktop_settings_use_a_durable_companion_file() {
    use nanacoin::journal::file::FileJournal;
    let dir = std::env::temp_dir().join(format!(
        "nanacoin-light-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("ledger");
    let mut journal = FileJournal::open(&path).unwrap();
    assert_eq!(journal.light_config().unwrap(), LightConfig::default());
    let config = LightConfig {
        phrases: ["One", "Two", "Three"].map(|p| p.try_into().unwrap()),
    };
    journal.set_light_config(&config).unwrap();
    drop(journal);
    assert_eq!(
        FileJournal::open(&path).unwrap().light_config().unwrap(),
        config
    );
    std::fs::remove_dir_all(dir).unwrap();
}
