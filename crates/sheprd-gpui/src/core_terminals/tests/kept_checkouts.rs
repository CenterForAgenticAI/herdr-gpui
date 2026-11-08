//! A kept workspace whose folder is a Git checkout carries it as Herdr 1's
//! workspaces do, so the readers on `Session` treat it alike.

use crate::{
    SheprdWindow,
    browser::Location,
    core_terminals::panes::Panes,
    sidebar::layout_tests::{SidebarFixture, fixture_window},
};
use gpui::{Entity, TestAppContext, VisualTestContext, prelude::*};
use sheprd_backend::TerminalId;
use sheprd_vcs::git::Identity;
use std::path::Path;

const FOLDER: &str = "/src/app-feature";
const KEY: &str = "/src/app/.git";

fn window(cx: &mut TestAppContext) -> (Entity<SheprdWindow>, &mut VisualTestContext, String) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| fixture_window(window, cx));
        SidebarFixture(view)
    });
    let view = cx.update(|_, cx| fixture.read(cx).0.clone());
    let id = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.live.snapshot = None;
            view.live.missing_installation = true;
            view.core_terminals.checkouts.seed(
                FOLDER,
                Some(Identity {
                    branch: Some("feature".into()),
                    key: KEY.into(),
                    label: "app".into(),
                    linked: true,
                }),
            );
            let location = Location::CoreTerminal {
                panes: Panes::single(TerminalId::new("t1").unwrap()),
                observe: false,
            };
            view.open_kept_workspace(None, Path::new(FOLDER), location, window, cx)
        })
    });
    (view, cx, id.unwrap())
}

#[gpui::test]
fn a_kept_checkout_reads_like_herdr_1s(cx: &mut TestAppContext) {
    let (view, cx, id) = window(cx);
    view.read_with(cx, |view, _| {
        let sessions = view.sessions(0);
        let workspace = sessions.workspace(&id).unwrap();
        assert_eq!(workspace.branch.as_deref(), Some("feature"));
        let tree = workspace.worktree.as_ref().unwrap();
        assert_eq!((tree.key.as_str(), tree.linked), (KEY, true));
        assert!(!workspace.custom_label, "named after its folder");
        assert!(crate::worktree_identity::front_is_linked(
            &sessions,
            Some(&id)
        ));
        let inputs = view.kept_checkout_inputs();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].1.repo_key.as_deref(), Some(KEY));
        assert_eq!(inputs[0].1.branch, "feature");
        assert_eq!(
            view.git_input(),
            Some(inputs[0].1.clone()),
            "it is in front"
        );
    });
}

#[gpui::test]
fn renaming_a_kept_workspace_marks_its_name_as_chosen(cx: &mut TestAppContext) {
    let (view, cx, id) = window(cx);
    cx.update(|_, cx| view.update(cx, |view, cx| view.rename_kept_workspace(&id, "api", cx)));
    view.read_with(cx, |view, _| {
        let sessions = view.sessions(0);
        let workspace = sessions.workspace(&id).unwrap();
        assert!(workspace.custom_label);
        assert_eq!(crate::sidebar::workspace_label(workspace, true), "api");
    });
}
