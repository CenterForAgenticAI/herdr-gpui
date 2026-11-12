//! Runs saved before launches were labelled: the agent was typed into an
//! unlabelled shell, which Stop and Send must still reach.
use super::*;

/// Replaces the fake core's inventory with the earlier launcher's shell.
fn unlabelled_shell(dir: &Path, foreground: &str) {
    let terminal = serde_json::json!({
        "address": "t9",
        "cwd": checkout(dir),
        "argv": ["zsh"],
        "state": "live",
        "foreground": {"name": foreground, "argv": [foreground], "pid": 4242},
    });
    std::fs::write(
        dir.join("home/core-inventory"),
        serde_json::json!({"terminals": [terminal]}).to_string(),
    )
    .unwrap();
}

fn dispatched(dir: &Path) -> (Site, String) {
    let site = site(dir, "");
    fake_core(dir, "idle");
    perform(
        &site,
        &Action::Dispatch(Box::new(herdr2_request())),
        &AtomicBool::new(false),
    )
    .unwrap();
    let id = Store::open(&site.database)
        .unwrap()
        .runs()
        .unwrap()
        .remove(0)
        .id;
    (site, id)
}

#[test]
fn a_run_in_an_unlabelled_shell_is_still_messaged_and_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let (site, id) = dispatched(dir.path());
    unlabelled_shell(dir.path(), "claude");
    let before = core_calls(dir.path()).len();
    let cancelled = AtomicBool::new(false);
    let send = Action::Send {
        run: id.clone(),
        text: "go on".into(),
    };
    perform(&site, &send, &cancelled).unwrap();
    perform(&site, &Action::Stop { run: id }, &cancelled).unwrap();
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
fn an_unlabelled_shell_without_its_agent_is_not_typed_into() {
    let dir = tempfile::tempdir().unwrap();
    let (site, id) = dispatched(dir.path());
    unlabelled_shell(dir.path(), "zsh");
    let before = core_calls(dir.path()).len();
    let send = Action::Send {
        run: id,
        text: "go on".into(),
    };
    let error = perform(&site, &send, &AtomicBool::new(false)).unwrap_err();
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
}

#[test]
fn a_run_in_an_unlabelled_shell_is_removed_with_its_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let (site, id) = dispatched(dir.path());
    unlabelled_shell(dir.path(), "claude");
    perform(&site, &Action::Remove { run: id }, &AtomicBool::new(false)).unwrap();
    let calls = core_calls(dir.path());
    assert!(calls.iter().any(|call| call == "kill --json -- t9"));
    assert!(!checkout(dir.path()).exists());
    assert!(
        Store::open(&site.database)
            .unwrap()
            .runs()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_unlabelled_shell_without_its_agent_keeps_its_checkout_on_remove() {
    let dir = tempfile::tempdir().unwrap();
    let (site, id) = dispatched(dir.path());
    unlabelled_shell(dir.path(), "zsh");
    let error = perform(&site, &Action::Remove { run: id }, &AtomicBool::new(false)).unwrap_err();
    assert!(
        matches!(
            error,
            Error::OwnedAgent(sheprd_remote::work::owned_agent::Error::Uncertain)
        ),
        "{error:?}"
    );
    assert!(
        !core_calls(dir.path())
            .iter()
            .any(|call| call.starts_with("kill "))
    );
    assert!(checkout(dir.path()).join(".git").exists());
}
