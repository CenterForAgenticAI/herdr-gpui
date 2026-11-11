use super::links::{at, frame, link_modifier, shown};
use super::worker::until;
use crate::core_terminals::view::ToWindow;
use gpui::{Modifiers, MouseButton, TestAppContext};
use sheprd_backend::SurfaceUpdate;

#[gpui::test]
fn remote_file_hyperlink_keeps_raw_target_and_only_emits_on_click(cx: &mut TestAppContext) {
    let raw = "file:///remote/notes%20one.md";
    let (view, cx, backend, asked) = shown(cx, &["notes"], false);
    let mut screen = frame(40, &["notes"]);
    screen.hyperlinks.push(raw.into());
    for cell in &mut screen.cells[..5] {
        cell.hyperlink = Some(0);
    }
    backend.push(vec![SurfaceUpdate::Frame(screen)]);
    until(|| {
        cx.update(|window, cx| view.update(cx, |view, cx| view.poll(window, cx)))
            .then_some(())
    });
    cx.update(|_, cx| view.update(cx, |view, cx| view.set_remote(true, cx)));
    let point = at(&view, cx, 2, 0);
    cx.simulate_mouse_move(point, None, link_modifier());
    cx.run_until_parked();
    assert!(
        asked.borrow().is_empty(),
        "hover must not invoke an adapter"
    );
    cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
    assert!(asked.borrow().is_empty());
    cx.simulate_mouse_down(point, MouseButton::Left, link_modifier());
    cx.simulate_mouse_up(point, MouseButton::Left, link_modifier());
    let asks = asked.borrow();
    let [ToWindow::OpenLink(click)] = asks.as_slice() else {
        panic!("one click expected: {asks:?}")
    };
    assert_eq!(click.handler_url.as_deref(), Some(raw));
}
