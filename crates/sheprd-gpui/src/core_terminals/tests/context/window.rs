//! The folder and Herdr variables a headless window gives a terminal it
//! starts in herdr-core, chosen from where the terminal opens.

use crate::{
    SheprdWindow,
    browser::{Location, Store, TabId},
    core_terminals::{Purpose, WORKSPACE, panes::Panes},
    sidebar::layout_tests::{fixture_window, full_draw},
};
use gpui::{Entity, TestAppContext, VisualTestContext};
use sheprd_backend::{RunState, Spawn, TerminalId, TerminalInfo};
use sheprd_client::{ConnectTarget, protocol::AgentStatus};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

fn info(name: &str, cwd: &str) -> TerminalInfo {
    TerminalInfo {
        agent: AgentStatus::Unknown,
        cwd: Some(PathBuf::from(cwd)),
        id: id(name),
        labels: Default::default(),
        name: None,
        state: RunState::Running,
        title: None,
    }
}

fn env<'a>(spawn: &'a Spawn, name: &str) -> Option<&'a str> {
    spawn
        .env
        .iter()
        .find(|(known, _)| known == name)
        .map(|(_, value)| value.as_str())
}

/// A window on a daemon whose client socket is `herdr-client.sock`, its
/// workspace w0's first tab's pane in `/src/app`.
fn window(cx: &mut TestAppContext) -> (Entity<SheprdWindow>, &mut VisualTestContext) {
    cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        view.endpoints[0] = crate::endpoint::Endpoint::new(
            crate::endpoint::LOCAL.into(),
            "Local".into(),
            ConnectTarget::Socket("/run/h/herdr-client.sock".into()),
            true,
        );
        let snapshot = Arc::make_mut(view.live.snapshot.as_mut().unwrap());
        snapshot.panes.push(
            serde_json::from_value(serde_json::json!({
                "pane_id": "p0", "workspace_id": "w0", "tab_id": "t0", "label": null,
                "cwd": "/src/app", "foreground_cwd": "/src/app/sub", "focused": true,
                "right_click_passthrough": false
            }))
            .unwrap(),
        );
        view
    })
}

fn spawn_for(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, purpose: Purpose) -> Spawn {
    let mut spawn = Spawn::default();
    view.update(cx, |view, cx| {
        view.give_core_context(&purpose, &mut spawn, cx)
    });
    spawn
}

#[gpui::test]
fn a_tab_in_a_herdr_workspace_starts_where_the_workspace_did(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let scope = view.read_with(cx, |view, _| crate::browser::scope(&view.endpoints[0]));
    let spawn = spawn_for(
        &view,
        cx,
        Purpose::Tab {
            scope,
            workspace: "w0".into(),
        },
    );
    // Where the workspace started, as Herdr's own new tabs do, not where
    // its pane's program is now.
    assert_eq!(spawn.cwd.as_deref(), Some(Path::new("/src/app")));
    assert_eq!(env(&spawn, "HERDR_WORKSPACE_ID"), Some("w0"));
    assert_eq!(env(&spawn, "HERDR_SOCKET_PATH"), Some("/run/h/herdr.sock"));
    assert_eq!(env(&spawn, "HERDR_PANE_ID"), Some(""));
}

#[gpui::test]
fn a_split_starts_where_the_pane_it_splits_is(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let tab = view.update(cx, |view, cx| {
        let scope = crate::browser::scope(&view.endpoints[0]);
        let location = Location::CoreTerminal {
            panes: Panes::single(id("t1")),
            observe: false,
        };
        let tab = Store::update(cx, |store| store.open(scope, "w0", Some(location), None)).unwrap();
        view.core_terminals.show_for_test(
            tab,
            Panes::single(id("t1")),
            false,
            vec![info("t1", "/elsewhere")],
        );
        tab
    });
    let spawn = spawn_for(
        &view,
        cx,
        Purpose::Split {
            tab,
            at: id("t1"),
            axis: crate::core_terminals::panes::Axis::Row,
        },
    );
    assert_eq!(spawn.cwd.as_deref(), Some(Path::new("/elsewhere")));
    // The split's tab is in w0, so it names w0 as a tab there would.
    assert_eq!(env(&spawn, "HERDR_WORKSPACE_ID"), Some("w0"));
}

#[gpui::test]
fn without_herdr_a_tab_starts_where_the_shown_core_pane_is(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let scope = view.update(cx, |view, cx| {
        view.live.snapshot = None;
        view.live.missing_installation = true;
        let scope = crate::browser::scope(&view.endpoints[0]);
        let location = Location::CoreTerminal {
            panes: Panes::single(id("t1")),
            observe: false,
        };
        let tab: TabId = Store::update(cx, |store| {
            store.open(scope.clone(), WORKSPACE, Some(location), None)
        })
        .unwrap();
        view.core_terminals.show_for_test(
            tab,
            Panes::single(id("t1")),
            false,
            vec![info("t1", "/work/repo")],
        );
        scope
    });
    let purpose = || Purpose::Tab {
        scope: scope.clone(),
        workspace: WORKSPACE.into(),
    };
    // Nothing shown yet: the home folder.
    let spawn = spawn_for(&view, cx, purpose());
    assert_eq!(spawn.cwd, sheprd_client::local_path::home_dir());
    assert_eq!(env(&spawn, "HERDR_WORKSPACE_ID"), Some(""));
    assert_eq!(env(&spawn, "HERDR_SOCKET_PATH"), None);
    cx.update(|window, cx| {
        let tab = view.read(cx).core_terminals.tab_of(&id("t1")).unwrap();
        view.update(cx, |view, cx| view.show_browser_tab(tab, window, cx));
        full_draw(window, cx).clear(cx);
    });
    let spawn = spawn_for(&view, cx, purpose());
    assert_eq!(spawn.cwd.as_deref(), Some(Path::new("/work/repo")));
}
