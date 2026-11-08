use super::*;
use crate::core_terminals::panes::{Axis, Panes};
use core::prelude::v1::test;
use sheprd_backend::TerminalId;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

/// The tabs a move rebuilt in the destination's Herdr 2 open in its new
/// workspace, on that host, with their splits.
#[gpui::test]
fn a_moves_herdr_2_tabs_open_in_the_destination_workspace(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let mut split = Panes::single(id("t5"));
    split.split(&id("t5"), Axis::Column, id("t6")).unwrap();
    let tabs = vec![split, Panes::single(id("t7"))];
    let outcome = Outcome {
        endpoint_id: "buildbox".into(),
        workspace_id: "w9".into(),
        repo_key: "repo".into(),
        branch: "feature".into(),
        warnings: Vec::new(),
        core_tabs: tabs.clone(),
    };
    view.update(cx, |view, cx| view.open_teleported_core_tabs(&outcome, cx));
    cx.update(|_, cx| {
        let opened: Vec<Panes> = cx
            .global::<Store>()
            .in_workspace(&Scope::endpoint("buildbox"), "w9")
            .filter_map(|tab| match &tab.location {
                Some(Location::CoreTerminal {
                    panes,
                    observe: false,
                }) => Some(panes.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(opened, tabs);
    });
}
