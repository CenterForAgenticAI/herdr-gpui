use super::*;
use crate::teleport::job::Landed;
use core::prelude::v1::test;

#[gpui::test]
fn a_closed_arrival_tab_is_not_reopened_by_recovery(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let scope = cx.update(|_, cx| view.read(cx).endpoint_scope("local"));
    let terminal = TerminalId::new("t-closed-arrival").unwrap();
    let outcome = Outcome {
        source_retired: false,
        endpoint_id: "local".into(),
        workspace: Landed::New {
            checkout: "/recovered/closed".into(),
            label: None,
        },
        repo_key: "/recovered/.git".into(),
        branch: "closed".into(),
        warnings: Vec::new(),
        core_tabs: vec![Panes::single(terminal.clone())],
    };
    // Unique to this test: adoptions are process-wide.
    let path = PathBuf::from("/nonexistent/teleport-arrivals/closed-arrival.json");
    let receipt = || Receipt::new(scope.as_str(), &outcome);
    let held = |cx: &App| -> Vec<crate::browser::TabId> {
        cx.global::<Store>()
            .in_scope(&scope)
            .filter(|tab| matches!(&tab.location, Some(Location::CoreTerminal { panes, .. }) if panes.contains(&terminal)))
            .map(|tab| tab.id)
            .collect()
    };
    let recover = |view: &Entity<SheprdWindow>, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            cx.default_global::<Recovery>()
                .pending
                .lock()
                .unwrap()
                .push((path.clone(), receipt()));
            view.update(cx, |view, cx| view.recover_teleport_arrivals(window, cx));
        });
    };

    recover(&view, cx);
    let tabs = cx.update(|_, cx| held(cx));
    assert_eq!(tabs.len(), 1, "recovery opens the arrival once");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.close_browser_tab(tabs[0], window, cx));
    });

    // The receipt is still on disk until the saved tabs catch up.
    recover(&view, cx);
    assert!(
        cx.update(|_, cx| held(cx)).is_empty(),
        "a closed tab stays closed"
    );
    assert_eq!(
        adopted().get(&path).map(|adopted| adopted.closed.clone()),
        Some(vec![terminal.clone()]),
        "its receipt no longer waits for the closed tab"
    );
    adopted().remove(&path);
}
