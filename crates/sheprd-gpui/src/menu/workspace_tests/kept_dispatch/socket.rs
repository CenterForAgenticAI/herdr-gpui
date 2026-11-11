use super::*;

#[gpui::test]
fn a_custom_socket_routes_only_app_kept_work_to_local_core_files(cx: &mut TestAppContext) {
    let (view, cx, workspace) = window(cx, false);
    view.update(cx, |view, cx| {
        view.endpoints[0].connection.target =
            sheprd_client::ConnectTarget::Socket("/private/custom-herdr.sock".into());
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
