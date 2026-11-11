//! A launch whose reply was lost keeps its session Pending; Stop and Send
//! still reach the labelled agent it started, without starting another.
use super::*;

#[test]
fn a_pending_run_recovers_its_labelled_agent_for_send_and_stop() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    fake_core(dir.path(), "idle");
    let cancelled = AtomicBool::new(false);
    perform(
        &site,
        &Action::Dispatch(Box::new(herdr2_request())),
        &cancelled,
    )
    .unwrap();
    let mut store = Store::open(&site.database).unwrap();
    let run = store.runs().unwrap().remove(0);
    let mut session = store.sessions().unwrap().remove(0);
    // As if `core_cli run` started the agent but its reply never arrived.
    session.pane = Pane::Pending;
    store.update_run(&run, Some(&session)).unwrap();
    let before = core_calls(dir.path()).len();

    let send = Action::Send {
        run: run.id.clone(),
        text: "go on".into(),
    };
    perform(&site, &send, &cancelled).unwrap();
    assert_eq!(
        Store::open(&site.database).unwrap().sessions().unwrap()[0].pane,
        Pane::Herdr2 {
            terminal: "t9".into()
        }
    );
    perform(&site, &Action::Stop { run: run.id }, &cancelled).unwrap();
    assert_eq!(
        core_calls(dir.path())[before..]
            .iter()
            .filter(|call| !call.starts_with("ls "))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["send --json -- t9 go on", "keys --json -- t9 esc ctrl+c"]
    );
}

#[test]
fn a_pending_run_without_its_labelled_agent_is_not_typed_into() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    fake_core(dir.path(), "idle");
    let cancelled = AtomicBool::new(false);
    perform(
        &site,
        &Action::Dispatch(Box::new(herdr2_request())),
        &cancelled,
    )
    .unwrap();
    let mut store = Store::open(&site.database).unwrap();
    let run = store.runs().unwrap().remove(0);
    let mut session = store.sessions().unwrap().remove(0);
    session.pane = Pane::Pending;
    store.update_run(&run, Some(&session)).unwrap();
    std::fs::write(
        dir.path().join("home/core-inventory"),
        r#"{"terminals":[]}"#,
    )
    .unwrap();
    let before = core_calls(dir.path()).len();

    let error = perform(&site, &Action::Stop { run: run.id }, &cancelled).unwrap_err();
    assert!(
        matches!(
            error,
            Error::OwnedAgent(sheprd_remote::work::owned_agent::Error::Uncertain)
        ),
        "{error:?}"
    );
    assert!(
        core_calls(dir.path())[before..]
            .iter()
            .all(|call| call.starts_with("ls "))
    );
    assert_eq!(
        Store::open(&site.database).unwrap().sessions().unwrap()[0].pane,
        Pane::Pending
    );
}
