use super::*;
use gpui::TestAppContext;

#[gpui::test]
fn the_state_lists_each_workspace_with_what_its_row_shows(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let state = view.update(cx, |view, cx| {
        view.live.snapshot = Some(Arc::new(crate::sidebar::layout_tests::snapshot(3)));
        view.hosted_state(cx)
    });
    let workspaces = state.main["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 3);
    assert!(workspaces[0]["id"].is_string());
    assert!(workspaces[0]["label"].is_string());
    assert!(
        state.main["panels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|panel| panel["id"] == "workspaces")
    );
}

#[gpui::test]
fn an_unknown_command_or_panel_changes_nothing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update_in(cx, |view, window, cx| {
        let before = view.hosted_state(cx);
        view.hosted_action(Action::Command("no-such-command".into()), window, cx);
        view.hosted_action(Action::Panel("no-such-panel".into()), window, cx);
        assert_eq!(view.hosted_state(cx), before);
    });
}

#[test]
fn every_panel_has_its_own_id() {
    let mut ids: Vec<_> = ActivityPanel::ALL.iter().copied().map(panel_id).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count);
}

#[gpui::test]
fn a_stamp_changes_with_the_snapshot_and_not_with_a_redraw(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update(cx, |view, cx| {
        view.live.snapshot = Some(Arc::new(crate::sidebar::layout_tests::snapshot(2)));
        let before = view.hosted_stamp();
        cx.notify();
        assert!(view.hosted_stamp() == before);
        view.live.snapshot = Some(Arc::new(crate::sidebar::layout_tests::snapshot(2)));
        assert!(view.hosted_stamp() != before);
    });
}

#[gpui::test]
fn a_stamp_changes_when_the_sidebar_hides(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update(cx, |view, _| {
        let before = view.hosted_stamp();
        view.sidebar_visible = !view.sidebar_visible;
        assert!(view.hosted_stamp() != before);
    });
}

#[test]
fn dark_backgrounds_are_told_from_light_ones() {
    assert!(is_dark(0x101419));
    assert!(!is_dark(0xf6f6f8));
}

#[gpui::test]
fn linked_worktrees_follow_their_parent_as_children(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let state = view.update(cx, |view, cx| {
        let mut snapshot = crate::sidebar::layout_tests::snapshot(3);
        let tree = |linked| sheprd_client::protocol::ClientShellWorktree {
            is_linked_worktree: linked,
            key: "repo".into(),
            label: "repo".into(),
        };
        snapshot.workspaces[0].worktree = Some(tree(true));
        snapshot.workspaces[1].worktree = Some(tree(false));
        snapshot.workspaces[2].worktree = None;
        view.live.snapshot = Some(Arc::new(snapshot));
        view.hosted_state(cx)
    });
    let rows = state.main["workspaces"].as_array().unwrap();
    assert_eq!(rows[0]["child"], false);
    assert_eq!(rows[0]["linked"], false);
    assert_eq!(rows[1]["child"], true);
    assert_eq!(rows[1]["linked"], true);
}
