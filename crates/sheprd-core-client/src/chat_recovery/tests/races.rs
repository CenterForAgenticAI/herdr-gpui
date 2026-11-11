//! A replacement's two durable steps, claim and child registration, lose to
//! any concurrent writer and to a close, and never assume a reply arrived.
use super::*;

fn conflict(peer: &mut crate::tests::MockCore, request: &serde_json::Value) {
    let message = json!({"jsonrpc": "2.0", "id": request["id"],
        "error": {"code": -32000, "message": "version changed", "data": {"kind": "conflict"}}});
    peer.send(
        0,
        crate::frame::Kind::Data,
        &serde_json::to_vec(&message).unwrap(),
    );
}

fn launching() -> Record {
    let mut record = record();
    record.state = State::Launching;
    record.generation = 1;
    record
}

fn pending() -> serde_json::Value {
    let mut replacement = terminal("t2");
    replacement["name"] = json!(launch_name(&ID.parse().unwrap(), 1));
    replacement["labels"] = json!({PENDING_LABEL: ID});
    replacement
}

#[test]
fn a_claim_that_loses_its_cas_race_never_publishes_a_mapping() {
    let (core, mut peer) = connected();
    let worker =
        std::thread::spawn(move || claim(&core, &ID.parse().unwrap(), "t2", 1, |_| Ok(true)));
    let get = peer.request();
    peer.answer(&get, json!({"value": launching(), "version": 4}));
    let list = peer.request();
    peer.answer(&list, json!({"terminals": [terminal("t1"), pending()]}));
    let set = peer.request();
    assert_eq!(set["params"]["version"], 4);
    conflict(&mut peer, &set);
    assert!(matches!(
        worker.join().unwrap(),
        Err(crate::Error::Refused {
            method: crate::Method::DocSet,
            ..
        })
    ));
}

#[test]
fn a_claim_whose_reply_is_lost_reports_failure_rather_than_success() {
    let (core, mut peer) = connected();
    let worker =
        std::thread::spawn(move || claim(&core, &ID.parse().unwrap(), "t2", 1, |_| Ok(true)));
    let get = peer.request();
    peer.answer(&get, json!({"value": launching(), "version": 4}));
    let list = peer.request();
    peer.answer(&list, json!({"terminals": [terminal("t1"), pending()]}));
    let _set = peer.request();
    drop(peer);
    assert!(worker.join().unwrap().is_err());
}

#[test]
fn a_claim_is_refused_once_the_original_terminal_is_gone() {
    let (core, mut peer) = connected();
    let worker =
        std::thread::spawn(move || claim(&core, &ID.parse().unwrap(), "t2", 1, |_| Ok(true)));
    let get = peer.request();
    peer.answer(&get, json!({"value": launching(), "version": 4}));
    let list = peer.request();
    peer.answer(&list, json!({"terminals": [pending()]}));
    assert!(matches!(
        worker.join().unwrap(),
        Err(crate::Error::ChatRecovery(Error::Moved))
    ));
}

#[test]
fn a_probe_failure_refuses_the_claim_instead_of_assuming_the_child_gone() {
    let (core, mut peer) = connected();
    let worker = std::thread::spawn(move || {
        claim(&core, &ID.parse().unwrap(), "t2", 1, |_| {
            Err(crate::sessions::Error::Identity.into())
        })
    });
    let get = peer.request();
    peer.answer(&get, json!({"value": launching(), "version": 4}));
    assert!(matches!(
        worker.join().unwrap(),
        Err(crate::Error::Sessions(crate::sessions::Error::Identity))
    ));
}

#[test]
fn a_child_registration_that_loses_its_cas_race_stays_starting() {
    let (core, mut peer) = connected();
    let mut child = record().child.unwrap();
    child.terminal = "t2".into();
    child.pid = 60;
    let worker = std::thread::spawn(move || child_started(&core, &ID.parse().unwrap(), 1, child));
    let mut record = launching();
    record.state = State::Starting;
    record.terminal = "t2".into();
    let get = peer.request();
    peer.answer(&get, json!({"value": record, "version": 5}));
    let set = peer.request();
    assert_eq!(set["params"]["version"], 5);
    assert_eq!(set["params"]["value"]["state"], "live");
    assert_eq!(set["params"]["value"]["child"]["pid"], 60);
    conflict(&mut peer, &set);
    assert!(matches!(
        worker.join().unwrap(),
        Err(crate::Error::Refused {
            method: crate::Method::DocSet,
            ..
        })
    ));
}

#[test]
fn a_child_registration_for_another_generation_or_terminal_is_refused() {
    for (generation, terminal) in [(2, "t2"), (1, "t3")] {
        let (core, mut peer) = connected();
        let mut child = record().child.unwrap();
        child.terminal = terminal.into();
        let worker = std::thread::spawn(move || {
            child_started(&core, &ID.parse().unwrap(), generation, child)
        });
        let mut record = launching();
        record.state = State::Starting;
        record.terminal = "t2".into();
        let get = peer.request();
        peer.answer(&get, json!({"value": record, "version": 5}));
        assert!(matches!(
            worker.join().unwrap(),
            Err(crate::Error::ChatRecovery(Error::Moved))
        ));
    }
}
