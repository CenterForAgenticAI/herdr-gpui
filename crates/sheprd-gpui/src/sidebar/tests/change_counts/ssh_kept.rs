use super::*;
use crate::pull_request::{Cache, Origin, View};
use std::sync::Arc;

#[test]
fn ssh_kept_badge_and_counts_read_the_prefetch_input_without_h1_lookup() {
    let mut workspace = crate::session_model::of_snapshot(&layout_tests::snapshot(6))
        .workspaces
        .remove(4);
    workspace.keeper = sheprd_session::Keeper::App;
    workspace.cwd = "/remote/checkout".into();
    workspace.worktree.as_mut().unwrap().key = "/remote/repo/.git".into();
    let input = crate::pull_request::kept_input(&workspace, true).unwrap();
    let view = View {
        origin: Origin::Ssh("box".into()),
        login: "remote-account".into(),
    };
    let mut cache = Cache::default();
    cache.scope(
        (0, 0, String::new()),
        Arc::new("fixture".into()),
        &view.login,
        view.origin.clone(),
    );
    let mut pr = crate::pull_request::fixture().unwrap();
    pr.additions = 1730;
    pr.deletions = 31;
    cache.seed(input.clone(), pr, Instant::now());
    assert!(
        cache
            .peek(&view, input.repo_key.as_deref().unwrap(), &input.branch)
            .is_none(),
        "no secondary H1 lookup is seeded"
    );
    let marks = crate::teleport::Marks::detached();
    let theme = Theme::default();
    // Owned results, so no call holds `workspace` or a `View` past itself.
    let badge = |workspace: &_, view: &View| {
        workspace_badge(
            workspace,
            Some((&cache, view)),
            (None, ChangeCounts::PullRequest),
            (&marks, "ssh:box"),
            (None, false),
            &theme,
        )
        .map(|found| {
            let counts = found
                .counts
                .map(|counts| (counts.additions.to_string(), counts.deletions.to_string()));
            (found.pr.map(|pr| pr.number.to_string()), counts)
        })
    };
    let (number, counts) = badge(&workspace, &view).unwrap();
    assert_eq!(number.as_deref(), Some("#8"));
    assert_eq!(counts, Some(("+1730".into(), "-31".into())));
    for wrong in [
        View {
            origin: Origin::Local,
            ..view.clone()
        },
        View {
            origin: Origin::Ssh("other".into()),
            ..view.clone()
        },
        View {
            login: "other-account".into(),
            ..view.clone()
        },
    ] {
        assert!(badge(&workspace, &wrong).is_none());
    }
    workspace.cwd = "/remote/other-checkout".into();
    assert!(
        badge(&workspace, &view).is_none(),
        "another checkout cannot borrow the answer"
    );
}
