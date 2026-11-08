use super::*;
use crate::browser::TabId;
use core::prelude::v1::test;
use sheprd_backend::TerminalId;
use sheprd_session::panes::{Axis, Panes};

fn panes(id: &str) -> Panes {
    Panes::single(TerminalId::new(id).unwrap())
}

/// The workspace's Herdr 2 tabs close once the move stopped their
/// terminals, and only then: a move that failed earlier leaves them working.
#[test]
fn herdr_2_tabs_close_once_their_terminals_stopped() {
    let tabs = vec![(TabId::test(1), panes("t1")), (TabId::test(2), panes("t2"))];
    let unchanged = |tab: TabId| {
        tabs.iter()
            .find(|(id, _)| *id == tab)
            .map(|(_, p)| p.clone())
    };
    let ids = [TabId::test(1), TabId::test(2)];
    let mut early = Teleport::start(source(), "feature".into(), Vec::new()).closing(tabs.clone());
    assert!(early.closed_tabs(false, unchanged).is_empty());
    assert_eq!(early.closed_tabs(true, unchanged), ids);
    // Taken once.
    assert!(early.closed_tabs(true, unchanged).is_empty());

    let mut late = Teleport::start(source(), "feature".into(), Vec::new()).closing(tabs.clone());
    late.retired = true;
    assert_eq!(late.closed_tabs(false, unchanged), ids);
}

/// A tab split during the move holds a terminal that did not move, so it
/// stays open with it; so does one already closed.
#[test]
fn a_tab_changed_during_the_move_stays() {
    let tabs = vec![(TabId::test(1), panes("t1")), (TabId::test(2), panes("t2"))];
    let mut teleport = Teleport::start(source(), "feature".into(), Vec::new()).closing(tabs);
    let now = |tab: TabId| {
        (tab == TabId::test(1)).then(|| {
            let mut split = panes("t1");
            let t1 = TerminalId::new("t1").unwrap();
            split
                .split(&t1, Axis::Row, TerminalId::new("t5").unwrap())
                .unwrap();
            split
        })
    };
    assert!(teleport.closed_tabs(true, now).is_empty());
}
