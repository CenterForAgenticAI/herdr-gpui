//! Typed input goes to the region's terminal only while it is drawn and
//! focused; clicking a pane takes the keyboard back.
use super::*;

fn type_text(view: &Entity<HerdrWindow>, text: &str, cx: &mut VisualTestContext) -> bool {
    view.update(cx, |view, cx| {
        view.send(ClientPaneInputEvent::TextCommit(text.into()), cx)
    })
}

fn inputs(messages: &[ClientMessage]) -> Vec<(String, String)> {
    messages
        .iter()
        .filter_map(|message| match message {
            ClientMessage::ClientShellPopupInput {
                terminal_id,
                events,
            } => Some(("region".into(), format!("{terminal_id} {events:?}"))),
            ClientMessage::ClientShellPaneInput { pane_id, events } if pane_id != "marker" => {
                Some(("pane".into(), format!("{pane_id} {events:?}")))
            }
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn typing_reaches_the_region_only_while_it_is_focused(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, true, true), cx);
    draw(cx);
    settle(&view, cx);
    assert!(type_text(&view, "a", cx));
    let sent = sent(&mut peer, &view, cx);
    assert_eq!(
        inputs(&sent),
        [("region".into(), r#"term_1 [TextCommit("a")]"#.into())]
    );
    // The daemon moved focus back to the pane.
    project(&view, 2, region(36, true, false), cx);
    draw(cx);
    settle(&view, cx);
    assert!(type_text(&view, "b", cx));
    let sent = super::sent(&mut peer, &view, cx);
    assert_eq!(
        inputs(&sent),
        [("pane".into(), r#"w1:p1 [TextCommit("b")]"#.into())]
    );
}

#[gpui::test]
fn a_hidden_region_never_takes_input(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    // Focused, but hidden: a stale or hostile projection.
    project(&view, 1, region(36, false, true), cx);
    draw(cx);
    settle(&view, cx);
    assert!(type_text(&view, "c", cx));
    let sent = sent(&mut peer, &view, cx);
    assert_eq!(
        inputs(&sent),
        [("pane".into(), r#"w1:p1 [TextCommit("c")]"#.into())]
    );
}

#[gpui::test]
fn without_the_capability_input_stays_with_the_pane(cx: &mut TestAppContext) {
    let mut peer = MockPeer::new();
    let (view, cx) = window(&peer, cx);
    offer(&view, 1, region(36, true, true), false, cx);
    draw(cx);
    settle(&view, cx);
    assert!(type_text(&view, "d", cx));
    let sent = sent(&mut peer, &view, cx);
    assert_eq!(
        inputs(&sent),
        [("pane".into(), r#"w1:p1 [TextCommit("d")]"#.into())]
    );
}

#[gpui::test]
fn clicking_the_region_focuses_it_and_clicking_the_pane_takes_it_back(cx: &mut TestAppContext) {
    let mut peer = peer();
    let (view, cx) = window(&peer, cx);
    project(&view, 1, region(36, true, false), cx);
    draw(cx);
    let terminal = cx.debug_bounds("desktop-region-terminal").unwrap();
    cx.simulate_click(terminal.center(), gpui::Modifiers::default());
    let request = answered(&mut peer);
    assert_eq!(request["method"], "region.focus");
    assert_eq!(request["params"]["region_id"], "region_1");
    // Focused, a click on the focused pane asks for the pane again, which
    // clears the region's focus in the daemon.
    project(&view, 2, region(36, true, true), cx);
    draw(cx);
    settle(&view, cx);
    view.update(cx, |view, cx| view.focus_clicked_pane("w1:p1", cx));
    let request = answered(&mut peer);
    assert_eq!(request["method"], "pane.focus");
    assert_eq!(request["params"]["pane_id"], "w1:p1");
}
