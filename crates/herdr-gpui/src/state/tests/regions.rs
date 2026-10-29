use super::*;
use herdr_client::protocol::{
    endpoint::EndpointServerWelcome,
    region_surface::{CAPABILITY, ClientRegion, RegionProjection, RegionSurface},
};

fn welcome(capabilities: &[&str]) -> ClientEvent {
    ClientEvent::Connected(EndpointServerWelcome {
        generation: 1,
        server_version: "test".into(),
        snapshot_codec: String::new(),
        surface_codec: String::new(),
        input_codec: String::new(),
        blob_codec: String::new(),
        methods: Vec::new(),
        capabilities: capabilities.iter().map(|value| (*value).into()).collect(),
        error: None,
    })
}

fn projection(boot: &str, revision: u64, title: &str) -> ClientEvent {
    ClientEvent::RegionProjection(Arc::new(RegionProjection {
        boot_id: boot.into(),
        revision,
        regions: vec![ClientRegion {
            region_id: "region_1".into(),
            terminal_id: "term_1".into(),
            plugin_id: "demo".into(),
            entrypoint: "board".into(),
            title: title.into(),
            edge: "right".into(),
            scope: "session".into(),
            size: 36,
            min_size: 12,
            max_size: 60,
            visible: true,
            focused: false,
            mouse_reporting: false,
        }],
        focused: None,
    }))
}

fn frame() -> ClientEvent {
    ClientEvent::RegionSurface(Arc::new(RegionSurface {
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
    }))
}

fn title(state: &LiveState) -> Option<&str> {
    state.regions.right().map(|region| region.title.as_str())
}

#[test]
fn the_region_capability_follows_each_welcome() {
    let mut state = LiveState::default();
    state.apply(welcome(&[CAPABILITY]));
    assert!(state.supports_regions);
    state.apply(projection("b", 1, "board"));
    assert!(state.regions.right().is_some());
    // A reconnect to a daemon without regions forgets them and their layout.
    state.apply(welcome(&[]));
    assert!(!state.supports_regions);
    assert!(state.regions.right().is_none());
}

#[test]
fn a_stale_projection_never_replaces_a_newer_one() {
    let mut state = LiveState::default();
    state.apply(welcome(&[CAPABILITY]));
    state.apply(projection("b", 2, "new"));
    state.dirty = false;
    state.apply(projection("b", 1, "old"));
    assert_eq!(title(&state), Some("new"));
    // Nothing changed, so nothing is redrawn.
    assert!(!state.dirty);
    state.apply(projection("b", 2, "same revision"));
    assert_eq!(title(&state), Some("new"));
    // A daemon that restarted counts from one again.
    state.apply(projection("c", 1, "rebooted"));
    assert_eq!(title(&state), Some("rebooted"));
    assert!(state.dirty);
}

#[test]
fn a_new_projection_redraws_the_window_but_a_frame_only_the_terminal() {
    let mut state = LiveState::default();
    state.apply(welcome(&[CAPABILITY]));
    let before = state.clone();
    state.apply(frame());
    assert!(before.only_surface_changed(&state));
    state.apply(projection("b", 1, "board"));
    assert!(!before.only_surface_changed(&state));
}

#[test]
fn disconnecting_drops_the_regions() {
    let mut state = LiveState::default();
    state.apply(welcome(&[CAPABILITY]));
    state.apply(projection("b", 1, "board"));
    state.apply(frame());
    state.apply(ClientEvent::Disconnected {
        reason: "closed".into(),
        ssh: None,
    });
    assert!(!state.supports_regions);
    assert!(state.regions.projection().is_none());
}
