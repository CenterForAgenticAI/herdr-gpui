//! Drawing the panel, and the viewport reports that follow what is drawn.
use super::*;

#[gpui::test]
fn without_the_capability_the_window_lays_out_as_before(cx: &mut TestAppContext) {
    let mut peer = MockPeer::new();
    let (view, cx) = window(&peer, cx);
    draw(cx);
    let before = cx.debug_bounds("terminal").unwrap();
    // A projection on a connection that did not negotiate regions.
    offer(&view, 1, region(36, true, true), false, cx);
    draw(cx);
    assert_eq!(cx.debug_bounds("desktop-region"), None);
    assert_eq!(cx.debug_bounds("terminal").unwrap(), before);
    let sent = sent(&mut peer, &view, cx);
    assert!(viewports(&sent).is_empty(), "{sent:?}");
}

#[gpui::test]
fn a_visible_region_is_drawn_beside_the_panes_and_reports_its_cells(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    draw(cx);
    let full = cx.debug_bounds("terminal").unwrap();
    project(&view, 1, region(36, true, false), cx);
    draw(cx);
    let cell_width = view.read_with(cx, |view, _| view.cell_width);
    let cell_height = view.read_with(cx, |view, _| view.config.terminal.line_height());
    let panel = cx.debug_bounds("desktop-region").unwrap();
    let terminal = cx.debug_bounds("desktop-region-terminal").unwrap();
    let panes = cx.debug_bounds("terminal").unwrap();
    assert!(
        (panel.size.width - px(36. * cell_width + BORDER)).abs() < px(0.5),
        "{panel:?}"
    );
    assert!(panes.right() <= panel.left(), "{panes:?} {panel:?}");
    assert!(
        panes.size.width < full.size.width,
        "the panes give up the room"
    );
    let sent = sent(&mut peer, &view, cx);
    let rows = (f32::from(terminal.size.height) / cell_height).floor() as u16;
    assert_eq!(
        viewports(&sent),
        [RegionViewport {
            region_id: "region_1".into(),
            cols: 36,
            rows,
        }]
    );
    // Drawn again unchanged, nothing more is reported.
    draw(cx);
    let again = super::sent(&mut peer, &view, cx);
    assert!(viewports(&again).is_empty(), "{again:?}");
}

#[gpui::test]
fn a_hidden_region_leaves_no_panel_and_reports_zero(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, true, false), cx);
    draw(cx);
    let shown = cx.debug_bounds("terminal").unwrap();
    sent(&mut peer, &view, cx);
    project(&view, 2, region(36, false, false), cx);
    draw(cx);
    let sent = sent(&mut peer, &view, cx);
    assert_eq!(
        viewports(&sent),
        [RegionViewport {
            region_id: "region_1".into(),
            cols: 0,
            rows: 0,
        }]
    );
    // Once it has slid out, the panes have the whole row again.
    std::thread::sleep(motion::LEAVE + std::time::Duration::from_millis(50));
    draw(cx);
    assert_eq!(cx.debug_bounds("desktop-region"), None);
    assert!(cx.debug_bounds("terminal").unwrap().size.width > shown.size.width);
}

#[gpui::test]
fn a_hostile_size_hides_the_panel_but_never_squeezes_the_panes(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    draw(cx);
    let full = cx.debug_bounds("terminal").unwrap();
    let mut huge = region(4000, true, true);
    huge.max_size = 4096;
    project(&view, 1, huge, cx);
    draw(cx);
    assert_eq!(cx.debug_bounds("desktop-region"), None);
    assert_eq!(cx.debug_bounds("terminal").unwrap(), full);
    let sent = sent(&mut peer, &view, cx);
    assert_eq!(
        viewports(&sent),
        [RegionViewport {
            region_id: "region_1".into(),
            cols: 0,
            rows: 0,
        }]
    );
    // Not drawn, so it takes no typed input though the daemon says focused.
    assert_eq!(
        view.read_with(cx, |view, _| view.region_input_target().map(str::to_owned)),
        None
    );
}
