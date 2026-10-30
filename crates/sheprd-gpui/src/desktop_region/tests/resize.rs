//! Dragging the panel's edge asks the daemon for a new size, held to the
//! region's bounds, and draws it until the daemon answers.
use super::*;
use gpui::{Modifiers, MouseButton, point};

#[gpui::test]
fn dragging_the_edge_asks_for_a_size_within_the_regions_bounds(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, true, false), cx);
    draw(cx);
    let cell_width = view.read_with(cx, |view, _| view.cell_width);
    let edge = cx.debug_bounds("desktop-region-resize").unwrap();
    let start = edge.center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    let wider = point(start.x - px(10. * cell_width), start.y);
    cx.simulate_mouse_move(
        point(start.x - px(4.), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(wider, MouseButton::Left, Modifiers::default());
    // The small first step asks for its own size; the last one wins.
    let request = loop {
        let request = answered(&mut peer);
        assert_eq!(request["method"], "region.resize");
        if request["params"]["size"] == 46 {
            break request;
        }
    };
    assert_eq!(request["params"]["region_id"], "region_1");
    draw(cx);
    let panel = cx.debug_bounds("desktop-region").unwrap();
    assert!(
        (panel.size.width - px(46. * cell_width + BORDER)).abs() < px(0.5),
        "drawn at once"
    );
    // Far past the region's maximum, the request stops at it.
    let far = point(start.x - px(400. * cell_width), start.y);
    cx.simulate_mouse_move(far, MouseButton::Left, Modifiers::default());
    let request = answered(&mut peer);
    assert_eq!(request["method"], "region.resize");
    assert_eq!(request["params"]["size"], 60);
    cx.simulate_mouse_up(far, MouseButton::Left, Modifiers::default());
    // Released, the dragged size stays drawn until a newer projection.
    draw(cx);
    let held = cx.debug_bounds("desktop-region").unwrap();
    assert!(
        (held.size.width - px(60. * cell_width + BORDER)).abs() < px(0.5),
        "{held:?}"
    );
    // The daemon clamped it to 50; its projection wins.
    project(&view, 2, region(50, true, false), cx);
    draw(cx);
    let clamped = cx.debug_bounds("desktop-region").unwrap();
    assert!(
        (clamped.size.width - px(50. * cell_width + BORDER)).abs() < px(0.5),
        "{clamped:?}"
    );
}
