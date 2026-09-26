mod common;
use nanacoin::{api, domain::*, journal::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Default)]
struct Audited {
    frames: Rc<RefCell<Vec<[u8; FRAME_SIZE]>>>,
    writes: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}
impl Journal for Audited {
    fn read(&mut self, i: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if let Some(value) = self.frames.borrow().get(i) {
            *frame = *value;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn append(&mut self, _: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        self.writes.set(self.writes.get() + 1);
        if self.fail.get() {
            return Err(Error::Storage);
        }
        self.frames.borrow_mut().push(*frame);
        Ok(())
    }
}
fn get(service: &mut Service<Audited>, path: &str) -> serde_json::Value {
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let (status, len) = api::handle(service, "GET", path, "", b"", &mut out);
    assert_eq!(status, 200);
    serde_json::from_slice(&out[..len]).unwrap()
}

#[test]
fn public_inventory_configuration_and_benchmarks_never_write() {
    let store = Audited::default();
    let mut service = Service::open(store.clone()).unwrap();
    common::provision(&mut service);
    service
        .execute(
            MemberId(1),
            2,
            Command::Issue {
                to: MemberId(1),
                amount: 12345,
                memo: Memo::try_from("fixture").unwrap(),
            },
        )
        .unwrap();
    let token = common::login(&mut service);
    let before = format!("{:?}", service.state());
    let writes = store.writes.get();
    let frames = store.frames.borrow().clone();
    let configuration = get(&mut service, "/api/v1/configuration");
    assert_eq!(configuration["household_name"], "Our house");
    assert_eq!(configuration["decimals"], 4);
    assert_eq!(configuration["minor_units_per_coin"], 10000);
    let inventory = get(&mut service, "/api/v1/diag/database");
    assert_eq!(inventory["invariants_ok"], true);
    let rows = inventory["collections"].as_array().unwrap();
    assert_eq!(rows.len(), 16);
    assert!(rows
        .iter()
        .any(|r| r["name"] == "Login sessions" && r["used"] == 1 && r["active"] == 1));
    assert!(rows
        .iter()
        .any(|r| r["name"] == "Physical fulfillment and recent updates"));
    assert!(rows.iter().any(|r| r["name"] == "Idempotency receipts"));
    let benchmark = get(&mut service, "/api/v1/diag/database/benchmark");
    assert_eq!(benchmark["read_only"], true);
    assert_eq!(benchmark["queries"].as_array().unwrap().len(), 4);
    assert!(benchmark["queries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|q| q["runs"].as_u64().unwrap() <= 3 && q["error"].is_null()));
    assert!(before == format!("{:?}", service.state()));
    assert_eq!(writes, store.writes.get());
    assert!(*store.frames.borrow() == frames);
    assert_eq!(
        inventory["collections"],
        get(&mut service, "/api/v1/diag/database")["collections"]
    );
    let serialized = serde_json::to_string(&inventory).unwrap();
    assert!(
        !serialized.contains("1234")
            && !serialized.contains("password")
            && !serialized.contains("username")
            && !serialized.contains(&token)
    );
    let mut out = vec![0; api::RESPONSE_LIMIT];
    let (status, _) = api::handle(
        &mut service,
        "PATCH",
        "/api/v1/configuration",
        &token,
        br#"{"decimals":2}"#,
        &mut out,
    );
    assert_ne!(status, 200);
    assert_eq!(service.state().decimals, 4);
    assert_eq!(writes, store.writes.get());
}

#[test]
fn empty_database_and_storage_failure_still_have_readable_diagnostics() {
    let store = Audited::default();
    let mut service = Service::open(store.clone()).unwrap();
    let empty = get(&mut service, "/api/v1/diag/database/benchmark");
    assert!(empty["queries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|q| q["error"] == "not_found"));
    assert_eq!(store.writes.get(), 0);
    common::provision(&mut service);
    store.fail.set(true);
    assert_eq!(
        service.execute(
            MemberId(1),
            2,
            Command::Issue {
                to: MemberId(1),
                amount: 1,
                memo: Memo::new()
            }
        ),
        Err(Error::Storage)
    );
    let writes = store.writes.get();
    assert_eq!(
        get(&mut service, "/api/v1/diag/database")["storage_failed"],
        true
    );
    assert_eq!(
        get(&mut service, "/api/v1/configuration")["household_name"],
        "Our house"
    );
    assert_eq!(
        get(&mut service, "/api/v1/diag/database/benchmark")["read_only"],
        true
    );
    assert_eq!(store.writes.get(), writes);
}
