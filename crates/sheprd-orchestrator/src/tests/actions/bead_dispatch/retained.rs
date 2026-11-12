use super::*;

/// Once the labelled launch is attempted the agent may be working, so the
/// claim stays; a launch whose intent could not be saved started nothing.
#[test]
fn uncertain_herdr2_agents_keep_their_beads_claim_but_a_failed_launch_releases_it() {
    for phase in ["intent", "run", "prompt"] {
        let dir = tempfile::tempdir().unwrap();
        let (site, mut request) = bead_site(dir.path(), "", UNCLAIMED);
        request.terminals = Terminals::Herdr2;
        let script = format!(
            r#"#!/bin/sh
case "$1" in
run) if [ '{phase}' = run ]; then printf applied > "$HOME/applied"; exit 75; fi
    echo '{{"address":"t9"}}' ;;
wait) echo '{{"ended":true,"exit_code":null,"reason":"status","status":"idle","turn":0}}' ;;
send) if [ '{phase}' = prompt ]; then printf applied > "$HOME/applied"; exit 75; fi
    echo '{{}}' ;;
ls) echo '{{"terminals":[]}}' ;;
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
        let _failure = (phase == "intent").then(|| crate::store::faults::session_updates(1));
        assert!(dispatch(&site, request).is_err());
        let retained = phase != "intent";
        assert_eq!(dir.path().join("home/applied").exists(), retained);
        assert_eq!(
            bd_calls(dir.path())
                .iter()
                .any(|call| call.starts_with("unclaim ")),
            !retained,
            "{phase}"
        );
        let store = Store::open(&site.database).unwrap();
        let run = store.runs().unwrap().remove(0);
        assert_eq!(run.state, RunState::Failed);
        assert_eq!(run.workspace.is_some(), retained);
        assert_eq!(!store.sessions().unwrap().is_empty(), retained);
    }
}
