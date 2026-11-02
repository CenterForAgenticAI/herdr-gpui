//! This Mac's region while another machine is viewed: it fills the edge the
//! viewed machine leaves free, takes the keyboard in this window only, and
//! is shown, hidden and resized with region commands to this Mac's server.
use super::*;
use crate::endpoint::{Endpoint, LOCAL};
use gpui::{Modifiers, MouseButton, point};
use sheprd_client::{
    ConnectTarget,
    protocol::region_surface::{BACKGROUND_CAPABILITY, COMMAND_KIND, RegionCommand},
};
use sheprd_commands::Command;

/// This Mac's daemon, offering regions and, when `background`, streaming
/// them while another machine is viewed. Its connection is not the one
/// viewed, so it also offers the surface interest such a connection needs.
fn local_peer(background: bool) -> MockPeer {
    let mut capabilities = vec![CAPABILITY, "surface_interest", "presentation_effects_fence"];
    if background {
        capabilities.push(BACKGROUND_CAPABILITY);
    }
    let mut methods = METHODS.to_vec();
    methods.push("client_shell.surface.set");
    MockPeer::greeting(
        &methods,
        &capabilities,
        Hello {
            surface_active: false,
            regions: true,
        },
    )
}

/// A window viewing a second machine, served by `remote`, which offers no
/// regions, while this Mac's server, `local`, has a 36-cell right region.
fn away<'a>(
    local: &MockPeer,
    remote: &MockPeer,
    background: bool,
    cx: &'a mut TestAppContext,
) -> (Entity<SheprdWindow>, &'a mut VisualTestContext) {
    let (view, cx) = window(local, cx);
    // One update, so no frame draws this Mac's region as the viewed one.
    view.update(cx, |view, cx| {
        view.live.supports_regions = true;
        view.live.supports_region_background = background;
        let region = region(36, true, false);
        view.live
            .regions
            .apply_projection(Arc::new(RegionProjection {
                boot_id: "boot-v1".into(),
                revision: 1,
                regions: vec![region],
                focused: None,
            }));
        view.endpoints[0].live = view.live.clone();
        let mut endpoint = Endpoint::new(
            "ssh:box".into(),
            "box".into(),
            ConnectTarget::Ssh {
                target: "penso@box".into(),
                session: "default".into(),
            },
            true,
        );
        endpoint.never_connect();
        endpoint.attach_shown(remote.client.handle.clone());
        view.live.supports_regions = false;
        view.live.supports_region_background = false;
        view.live.regions = Default::default();
        endpoint.live = view.live.clone();
        view.endpoints.push(endpoint);
        view.selected_endpoint = 1;
        cx.notify();
    });
    (view, cx)
}

/// Gives the viewed machine its own right region, `region_r`, 30 cells.
fn remote_region(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) {
    view.update(cx, |view, cx| {
        let mut region = region(30, true, false);
        region.region_id = "region_r".into();
        region.terminal_id = "term_r".into();
        view.live.supports_regions = true;
        view.live
            .regions
            .apply_projection(Arc::new(RegionProjection {
                boot_id: "boot-v1".into(),
                revision: 1,
                regions: vec![region],
                focused: None,
            }));
        cx.notify();
    });
}

/// The region commands among `messages`.
fn commands(messages: &[ClientMessage]) -> Vec<RegionCommand> {
    messages
        .iter()
        .filter_map(|message| match message {
            ClientMessage::EndpointControl { kind, data } if kind == COMMAND_KIND => {
                RegionCommand::decode(data).ok()
            }
            _ => None,
        })
        .collect()
}

/// The typed input among `messages`: region terminal or pane, and its text.
fn typed(messages: &[ClientMessage]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            ClientMessage::ClientShellPopupInput {
                terminal_id,
                events,
            } => Some(format!("region {terminal_id} {events:?}")),
            ClientMessage::ClientShellPaneInput { pane_id, events } if pane_id != "marker" => {
                Some(format!("pane {pane_id} {events:?}"))
            }
            _ => None,
        })
        .collect()
}

fn type_text(view: &Entity<SheprdWindow>, text: &str, cx: &mut VisualTestContext) {
    view.update(cx, |view, cx| {
        assert!(view.send(ClientPaneInputEvent::TextCommit(text.into()), cx));
    });
}

fn local_focus(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |view, _| view.desktop_region.local_focus.clone())
}

fn run(view: &Entity<SheprdWindow>, command: Command, cx: &mut VisualTestContext) {
    cx.update(|window, cx| view.update(cx, |view, cx| view.command(command, window, cx)));
}

#[gpui::test]
fn local_region_stays_drawn_while_another_machine_is_viewed(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (local_peer(true), peer());
    let (view, cx) = away(&local, &remote, true, cx);
    draw(cx);
    let cell_height = view.read_with(cx, |view, _| view.config.terminal.line_height());
    let terminal = cx.debug_bounds("desktop-region-terminal").unwrap();
    let rows = (f32::from(terminal.size.height) / cell_height).floor() as u16;
    // The cells it is drawn at go to this Mac's server, not the viewed one.
    assert_eq!(
        viewports(&sent(&mut local, &view, cx)),
        [RegionViewport {
            region_id: "region_1".into(),
            cols: 36,
            rows,
        }]
    );
    assert_eq!(viewports(&sent(&mut remote, &view, cx)), []);
}

#[gpui::test]
fn a_local_server_without_background_regions_is_not_drawn_while_away(cx: &mut TestAppContext) {
    let (mut local, remote) = (local_peer(false), peer());
    let (view, cx) = away(&local, &remote, false, cx);
    draw(cx);
    assert!(cx.debug_bounds("desktop-region").is_none());
    assert_eq!(viewports(&sent(&mut local, &view, cx)), []);
}

#[gpui::test]
fn the_viewed_machine_region_wins_its_edge_and_the_covered_local_one_reports_zero(
    cx: &mut TestAppContext,
) {
    let (mut local, mut remote) = (local_peer(true), peer());
    let (view, cx) = away(&local, &remote, true, cx);
    remote_region(&view, cx);
    draw(cx);
    let first = viewports(&sent(&mut local, &view, cx));
    // Once covered, this Mac's region has no room here.
    assert_eq!(
        first.last(),
        Some(&RegionViewport {
            region_id: "region_1".into(),
            cols: 0,
            rows: 0,
        })
    );
    draw(cx);
    let cell_width = view.read_with(cx, |view, _| view.cell_width);
    let panel = cx.debug_bounds("desktop-region").unwrap();
    assert!(
        (panel.size.width - px(30. * cell_width + BORDER)).abs() < px(0.5),
        "{panel:?}"
    );
    assert_eq!(
        viewports(&sent(&mut local, &view, cx)),
        [],
        "nothing more until it changes"
    );
    let remote = viewports(&sent(&mut remote, &view, cx));
    assert_eq!(
        remote
            .iter()
            .map(|v| (v.region_id.as_str(), v.cols))
            .collect::<Vec<_>>(),
        [("region_r", 30)]
    );
}

#[gpui::test]
fn clicking_a_local_region_on_a_remote_view_focuses_it_on_the_client_only(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (local_peer(true), peer());
    let (view, cx) = away(&local, &remote, true, cx);
    draw(cx);
    let terminal = cx.debug_bounds("desktop-region-terminal").unwrap();
    cx.simulate_click(terminal.center(), Modifiers::default());
    assert_eq!(local_focus(&view, cx).as_deref(), Some("region_1"));
    draw(cx);
    settle(&view, cx);
    type_text(&view, "a", cx);
    let to_local = sent(&mut local, &view, cx);
    assert!(requests(&to_local).is_empty(), "no server holds this focus");
    assert_eq!(typed(&to_local), [r#"region term_1 [TextCommit("a")]"#]);
    let to_remote = sent(&mut remote, &view, cx);
    assert!(requests(&to_remote).is_empty());
    assert_eq!(typed(&to_remote), Vec::<String>::new());
    // Clicking a pane of the viewed machine takes the keyboard back.
    view.update(cx, |view, cx| view.focus_clicked_pane("w1:p1", cx));
    assert_eq!(local_focus(&view, cx), None);
    let target = view.read_with(cx, |view, _| view.region_input_target().map(str::to_owned));
    assert_eq!(target, None, "typing goes to the viewed machine again");
}

#[gpui::test]
fn focus_binding_toggles_local_focus_and_switching_machines_clears_it(cx: &mut TestAppContext) {
    let (mut local, remote) = (local_peer(true), peer());
    let (view, cx) = away(&local, &remote, true, cx);
    draw(cx);
    run(&view, Command::FocusDesktopRegion, cx);
    assert_eq!(local_focus(&view, cx).as_deref(), Some("region_1"));
    run(&view, Command::FocusDesktopRegion, cx);
    assert_eq!(local_focus(&view, cx), None);
    run(&view, Command::FocusDesktopRegion, cx);
    assert_eq!(local_focus(&view, cx).as_deref(), Some("region_1"));
    assert!(requests(&sent(&mut local, &view, cx)).is_empty());
    let switched = view.update(cx, |view, cx| view.select_endpoint(LOCAL, cx));
    assert!(switched);
    assert_eq!(local_focus(&view, cx), None);
}

#[gpui::test]
fn toggle_and_drag_on_a_local_region_send_region_commands_to_the_local_server(
    cx: &mut TestAppContext,
) {
    let (mut local, mut remote) = (local_peer(true), peer());
    let (view, cx) = away(&local, &remote, true, cx);
    draw(cx);
    let cell_width = view.read_with(cx, |view, _| view.cell_width);
    let start = cx.debug_bounds("desktop-region-resize").unwrap().center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    // The first small step starts the drag.
    cx.simulate_mouse_move(
        point(start.x - px(4.), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(start.x - px(10. * cell_width), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(start.x - px(10. * cell_width), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    run(&view, Command::ToggleDesktopRegion, cx);
    let to_local = sent(&mut local, &view, cx);
    assert!(requests(&to_local).is_empty());
    assert_eq!(
        commands(&to_local).last(),
        Some(&RegionCommand::Hide {
            region_id: "region_1".into()
        })
    );
    assert!(commands(&to_local).contains(&RegionCommand::Resize {
        region_id: "region_1".into(),
        size: 46,
    }));
    // This Mac's server hid it; toggling shows it again.
    view.update(cx, |view, _| {
        let mut hidden = region(46, false, false);
        hidden.size = 46;
        view.endpoints[0]
            .live
            .regions
            .apply_projection(Arc::new(RegionProjection {
                boot_id: "boot-v1".into(),
                revision: 2,
                regions: vec![hidden],
                focused: None,
            }));
    });
    draw(cx);
    run(&view, Command::ToggleDesktopRegion, cx);
    assert_eq!(
        commands(&sent(&mut local, &view, cx)),
        [RegionCommand::Show {
            region_id: "region_1".into()
        }]
    );
    let to_remote = sent(&mut remote, &view, cx);
    assert!(requests(&to_remote).is_empty());
    assert_eq!(commands(&to_remote), []);
}

#[gpui::test]
fn a_pane_focus_binding_takes_the_keyboard_off_a_local_region(cx: &mut TestAppContext) {
    let mut methods = METHODS.to_vec();
    methods.push("pane.focus_direction");
    let hello = Hello {
        surface_active: true,
        regions: true,
    };
    let (local, mut remote) = (
        local_peer(true),
        MockPeer::greeting(&methods, &[CAPABILITY], hello),
    );
    let (view, cx) = away(&local, &remote, true, cx);
    draw(cx);
    settle(&view, cx);
    run(&view, Command::FocusDesktopRegion, cx);
    assert_eq!(local_focus(&view, cx).as_deref(), Some("region_1"));
    run(&view, Command::FocusLeft, cx);
    assert_eq!(local_focus(&view, cx), None);
    let error = view.read_with(cx, |view, _| view.local_error.clone());
    assert_eq!(error, None);
    assert_eq!(remote.request()["method"], "pane.focus_direction");
}
