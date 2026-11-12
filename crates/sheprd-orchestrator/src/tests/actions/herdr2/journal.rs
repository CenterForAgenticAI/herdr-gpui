//! A dispatch journals its pre-start recovery identity before Git or the
//! core is touched, so a failed first session write leaves nothing to undo.
use super::*;
use crate::store::faults;

#[test]
fn a_failed_prestart_record_creates_no_checkout_and_starts_no_agent() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    fake_core(dir.path(), "idle");
    let _failure = faults::session_updates(1);
    let error = perform(
        &site,
        &Action::Dispatch(Box::new(herdr2_request())),
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(
        matches!(&error, Error::Database(rusqlite::Error::SqliteFailure(code, _)) if code.code == rusqlite::ErrorCode::DatabaseBusy)
    );
    assert!(std::error::Error::source(&error).is_some());
    let store = Store::open(&site.database).unwrap();
    let run = store.runs().unwrap().remove(0);
    assert_eq!(run.state, RunState::Failed);
    assert!(run.workspace.is_none());
    assert!(!checkout(dir.path()).exists());
    assert!(
        !core_calls(dir.path())
            .iter()
            .any(|call| call.starts_with("run ") || call.contains("Fix the icons"))
    );
}

#[test]
fn a_failed_prestart_record_is_written_once_and_keeps_its_database_cause() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    fake_core(dir.path(), "idle");
    let _failure = faults::session_updates(3);
    let error = perform(
        &site,
        &Action::Dispatch(Box::new(herdr2_request())),
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(
        matches!(error, Error::Database(rusqlite::Error::SqliteFailure(_, Some(message))) if message == "injected session write failure 3")
    );
    assert_eq!(faults::remaining(), 2, "no session write is retried");
    assert!(!checkout(dir.path()).exists());
}
