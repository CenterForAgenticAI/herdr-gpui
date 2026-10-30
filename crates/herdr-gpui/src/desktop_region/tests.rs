//! Tests for the desktop region panel. Shared fixtures live here; each
//! topic has its own file.
#![allow(clippy::unwrap_used)]

use super::*;
use crate::{
    sidebar::layout_tests::{fixture_window, full_draw},
    window::MockPeer,
};
use gpui::{Entity, TestAppContext, VisualTestContext, px};
use herdr_client::{
    Hello,
    protocol::{
        ClientMessage, ClientPaneInputEvent, FrameData,
        region_surface::{CAPABILITY, RegionProjection, RegionSurface, VIEWPORT_KIND},
    },
};
use std::sync::Arc;

mod commands;
mod input;
mod layout;
mod panel;
mod resize;

const METHODS: &[&str] = &[
    "region.focus",
    "region.hide",
    "region.resize",
    "region.show",
];

/// A daemon peer whose welcome offers regions to a window that asked.
fn peer() -> MockPeer {
    MockPeer::greeting(
        METHODS,
        &[CAPABILITY],
        Hello {
            surface_active: true,
            regions: true,
        },
    )
}

/// A right-edge region `size` cells wide, 12 to 60.
fn region(size: u16, visible: bool, focused: bool) -> ClientRegion {
    ClientRegion {
        region_id: "region_1".into(),
        terminal_id: "term_1".into(),
        plugin_id: "demo.agent-board".into(),
        entrypoint: "board".into(),
        title: "Agent board".into(),
        edge: "right".into(),
        scope: "session".into(),
        size,
        min_size: 12,
        max_size: 60,
        visible,
        focused,
        mouse_reporting: false,
    }
}

/// A window connected to `peer` as the daemon's fixture snapshot says.
fn window<'a>(
    peer: &MockPeer,
    cx: &'a mut TestAppContext,
) -> (Entity<HerdrWindow>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| view.update(cx, |view, cx| peer.prepare(view, cx)));
    (view, cx)
}

/// Gives the window projection `revision` with `region`, on a connection
/// that negotiated regions.
fn project(
    view: &Entity<HerdrWindow>,
    revision: u64,
    region: ClientRegion,
    cx: &mut VisualTestContext,
) {
    offer(view, revision, region, true, cx);
}

/// Gives the window projection `revision` with `region`, on a connection
/// that negotiated regions when `negotiated`.
fn offer(
    view: &Entity<HerdrWindow>,
    revision: u64,
    region: ClientRegion,
    negotiated: bool,
    cx: &mut VisualTestContext,
) {
    view.update(cx, |view, cx| {
        view.live.supports_regions = negotiated;
        let focused = region.focused.then(|| region.region_id.clone());
        view.live
            .regions
            .apply_projection(Arc::new(RegionProjection {
                boot_id: "boot-v1".into(),
                revision,
                regions: vec![region],
                focused,
            }));
        view.live.regions.apply_surface(Arc::new(RegionSurface {
            region_id: "region_1".into(),
            terminal_id: "term_1".into(),
            frame: FrameData {
                cells: vec![],
                width: 0,
                height: 0,
                cursor: None,
                hyperlinks: vec![],
                graphics: vec![],
            },
        }));
        cx.notify();
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
}

/// Puts the window back to the size its fixture surface was drawn at, as a
/// daemon answering the resize a draw sends would, so pane input is ready.
fn settle(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) {
    view.update(cx, |view, _| {
        view.options = herdr_client::ConnectOptions::default();
    });
}

/// The next endpoint request, answered so the connection's one request
/// lease frees for the next.
fn answered(peer: &mut MockPeer) -> serde_json::Value {
    let request = peer.request();
    let id = request["id"].as_str().unwrap_or_default().to_owned();
    peer.respond(
        "boot-v1",
        &id,
        &serde_json::json!({"id": id, "result": {"type": "ok"}}),
    );
    request
}

/// Every message the window sent before a marker input it sends now.
fn sent(
    peer: &mut MockPeer,
    view: &Entity<HerdrWindow>,
    cx: &mut VisualTestContext,
) -> Vec<ClientMessage> {
    view.read_with(cx, |_, _| {
        peer.client
            .handle
            .send_input(
                "boot-v1",
                "marker",
                [ClientPaneInputEvent::TextCommit("marker".into())],
            )
            .unwrap();
    });
    let mut messages = Vec::new();
    loop {
        match peer.receive() {
            ClientMessage::ClientShellPaneInput { pane_id, .. } if pane_id == "marker" => {
                return messages;
            }
            message => messages.push(message),
        }
    }
}

/// The viewports among `messages`.
fn viewports(messages: &[ClientMessage]) -> Vec<RegionViewport> {
    messages
        .iter()
        .filter_map(|message| match message {
            ClientMessage::EndpointControl { kind, data } if kind == VIEWPORT_KIND => {
                RegionViewport::decode(data).ok()
            }
            _ => None,
        })
        .collect()
}

/// The endpoint requests among `messages`, as JSON.
fn requests(messages: &[ClientMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter_map(|message| match message {
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                serde_json::from_str(request).ok()
            }
            _ => None,
        })
        .collect()
}
