//! CLI adapters must reach app-kept work without a Herdr 1 snapshot.
use super::*;
use crate::{
    browser::{Location, Store},
    control::protocol::work::Reply,
    core_terminals::panes::Panes,
    notification_mutes::{Mutes, Scope},
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::{control::protocol::work::FanOutRequest, menu::Page};
use sheprd_backend::TerminalId;

fn kept(
    view: &mut SheprdWindow,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<SheprdWindow>,
) -> Caller {
    view.endpoints[0].connection.target = sheprd_client::ConnectTarget::Local;
    view.live.snapshot = None;
    view.live.missing_installation = true;
    view.core_terminals.seed_checkout_for_test(
        "/src/tools",
        Some(sheprd_vcs::git::Identity {
            branch: Some("feature".into()),
            key: "/src/tools/.git".into(),
            label: "tools".into(),
            linked: true,
        }),
    );
    let terminal = TerminalId::new("t-kept").unwrap();
    let location = Location::core_terminal(Panes::single(terminal.clone()), false);
    let id = view
        .open_kept_workspace(
            Some("tools"),
            std::path::Path::new("/src/tools"),
            location,
            window,
            cx,
        )
        .unwrap();
    let scope = crate::browser::scope(&view.endpoints[0]);
    let tab = cx
        .global::<Store>()
        .in_workspace(&scope, &id)
        .next()
        .unwrap()
        .id;
    view.core_terminals
        .show_for_test(tab, Panes::single(terminal), false, Vec::new());
    view.core_terminals.place_for_test(tab, scope, &id);
    Caller {
        terminal_id: Some("t-kept".into()),
        ..caller(None)
    }
}

#[gpui::test]
fn kept_mute_uses_the_callers_scope_and_preserves_the_host_mute(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|window, cx| view.update(cx, |view, cx| {
        let here = kept(view, window, cx);
        let scope = crate::browser::scope(&view.endpoints[0]);
        let id = find(view, &here).ok().unwrap().workspace.id;
        for muted in [true, true, false] {
            let response = mute::set(view, &here, muted, cx);
            let Response::Work { reply: Reply::Mute { mute } } = response else {
                panic!("{response:?}");
            };
            assert_eq!(mute.muted, muted);
            assert_eq!(mute.workspace, "tools");
            assert_eq!(Mutes::of(cx).unwrap().kept(scope.as_str(), Scope::Workspace, &id), muted);
            assert!(!Mutes::of(cx).unwrap().kept("ssh:elsewhere", Scope::Workspace, &id));
        }
        Mutes::update(cx, |mutes| mutes.set_host(crate::core_terminals::Host::Local.endpoint(), true));
        let response = mute::set(view, &caller(None), false, cx);
        assert!(matches!(response, Response::Work { reply: Reply::Mute { mute } } if !mute.muted && mute.host_muted));
        let foreign = Caller { daemon_socket: Some("/unrelated/daemon.sock".into()), ..caller(None) };
        assert!(error_code(&mute::set(view, &foreign, true, cx)).is_some());
        assert!(!Mutes::of(cx).unwrap().kept(scope.as_str(), Scope::Workspace, &id));
    }));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[gpui::test]
fn kept_fan_out_only_composes_and_refuses_a_different_selected_host(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let here = kept(view, window, cx);
            let request = || FanOutRequest {
                prompt: "fix the test".into(),
                agents: vec![("claude".into(), 1)],
            };
            let response = flows::fan_out(view, &here, request(), window, cx);
            assert!(matches!(response, Response::Done { .. }), "{response:?}");
            assert_eq!(view.menu.page, Some(Page::FanOut));
            assert!(
                view.fan_out.as_ref().unwrap().composing(),
                "the user still confirms launch"
            );
            view.dismiss_menu(window, cx);
            view.fan_out = None;
            view.endpoints.push(crate::endpoint::Endpoint::new(
                "ssh:box".into(),
                "Box".into(),
                sheprd_client::ConnectTarget::Ssh {
                    target: "nobody@invalid.invalid".into(),
                    session: "default".into(),
                },
                true,
            ));
            view.selected_endpoint = 1;
            let response = flows::fan_out(view, &here, request(), window, cx);
            assert_eq!(
                error_code(&response),
                Some(ErrorCode::InvalidRequest),
                "{response:?}"
            );
            assert!(view.menu.page.is_none());
            assert!(view.fan_out.is_none());
        })
    });
}

#[gpui::test]
fn a_kept_caller_cannot_mute_in_a_window_that_does_not_hold_it(cx: &mut gpui::TestAppContext) {
    let (own, own_cx) = cx.add_window_view(fixture_window);
    let here = own_cx.update(|window, cx| own.update(cx, |view, cx| kept(view, window, cx)));
    let (other, other_cx) = cx.add_window_view(fixture_window);
    other.update(other_cx, |view, cx| {
        let response = mute::set(view, &here, true, cx);
        assert!(error_code(&response).is_some(), "{response:?}");
    });
    own.read_with(own_cx, |view, cx| {
        assert!(matches!(mute::status(view, &here, cx), Response::Work { reply: Reply::Mute { mute } } if !mute.muted));
    });
}
