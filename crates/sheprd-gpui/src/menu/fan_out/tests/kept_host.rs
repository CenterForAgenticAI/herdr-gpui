use super::*;

#[gpui::test]
fn a_kept_origin_fans_out_on_its_own_host(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update(cx, |view, _| {
        let source = Source {
            workspace_id: "space-1".into(),
            repo_key: "/same/path/.git".into(),
            repo_label: "app".into(),
            base: "HEAD".into(),
            checkout: Some("/same/path".into()),
        };
        assert!(
            view.fan_out_host(&source).unwrap().is_local(),
            "a custom Herdr socket still keeps local core files"
        );
        for target in [
            sheprd_client::ConnectTarget::Ssh {
                target: "box".into(),
                session: "work".into(),
            },
            sheprd_client::ConnectTarget::Wsl {
                distro: "Ubuntu".into(),
                session: "work".into(),
            },
        ] {
            view.endpoints[0].connection.target = target.clone();
            let host = view.fan_out_host(&source);
            assert_eq!(host, host_for(&target).ok());
            assert!(host.is_none_or(|host| !host.is_local()));
        }
    });
}
