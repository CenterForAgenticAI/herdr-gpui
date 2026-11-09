//! Re-clicking an agents-list row scrolls its pane to the live bottom
//! instead of focusing it again: the classifier's decision, the guards
//! that keep navigation, and the wire the second click produces.

use super::*;
use crate::{
    connection::ConnectionBridge,
    navigation::{AgentRowAction, NavigationTarget, RescrollTarget},
    state::ConnectionStatus,
    terminal::InputTarget,
    window::MockPeer,
};
use sheprd_backend::AgentStatus as CoreAgentStatus;
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use sheprd_session::{Keeper, Session, Workspace as CoreWorkspace};
use std::sync::Arc;

/// The pane the local row names, focused in the snapshot and in its agent,
/// so the row paints selected and the classifier agrees with the paint.
const PANE: &str = "p0";
const KEPT: &str = "kept";
/// The local row's debug selector, static for `debug_bounds`.
const LOCAL_ROW: &str = "agent-local-p0";

/// The surface fixture for `listing`: an 80x24 frame showing `PANE`, the
/// size the window asked for.
fn fixture_surface(listing: &ClientShellSnapshot) -> PaneSurfaceFrame {
    PaneSurfaceFrame {
        boot_id: listing.boot_id.clone(),
        projection_revision: listing.revision,
        surface_revision: 1,
        frame: FrameData {
            width: 80,
            height: 24,
            cells: vec![],
            cursor: None,
            hyperlinks: vec![],
            graphics: vec![],
        },
        panes: vec![PaneSurfacePane {
            pane_id: PANE.into(),
            content_revision: 1,
            rect: SurfaceRect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            inner_rect: SurfaceRect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            scrollbar_rect: None,
            scroll: None,
            focused: true,
            mouse_reporting: true,
            sgr_pixel_mouse: false,
            alternate_screen_active: false,
            pixel_width: 800,
            pixel_height: 480,
        }],
        splits: vec![],
        popup: None,
        graphics: Default::default(),
    }
}

/// Local and `ssh:test` hosts, the local one ready for input on the mock
/// peer: its surface arrived, sized as the window asked, showing `PANE`.
fn rescroll_fixture(
    cx: &mut gpui::TestAppContext,
) -> (Entity<SheprdWindow>, MockPeer, &mut gpui::VisualTestContext) {
    let peer = MockPeer::advertising(&["pane.focus", "pane.scroll"]);
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let mut view = fixture_window(window, cx);
            peer.prepare(&mut view, cx);
            let mut listing = snapshot(1);
            listing.agents[0].focused = true;
            listing.panes = vec![serde_json::from_value(serde_json::json!({
                "pane_id": PANE, "workspace_id": "w0", "tab_id": "t0", "label": "review",
                "cwd": "/repo", "foreground_cwd": "/repo", "focused": true,
                "right_click_passthrough": false,
            }))
            .unwrap()];
            listing.focused_pane_id = Some(PANE.into());
            view.live.snapshot = Some(Arc::new(listing.clone()));
            view.live.surface = Some(Arc::new(fixture_surface(&listing)));
            let mut remote = crate::endpoint::Endpoint::new(
                "ssh:test".into(),
                "Remote".into(),
                ConnectTarget::Ssh {
                    target: "unused".into(),
                    session: "default".into(),
                },
                true,
            );
            remote.live.snapshot = Some(Arc::new(listing));
            remote.live.status = ConnectionStatus::Connected;
            view.endpoints.push(remote);
            view
        });
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        SidebarFixture(view)
    });
    let view = cx.update(|_, cx| fixture.read(cx).0.clone());
    cx.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();
    let ready = view.read_with(cx, |view, _| view.input_ready());
    assert!(ready, "fixture window should be ready for input");
    (view, peer, cx)
}

/// The next `pane.` request on the wire, answering the surface traffic a
/// drawn window sends on its own.
fn next_pane_request(peer: &mut MockPeer) -> serde_json::Value {
    loop {
        if let ClientMessage::ClientShellEndpointRequest { boot_id, request } = peer.receive() {
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            if request["method"]
                .as_str()
                .is_some_and(|method| method.starts_with("pane."))
            {
                return request;
            }
            let id = request["id"].as_str().unwrap();
            peer.respond(&boot_id, id, &serde_json::json!({"id": id, "result": {"type": "ok"}}));
        }
    }
}

/// Proves no pane request follows on the wire: an input sentinel anything
/// the rescroll left behind would have to precede.
fn assert_wire_quiet(
    peer: &mut MockPeer,
    boot: &str,
    cx: &mut gpui::VisualTestContext,
) {
    let handle = peer.client.handle.clone();
    cx.update(|_, _| {
        ConnectionBridge::send_input(
            &handle,
            boot,
            &InputTarget::Pane(PANE.into()),
            ClientPaneInputEvent::TextCommit("quiet".into()),
        )
        .unwrap();
    });
    loop {
        match peer.receive() {
            ClientMessage::ClientShellPaneInput { events, .. } => {
                assert_eq!(events[0], ClientPaneInputEvent::TextCommit("quiet".into()));
                return;
            }
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let request: serde_json::Value = serde_json::from_str(&request).unwrap();
                panic!(
                    "unexpected {} after the rescroll",
                    request["method"].as_str().unwrap_or("?")
                );
            }
            _ => {}
        }
    }
}

#[gpui::test]
fn the_focused_row_means_a_rescroll_and_other_rows_mean_navigation(
    cx: &mut gpui::TestAppContext,
) {
    let (view, _peer, cx) = rescroll_fixture(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.agent_row_action("local", PANE),
            AgentRowAction::Rescroll(RescrollTarget::HerdrPane {
                pane: PANE.into()
            })
        );
        assert_eq!(view.agent_row_action("ssh:test", PANE), AgentRowAction::Navigate);
        assert_eq!(view.agent_row_action("local", "p1"), AgentRowAction::Navigate);
    });
}

#[gpui::test]
fn an_unready_or_covered_pane_keeps_navigating(cx: &mut gpui::TestAppContext) {
    let (view, _peer, cx) = rescroll_fixture(cx);
    // Without its surface the window is not ready, so the click keeps
    // the existing queueing.
    let held = view.update(cx, |view, _| {
        let held = view.live.surface.take();
        assert!(!view.input_ready());
        assert_eq!(view.agent_row_action("local", PANE), AgentRowAction::Navigate);
        held
    });
    view.update(cx, |view, cx| {
        assert!(view.open_agent_row("local", PANE, cx));
        assert_eq!(
            view.pending_navigation,
            Some(NavigationTarget::Pane(PANE.into()))
        );
        view.pending_navigation = None;
        view.live.surface = held;
    });
    // A kept herdr-core workspace in front covers the pane, so the click is
    // a real navigation until the user leaves it.
    view.update(cx, |view, cx| {
        let mut session = Session::default();
        session.workspaces.push(CoreWorkspace {
            ahead_behind: None,
            branch: None,
            cwd: "/src".into(),
            custom_label: false,
            id: KEPT.into(),
            keeper: Keeper::App,
            label: KEPT.into(),
            status: CoreAgentStatus::Idle,
            tabs: Vec::new(),
            tokens: Vec::new(),
            worktree: None,
        });
        let scope = crate::browser::scope(&view.endpoints[0]);
        view.set_core_session(scope, session);
        assert!(view.focus_kept_workspace(KEPT, cx));
        assert_eq!(view.agent_row_action("local", PANE), AgentRowAction::Navigate);
        view.leave_kept_workspace();
        assert_eq!(
            view.agent_row_action("local", PANE),
            AgentRowAction::Rescroll(RescrollTarget::HerdrPane {
                pane: PANE.into()
            })
        );
    });
}

#[gpui::test]
fn a_second_click_supersedes_a_queued_navigation_and_scrolls(cx: &mut gpui::TestAppContext) {
    let (view, mut peer, cx) = rescroll_fixture(cx);
    let boot = view.update(cx, |view, _| {
        view.pending_navigation = Some(NavigationTarget::Tab("w9".into()));
        view.live.snapshot.as_ref().unwrap().boot_id.clone()
    });
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    let row = cx.debug_bounds(LOCAL_ROW).unwrap();
    cx.simulate_click(row.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(view.pending_navigation.is_none());
    });
    let scroll = next_pane_request(&mut peer);
    assert_eq!(scroll["method"], "pane.scroll");
    assert_eq!(
        scroll["params"],
        serde_json::json!({"pane_id": PANE, "offset_from_bottom": 0})
    );
    let id = scroll["id"].as_str().unwrap().to_owned();
    peer.respond(&boot, &id, &serde_json::json!({"id": id, "result": {"type": "ok"}}));
    // No focus follows: the pane is already the shown one.
    assert_wire_quiet(&mut peer, &boot, cx);
}

#[gpui::test]
fn an_unfocused_row_still_navigates(cx: &mut gpui::TestAppContext) {
    let (view, _peer, cx) = rescroll_fixture(cx);
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    let row = cx.debug_bounds("agent-ssh:test-p0").unwrap();
    cx.simulate_click(row.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.selected_endpoint, 1, "the row's host is selected");
    });
}
