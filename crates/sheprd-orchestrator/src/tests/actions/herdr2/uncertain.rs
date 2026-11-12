use super::*;

#[test]
fn a_prompt_applied_before_its_reply_is_lost_is_never_cleaned_up_or_replayed() {
    let dir = tempfile::tempdir().unwrap();
    let site = site(dir.path(), "");
    let script = r#"#!/bin/sh
printf '%s\n' "$*" >> "$HOME/core-calls"
case "$1" in
run) echo '{"address":"t9"}' ;;
wait) echo '{"ended":true,"exit_code":null,"reason":"status","status":"idle","turn":0}' ;;
send) case "$*" in *'Fix the icons'*) printf applied > "$HOME/prompt-applied"; exit 75;; *) echo '{}';; esac ;;
*) echo '{}' ;;
esac
"#;
    test_executable::write(dir.path().join("home/.local/bin/herdr-core"), script, 0o755).unwrap();
    assert!(
        perform(
            &site,
            &Action::Dispatch(Box::new(herdr2_request())),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(dir.path().join("home/prompt-applied").exists());
    assert!(checkout(dir.path()).exists());
    let store = Store::open(&site.database).unwrap();
    assert_eq!(
        store.sessions().unwrap()[0].pane,
        Pane::Herdr2 {
            terminal: "t9".into()
        }
    );
    let calls = core_calls(dir.path());
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.contains("Fix the icons"))
            .count(),
        1
    );
    assert!(!calls.iter().any(|call| call.starts_with("kill ")));
}

/// A labelled launch whose reply is rejected or lost may still have
/// started: its checkout and launch intent stay for recovery by label, and
/// nothing is killed, relaunched, or prompted.
#[test]
fn rejected_or_lost_start_replies_retain_the_launch_intent_and_checkout() {
    for applied in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let site = site(dir.path(), "");
        let mark = if applied {
            "printf started > \"$HOME/agent-applied\""
        } else {
            ":"
        };
        let script = format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$HOME/core-calls"
case "$1" in
run) {mark}; exit 75 ;;
*) echo '{{}}' ;;
esac
"#
        );
        test_executable::write(
            dir.path().join("home/.local/bin/herdr-core"),
            &script,
            0o755,
        )
        .unwrap();
        let error = perform(
            &site,
            &Action::Dispatch(Box::new(herdr2_request())),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(matches!(error, Error::OwnedAgent(_)), "{error:?}");
        assert_eq!(dir.path().join("home/agent-applied").exists(), applied);
        assert!(checkout(dir.path()).exists());
        let store = Store::open(&site.database).unwrap();
        let run = store.runs().unwrap().remove(0);
        assert_eq!(run.state, RunState::Failed);
        assert!(run.workspace.is_some());
        assert_eq!(store.sessions().unwrap()[0].pane, Pane::Pending);
        let calls = core_calls(dir.path());
        assert_eq!(
            calls.iter().filter(|call| call.starts_with("run ")).count(),
            1
        );
        assert!(
            !calls
                .iter()
                .any(|call| call.starts_with("kill ") || call.starts_with("send "))
        );
    }
}
