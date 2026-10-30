//! The toggle and focus commands, and the header's hide button.
use super::*;
use herdr_commands::Command;

fn run(view: &Entity<HerdrWindow>, command: Command, cx: &mut VisualTestContext) {
    cx.update(|window, cx| view.update(cx, |view, cx| view.command(command, window, cx)));
}

#[gpui::test]
fn toggling_hides_a_shown_region_and_shows_a_hidden_one(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, true, false), cx);
    run(&view, Command::ToggleDesktopRegion, cx);
    let request = answered(&mut peer);
    assert_eq!(request["method"], "region.hide");
    assert_eq!(request["params"]["region_id"], "region_1");
    project(&view, 2, region(36, false, false), cx);
    run(&view, Command::ToggleDesktopRegion, cx);
    assert_eq!(answered(&mut peer)["method"], "region.show");
}

#[gpui::test]
fn focusing_shows_a_hidden_region_first_and_a_second_press_returns(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, false, false), cx);
    run(&view, Command::FocusDesktopRegion, cx);
    assert_eq!(answered(&mut peer)["method"], "region.show");
    assert_eq!(answered(&mut peer)["method"], "region.focus");
    project(&view, 2, region(36, true, true), cx);
    // Returning focus is pane navigation, which waits for a settled surface.
    draw(cx);
    settle(&view, cx);
    run(&view, Command::FocusDesktopRegion, cx);
    let request = answered(&mut peer);
    assert_eq!(request["method"], "pane.focus");
    assert_eq!(request["params"]["pane_id"], "w1:p1");
}

#[gpui::test]
fn without_the_capability_the_commands_say_so_and_send_nothing(cx: &mut TestAppContext) {
    let mut peer = MockPeer::new();
    let (view, cx) = window(&peer, cx);
    run(&view, Command::ToggleDesktopRegion, cx);
    let flash = view.read_with(cx, |view, _| {
        view.flash.as_ref().map(|(flash, _)| flash.text.to_string())
    });
    assert_eq!(
        flash.as_deref(),
        Some(fl!("desktop-region-unsupported").as_str())
    );
    run(&view, Command::FocusDesktopRegion, cx);
    let sent = sent(&mut peer, &view, cx);
    assert!(requests(&sent).is_empty(), "{sent:?}");
}

#[gpui::test]
fn the_header_button_hides_the_region(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, true, false), cx);
    draw(cx);
    let hide = cx.debug_bounds("desktop-region-hide").unwrap();
    cx.simulate_click(hide.center(), gpui::Modifiers::default());
    let request = answered(&mut peer);
    assert_eq!(request["method"], "region.hide");
    assert_eq!(request["params"]["region_id"], "region_1");
}
