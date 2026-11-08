//! A Herdr 2 terminal a phone started for one of this computer's
//! workspaces lands in a tab of that workspace, once.

use crate::{
    SheprdWindow,
    browser::{Location, Store},
    sidebar::layout_tests::{SidebarFixture, fixture_window, snapshot},
};
use gpui::{Entity, TestAppContext, VisualTestContext, prelude::*};
use sheprd_backend::{AgentStatus, RunState, TerminalId, TerminalInfo};
use std::sync::Arc;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

/// A terminal the core lists, labelled for `workspace` when given.
fn listed(name: &str, workspace: Option<&str>) -> TerminalInfo {
    TerminalInfo {
        agent: AgentStatus::Unknown,
        cwd: None,
        id: id(name),
        labels: workspace
            .map(|workspace| {
                [(
                    sheprd_session::WORKSPACE_LABEL.to_owned(),
                    workspace.to_owned(),
                )]
                .into()
            })
            .unwrap_or_default(),
        name: None,
        state: RunState::Running,
        title: None,
    }
}

/// A window on a Herdr 1 session with workspaces `w0` and `w1`.
fn window(cx: &mut TestAppContext) -> (Entity<SheprdWindow>, &mut VisualTestContext) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| fixture_window(window, cx));
        SidebarFixture(view)
    });
    let view = cx.update(|_, cx| fixture.read(cx).0.clone());
    cx.update(|_, cx| {
        Store::update(cx, |_| {});
        view.update(cx, |view, _| {
            view.live.snapshot = Some(Arc::new(snapshot(2)));
        })
    });
    (view, cx)
}

/// The workspaces of the tabs holding `terminal`.
fn tabs_of(cx: &mut VisualTestContext, terminal: &str) -> Vec<String> {
    cx.update(|_, cx| {
        cx.global::<Store>()
            .tabs()
            .filter(|tab| match &tab.location {
                Some(Location::CoreTerminal { panes, .. }) => panes.contains(&id(terminal)),
                _ => false,
            })
            .map(|tab| tab.workspace_id.clone())
            .collect()
    })
}

#[gpui::test]
fn a_terminal_labelled_for_a_workspace_gets_a_tab_there_once(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.update(|_, cx| {
        // Another host's tab holds its own `t5`, another terminal.
        Store::update(cx, |store| {
            let location = Location::CoreTerminal {
                panes: sheprd_session::panes::Panes::single(id("t5")),
                observe: false,
            };
            store.open(
                crate::browser::Scope::endpoint("box"),
                "w9",
                Some(location),
                None,
            )
        });
        view.update(cx, |view, cx| {
            let infos = &mut view.core_terminals.infos;
            infos.insert(listed("t5", Some("w1")));
            // Not labelled, or for a workspace this computer does not have.
            infos.insert(listed("t6", None));
            infos.insert(listed("t7", Some("gone")));
            view.adopt_labelled_terminals(true, cx);
        })
    });
    assert_eq!(tabs_of(cx, "t5"), ["w9", "w1"]);
    assert!(tabs_of(cx, "t6").is_empty());
    assert!(tabs_of(cx, "t7").is_empty());
    // Once, even after the user closes its tab.
    cx.update(|_, cx| {
        Store::update(cx, |store| {
            let tabs: Vec<_> = store.tabs().map(|tab| tab.id).collect();
            for tab in tabs {
                store.close(tab);
            }
        });
        view.update(cx, |view, cx| view.adopt_labelled_terminals(true, cx));
    });
    assert!(tabs_of(cx, "t5").is_empty());
}

#[gpui::test]
fn a_phones_new_workspace_is_kept_in_its_folder_with_the_terminal_in_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let new = |name: &str| {
        let mut info = listed(name, None);
        info.labels.insert(
            sheprd_session::NEW_WORKSPACE_LABEL.to_owned(),
            "/code/api".to_owned(),
        );
        info
    };
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.core_terminals.infos.insert(new("t5"));
            view.adopt_labelled_terminals(true, cx);
            // A second one for the same folder joins that workspace.
            view.core_terminals.infos.insert(new("t6"));
            view.adopt_labelled_terminals(true, cx);
        })
    });
    let kept = view.read_with(cx, |view, _| {
        view.kept_workspace_at(std::path::Path::new("/code/api"))
    });
    let kept = kept.expect("a workspace kept in the folder");
    assert_eq!(tabs_of(cx, "t5"), [kept.clone()]);
    assert_eq!(tabs_of(cx, "t6"), [kept]);
}
