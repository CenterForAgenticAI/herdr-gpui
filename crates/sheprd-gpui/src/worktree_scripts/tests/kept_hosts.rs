use super::*;

#[gpui::test]
fn queued_setup_is_cancelled_when_its_saved_route_is_replaced(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    view.update(cx, |view, cx| {
        let mut queued = launch(
            view,
            ScriptKind::Setup,
            Some(std::path::Path::new("/same/path")),
        );
        queued.workspace.clear();
        view.queue_kept_setup(queued);
        assert_eq!(view.queued_kept_setups().len(), 1);
        view.endpoints[0].connection.target = sheprd_client::ConnectTarget::Ssh {
            target: "replacement".into(),
            session: "work".into(),
        };
        view.poll_kept_scripts(cx);
        assert!(view.queued_kept_setups().is_empty());
        assert!(view.worktree_script.is_none());
    });
}
