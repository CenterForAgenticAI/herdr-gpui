use super::*;
use crate::protocol::{CellData, FrameData};

fn region(id: &str, edge: &str, visible: bool) -> ClientRegion {
    ClientRegion {
        region_id: id.into(),
        terminal_id: format!("term_{id}"),
        plugin_id: "board".into(),
        entrypoint: "board".into(),
        title: "Board".into(),
        edge: edge.into(),
        scope: "session".into(),
        size: 30,
        min_size: 10,
        max_size: 60,
        visible,
        focused: false,
        mouse_reporting: false,
    }
}

fn projection(boot: &str, revision: u64, regions: Vec<ClientRegion>) -> Arc<RegionProjection> {
    Arc::new(RegionProjection {
        boot_id: boot.into(),
        revision,
        regions,
        focused: None,
    })
}

fn surface(region: &str, terminal: &str, symbol: &str) -> Arc<RegionSurface> {
    Arc::new(RegionSurface {
        region_id: region.into(),
        terminal_id: terminal.into(),
        frame: FrameData {
            cells: vec![CellData {
                symbol: symbol.into(),
                fg: 0,
                bg: 0,
                modifier: 0,
                skip: false,
                hyperlink: None,
            }],
            width: 1,
            height: 1,
            cursor: None,
            hyperlinks: vec![],
            graphics: vec![],
        },
    })
}

fn symbol(state: &RegionState, region: &ClientRegion) -> Option<String> {
    state
        .frame(region)
        .map(|frame| frame.cells[0].symbol.clone())
}

#[test]
fn a_stale_projection_never_replaces_a_newer_one() {
    let mut state = RegionState::default();
    assert!(state.apply_projection(projection("b", 5, vec![region("a", "right", true)])));
    for stale in [4, 5] {
        assert!(!state.apply_projection(projection("b", stale, vec![])));
        assert_eq!(state.projection().unwrap().revision, 5);
        assert!(state.right().is_some());
    }
    assert!(state.apply_projection(projection("b", 6, vec![])));
    assert!(state.right().is_none());
}

#[test]
fn a_new_boot_resets_the_projection_and_its_frames() {
    let mut state = RegionState::default();
    let right = region("a", "right", true);
    state.apply_projection(projection("old", 9, vec![right.clone()]));
    state.apply_surface(surface("a", "term_a", "x"));
    assert_eq!(symbol(&state, &right).as_deref(), Some("x"));
    // A lower revision from a new boot is not stale: revisions restart.
    assert!(state.apply_projection(projection("new", 1, vec![right.clone()])));
    assert_eq!(state.projection().unwrap().boot_id, "new");
    assert_eq!(symbol(&state, &right), None);
}

#[test]
fn frames_follow_their_region_and_terminal() {
    let mut state = RegionState::default();
    // A frame that overtakes its projection is kept.
    state.apply_surface(surface("a", "term_a", "early"));
    let right = region("a", "right", true);
    state.apply_projection(projection("b", 1, vec![right.clone()]));
    assert_eq!(symbol(&state, &right).as_deref(), Some("early"));
    state.apply_surface(surface("a", "term_a", "late"));
    assert_eq!(symbol(&state, &right).as_deref(), Some("late"));
    // A frame from another terminal is never drawn as this region.
    state.apply_surface(surface("a", "term_other", "spoof"));
    assert_eq!(symbol(&state, &right), None);
    // Hiding a region drops its frame, so it never shows stale on return.
    state.apply_surface(surface("a", "term_a", "x"));
    state.apply_projection(projection("b", 2, vec![region("a", "right", false)]));
    assert_eq!(symbol(&state, &right), None);
}

#[test]
fn frames_are_bounded() {
    let mut state = RegionState::default();
    let named = region("keep", "right", true);
    state.apply_projection(projection("b", 1, vec![named.clone()]));
    state.apply_surface(surface("keep", "term_keep", "k"));
    for index in 0..(MAX_SURFACES * 3) {
        state.apply_surface(surface(&format!("r{index}"), "t", "x"));
        assert!(state.surfaces.len() <= MAX_SURFACES);
    }
    // Frames of regions the projection names survive a flood of others.
    assert_eq!(symbol(&state, &named).as_deref(), Some("k"));
}

#[test]
fn only_the_visible_right_region_takes_keys_and_only_while_focused() {
    let mut state = RegionState::default();
    let mut left = region("l", "left", true);
    left.focused = true;
    let mut right = region("r", "right", true);
    state.apply_projection(projection("b", 1, vec![left.clone(), right.clone()]));
    assert_eq!(state.right().unwrap().region_id, "r");
    assert_eq!(state.focused_terminal(), None);
    right.focused = true;
    state.apply_projection(projection("b", 2, vec![left.clone(), right.clone()]));
    assert_eq!(state.focused_terminal(), Some("term_r"));
    // `focused` on the projection works as well as the region's own flag.
    right.focused = false;
    let mut by_id = (*projection("b", 3, vec![right.clone()])).clone();
    by_id.focused = Some("r".into());
    state.apply_projection(Arc::new(by_id));
    assert_eq!(state.focused_terminal(), Some("term_r"));
    // Hidden, it takes nothing even if the projection still names it.
    right.visible = false;
    right.focused = true;
    let mut hidden = (*projection("b", 4, vec![right])).clone();
    hidden.focused = Some("r".into());
    state.apply_projection(Arc::new(hidden));
    assert_eq!(state.focused_terminal(), None);
    assert!(state.visible_right().is_none());
}
