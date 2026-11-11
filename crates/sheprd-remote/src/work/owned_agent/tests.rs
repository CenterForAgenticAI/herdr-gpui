use super::*;

const OWNER: &str = "sheprd.fanout.lane";

fn select(list: List, path: &str, token: &str, expected: Option<&str>) -> Result<Started> {
    super::select(
        list,
        path,
        Owner {
            key: OWNER,
            value: token,
        },
        &["claude".into()],
        expected,
        false,
        crate::teleport::AgentKind::Claude,
    )
}

fn list(values: serde_json::Value) -> List {
    serde_json::from_value(serde_json::json!({"terminals": values})).unwrap()
}

fn terminal(token: &str, path: &str, address: &str) -> serde_json::Value {
    serde_json::json!({"address": address, "cwd": path, "argv": ["claude"], "state": "live", "foreground": {"name":"claude", "argv":["claude"], "pid":42}, "labels": {OWNER: token}})
}

#[test]
fn recovery_requires_unique_owned_live_terminal_at_the_saved_checkout() {
    let owned = terminal("lane-a", "/work/a", "t7");
    assert_eq!(
        select(
            list(serde_json::json!([owned])),
            "/work/a",
            "lane-a",
            Some("t7")
        )
        .unwrap(),
        Started::Herdr2 {
            terminal: "t7".into()
        }
    );
    for values in [
        serde_json::json!([]),
        serde_json::json!([terminal("other", "/work/a", "t7")]),
        serde_json::json!([terminal("lane-a", "/work/b", "t7")]),
        serde_json::json!([terminal("lane-a", "/work/a", "t8")]),
        serde_json::json!([owned, owned]),
    ] {
        assert!(matches!(
            select(list(values), "/work/a", "lane-a", Some("t7")),
            Err(Error::Uncertain)
        ));
    }
    let mut ended = owned;
    ended["state"] = "finished".into();
    assert!(matches!(
        select(list(serde_json::json!([ended])), "/work/a", "lane-a", None),
        Err(Error::Uncertain)
    ));
}

#[test]
fn cleanup_can_inspect_a_finished_owned_terminal_but_never_an_unknown_state() {
    let owner = Owner {
        key: OWNER,
        value: "lane-a",
    };
    for (state, accepted) in [("live", true), ("finished", true), ("unexpected", false)] {
        let mut value = terminal("lane-a", "/work/a", "t7");
        value["state"] = state.into();
        assert_eq!(
            super::select(
                list(serde_json::json!([value])),
                "/work/a",
                owner,
                &["claude".into()],
                Some("t7"),
                true,
                crate::teleport::AgentKind::Claude
            )
            .is_ok(),
            accepted
        );
    }
}

#[test]
fn a_kept_shell_does_not_inherit_the_original_agents_ownership() {
    let mut shell = terminal("lane-a", "/work/a", "t7");
    shell["foreground"] = serde_json::json!({"name":"sh", "argv":["/bin/sh"], "pid":43});
    assert!(matches!(
        select(
            list(serde_json::json!([shell])),
            "/work/a",
            "lane-a",
            Some("t7")
        ),
        Err(Error::Uncertain)
    ));
}
