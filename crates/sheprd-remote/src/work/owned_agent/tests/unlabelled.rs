use super::*;

fn select(values: serde_json::Value) -> Result<Started> {
    select_unlabelled(
        list(values),
        "/work/a",
        Owner {
            key: OWNER,
            value: "run-1",
        },
        crate::teleport::AgentKind::Claude,
        "t7",
    )
}

/// The earlier launcher's shell: no labels, the agent typed into it.
fn shell(address: &str, path: &str) -> serde_json::Value {
    serde_json::json!({"address": address, "cwd": path, "argv": ["zsh"], "state": "live", "foreground": {"name":"claude", "argv":["claude"], "pid":42}})
}

#[test]
fn an_unlabelled_saved_shell_running_its_agent_in_the_checkout_is_recovered() {
    assert_eq!(
        select(serde_json::json!([shell("t7", "/work/a")])).unwrap(),
        Started::Herdr2 {
            terminal: "t7".into()
        }
    );
}

#[test]
fn an_unlabelled_shell_is_refused_unless_every_fence_holds() {
    let mut finished = shell("t7", "/work/a");
    finished["state"] = "finished".into();
    let mut other_agent = shell("t7", "/work/a");
    other_agent["foreground"] = serde_json::json!({"name":"zsh", "argv":["zsh"], "pid":42});
    let mut no_foreground = shell("t7", "/work/a");
    no_foreground["foreground"] = serde_json::Value::Null;
    let mut labelled_other = shell("t7", "/work/a");
    labelled_other["labels"] = serde_json::json!({OWNER: "run-2"});
    for values in [
        serde_json::json!([]),
        serde_json::json!([shell("t8", "/work/a")]),
        serde_json::json!([shell("t7", "/work/b")]),
        serde_json::json!([finished]),
        serde_json::json!([other_agent]),
        serde_json::json!([no_foreground]),
        serde_json::json!([labelled_other]),
        // A labelled launch for this run exists: never bypass its checks.
        serde_json::json!([shell("t7", "/work/a"), terminal("run-1", "/work/a", "t9")]),
    ] {
        assert!(matches!(select(values), Err(Error::Uncertain)));
    }
}
