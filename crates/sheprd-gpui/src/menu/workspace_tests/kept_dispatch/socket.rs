use super::*;

/// Points the shown endpoint at `socket` and keeps a workspace at `LINKED`
/// there: kept workspaces are scoped by socket path, so the one `window`
/// opened under `Local` is not this endpoint's.
fn keep_on_socket(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, socket: &str) -> String {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.endpoints[0].connection.target =
                sheprd_client::ConnectTarget::Socket(socket.into());
            let location =
                Location::core_terminal(Panes::single(TerminalId::new("t2").unwrap()), false);
            view.open_kept_workspace(None, Path::new(LINKED), location, window, cx)
        })
    })
    .unwrap()
}

#[gpui::test]
fn forwarded_socket_snapshot_never_supplies_local_core_repository_paths(cx: &mut TestAppContext) {
    let (view, cx, _) = window(cx, false);
    keep_on_socket(&view, cx, "/private/forwarded.sock");
    view.update(cx, |view, cx| {
        view.config.herdr_2_terminals = true;
        view.live.missing_installation = false;
        view.live.status = crate::state::ConnectionStatus::Connected;
        let mut snapshot = sidebar::layout_tests::snapshot(7);
        for workspace in &mut snapshot.workspaces {
            if let Some(tree) = &mut workspace.worktree {
                tree.key = "/foreign/project/.git".into();
                tree.label = "foreign-project".into();
            }
        }
        view.live.snapshot = Some(std::sync::Arc::new(snapshot));
        view.refresh_core_session(cx);
        let endpoint = view.endpoints[0].id.clone();
        let destination = view.dispatch_destination(&endpoint).unwrap();
        let repositories = destination.repositories.unwrap();
        assert!(repositories.iter().any(|repo| repo.key == KEY));
        assert!(
            repositories
                .iter()
                .all(|repo| repo.key != "/foreign/project/.git")
        );
        let candidates = view.dispatch_candidates("foreign-project", cx);
        let local = candidates
            .iter()
            .find(|candidate| candidate.endpoint_id == endpoint)
            .unwrap();
        assert_eq!(local.repository, Repository::Missing);
        assert_eq!(
            local.working, 0,
            "the forwarded daemon's agents do not run locally"
        );
        assert!(view.herdr_1_destination(&endpoint).is_none());
    });
}

#[gpui::test]
fn a_custom_socket_routes_only_app_kept_work_to_local_core_files(cx: &mut TestAppContext) {
    let (view, cx, _) = window(cx, false);
    let workspace = keep_on_socket(&view, cx, "/private/custom-herdr.sock");
    view.update(cx, |view, cx| {
        let endpoint = view.endpoints[0].id.clone();
        let local = crate::teleport::host_for(&sheprd_client::ConnectTarget::Local).unwrap();
        let origin = view.dispatch_origin(&workspace, KEY, "app").unwrap();
        assert_eq!(origin.place.host, local);
        let destination = view.dispatch_destination(&endpoint).unwrap();
        assert_eq!(destination.lands_in, sheprd_session::Keeper::App);
        assert_eq!(destination.place.host, local);
        assert!(
            view.dispatch_candidates("app", cx)
                .iter()
                .any(|candidate| candidate.endpoint_id == endpoint && candidate.online)
        );
        assert!(
            view.dispatch_origin("herdr-1-workspace", KEY, "app")
                .is_none()
        );
        assert!(view.herdr_1_destination(&endpoint).is_none());
    });
}
