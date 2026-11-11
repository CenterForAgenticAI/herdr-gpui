use super::*;
use sheprd_session::{Keeper, Workspace, Worktree};

fn workspace(branch: &str, worktree: Option<(&str, bool)>) -> Workspace {
    Workspace {
        ahead_behind: None,
        branch: Some(branch.into()),
        cwd: "/repo/lane".into(),
        custom_label: false,
        id: "lane".into(),
        keeper: Keeper::App,
        label: "lane".into(),
        status: sheprd_backend::AgentStatus::Idle,
        tabs: Vec::new(),
        tokens: Vec::new(),
        worktree: worktree.map(|(key, linked)| Worktree {
            key: key.into(),
            label: "repo".into(),
            linked,
        }),
    }
}

#[test]
fn the_same_linked_checkout_keeps_the_archive() {
    let current = workspace("lane", Some(("/repo/.git", true)));
    assert_eq!(
        checkout_identity(Some(&current), "/repo/.git", Some("lane")),
        Identity::Same
    );
}

/// A failed remote Git lookup leaves the workspace without its worktree;
/// that is no proof the checkout changed, so the deletion keeps waiting.
#[test]
fn an_unknown_worktree_keeps_waiting() {
    let current = workspace("lane", None);
    assert_eq!(
        checkout_identity(Some(&current), "/repo/.git", Some("lane")),
        Identity::Unknown
    );
}

#[test]
fn a_closed_workspace_or_another_checkout_cancels_the_archive() {
    assert_eq!(
        checkout_identity(None, "/repo/.git", Some("lane")),
        Identity::Changed
    );
    for current in [
        workspace("other", Some(("/repo/.git", true))),
        workspace("other", None),
        workspace("lane", Some(("/other/.git", true))),
        workspace("lane", Some(("/repo/.git", false))),
    ] {
        assert_eq!(
            checkout_identity(Some(&current), "/repo/.git", Some("lane")),
            Identity::Changed,
            "{current:?}"
        );
    }
}
