use super::*;

/// Desktop regions belong to the window's own connection. A group that needs
/// a connection of its own gets one that never asks for them.
#[gpui::test]
fn a_parked_connection_never_asks_for_regions(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    draw(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.command(Command::SplitEditor, window, cx);
            let slots = view.group_slots();
            terminals_side_by_side(view, slots[0].id, slots[1].id);
            view.live.supports_regions = true;
            view.reconcile_group_terminals(cx);
            let parked = &view.browser.terminals.parked;
            assert_eq!(parked.len(), 1);
            assert_eq!(parked[0].group, slots[0].id);
            assert!(!parked[0].connection.hello.regions);
        })
    });
}

/// Swapping would park the connection that holds the regions and promote
/// one that never asked for them. While regions are negotiated, using a
/// parked group moves the window's connection to its tab instead.
#[gpui::test]
fn using_a_parked_group_keeps_the_region_connection(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    draw(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.command(Command::SplitEditor, window, cx)
        })
    });
    draw(cx);
    let [left, right] = view.read_with(cx, |view, _| {
        let slots = view.group_slots();
        [slots[0].id, slots[1].id]
    });
    let peer = MockPeer::new();
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            terminals_side_by_side(view, left, right);
            view.live.supports_regions = true;
            view.endpoints[0].live.supports_regions = true;
            let mut on_t1 = snapshot(40);
            on_t1.focused_workspace_id = Some("w0".into());
            on_t1.focused_tab_id = Some("t1".into());
            let key = view.browser_key().unwrap();
            let target = view.endpoints[0].connection.target.clone();
            let parked = parked(left, key, target, &peer, on_t1);
            view.browser.terminals.parked.push(parked);
        })
    });
    draw(cx);
    let epoch = view.read_with(cx, |view, _| view.selection_epoch);
    cx.update(|window, cx| view.update(cx, |view, cx| view.activate_group(left, window, cx)));
    draw(cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.primary_group(), Some(left));
        // Nothing was traded: the window keeps its connection and its
        // projection, which still shows the tab it showed.
        assert!(view.live.supports_regions);
        assert_eq!(view.selection_epoch, epoch);
        assert_eq!(
            view.endpoints[0]
                .live
                .snapshot
                .as_ref()
                .and_then(|s| s.focused_tab_id.as_deref()),
            Some("t0")
        );
        // The left group's own connection is no longer needed.
        assert!(
            view.browser
                .terminals
                .parked
                .iter()
                .all(|parked| parked.group != left)
        );
    });
}
