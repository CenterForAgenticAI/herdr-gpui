//! Work commands from a herdr-core terminal in a workspace of herdr-core
//! tabs alone refuse as unsupported, whether Herdr runs beside it or not.

use super::*;
use crate::{
    browser::{Location, Store},
    core_terminals::panes::Panes,
};
use sheprd_backend::TerminalId;

#[gpui::test]
fn a_core_only_workspace_beside_herdr_is_unsupported(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        connected(&mut view);
        view
    });
    view.update(cx, |view, cx| {
        let scope = crate::browser::scope(&view.endpoints[0]);
        let id = TerminalId::new("t1").unwrap();
        let location = Location::core_terminal(Panes::single(id.clone()), false);
        // A workspace the app keeps, which Herdr does not list.
        let tab = Store::update(cx, |store| {
            store.open(scope.clone(), "kept-1", Some(location), None)
        })
        .unwrap();
        view.core_terminals
            .show_for_test(tab, Panes::single(id), false, Vec::new());
        view.core_terminals.place_for_test(tab, scope, "kept-1");
    });
    view.read_with(cx, |view, _| {
        let from_core = Caller {
            terminal_id: Some("t1".into()),
            ..caller(None)
        };
        let Err(response) = find(view, &from_core) else {
            panic!("a core-only workspace has no Herdr state");
        };
        assert_eq!(error_code(&response), Some(ErrorCode::Unsupported));
        // A Herdr workspace that is gone is still not found.
        let Err(response) = find(view, &caller(Some("w99"))) else {
            panic!("w99 is not shown");
        };
        assert_eq!(error_code(&response), Some(ErrorCode::WorkspaceNotFound));
    });
}
