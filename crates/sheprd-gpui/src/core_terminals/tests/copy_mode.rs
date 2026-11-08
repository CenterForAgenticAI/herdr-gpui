//! Copy mode and selections reaching into the history on a herdr-core view,
//! answered by its own emulator through the worker.

use super::{
    history::{point, requests, shown},
    worker::until,
};
use gpui::{Modifiers, MouseButton, point as at, px};
use sheprd_backend::{CopyMotion, History, HistoryAnswer, ScrollMetrics, SurfaceUpdate};

#[gpui::test]
fn copy_mode_walks_the_history_and_copies_what_it_marked(cx: &mut gpui::TestAppContext) {
    let (view, cx, backend) = shown(cx);
    backend.answers.lock().unwrap().extend([
        HistoryAnswer::Point(point(9, 4)),
        HistoryAnswer::Text("ab".into()),
    ]);
    cx.update(|window, cx| view.update(cx, |view, cx| view.enter_copy_mode(window, cx)));
    // With no cursor shown, copy mode starts on the last row.
    cx.simulate_keystrokes("w");
    assert_eq!(
        requests(&backend, 1),
        vec![History::Motion {
            cursor: point(9, 0),
            motion: CopyMotion::NextWordStart,
        }]
    );
    // Keys typed while the motion is out wait for its answer.
    cx.simulate_keystrokes("v l y");
    let read = until(|| {
        cx.update(|window, cx| view.update(cx, |view, cx| view.poll(window, cx)));
        requests(&backend, 1).get(1).cloned()
    });
    assert_eq!(
        read,
        History::Read {
            anchor: point(9, 4),
            cursor: point(9, 5),
        }
    );
    // Leaving goes back to where the view was.
    assert_eq!(requests(&backend, 3)[2], History::ScrollTo { offset: 0 });
    let copied = until(|| {
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.poll(window, cx));
            cx.read_from_clipboard().and_then(|item| item.text())
        })
    });
    assert_eq!(copied, "ab");
    cx.update(|_, cx| assert!(!view.read(cx).copy_mode_active()));
}

#[gpui::test]
fn a_selection_reaching_past_the_screen_is_read_from_the_history(cx: &mut gpui::TestAppContext) {
    let (view, cx, backend) = shown(cx);
    cx.run_until_parked();
    let bounds = cx.update(|_, cx| view.read(cx).bounds()).unwrap();
    let cell =
        |column: f32, row: f32| bounds.origin + at(px(8.5 * column + 1.), px(17. * row + 1.));
    // Press on the bottom row, history row 9.
    cx.simulate_mouse_down(cell(2., 2.), MouseButton::Left, Modifiers::none());
    // Scrolled up five rows, the top row is history row 2.
    backend.push(vec![SurfaceUpdate::Scroll {
        metrics: ScrollMetrics {
            offset: 5,
            history: 7,
            rows: 3,
        },
        revision: 4,
    }]);
    until(|| {
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.poll(window, cx);
                (view.top() == 2).then_some(())
            })
        })
    });
    cx.simulate_mouse_move(cell(1., 0.), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(cell(1., 0.), MouseButton::Left, Modifiers::none());
    cx.update(|_, cx| view.update(cx, |view, cx| assert!(view.copy_selection(cx))));
    assert_eq!(
        requests(&backend, 1),
        vec![History::Read {
            anchor: point(2, 1),
            cursor: point(9, 2),
        }]
    );
}

#[gpui::test]
fn a_drag_held_above_the_screen_scrolls_the_history(cx: &mut gpui::TestAppContext) {
    let (view, cx, backend) = shown(cx);
    cx.run_until_parked();
    let bounds = cx.update(|_, cx| view.read(cx).bounds()).unwrap();
    let inside = bounds.origin + at(px(5.), px(5.));
    cx.simulate_mouse_down(inside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        bounds.origin + at(px(5.), px(-20.)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    cx.update(|window, cx| view.update(cx, |view, cx| view.poll(window, cx)));
    assert_eq!(requests(&backend, 1), vec![History::ScrollTo { offset: 1 }]);
}
