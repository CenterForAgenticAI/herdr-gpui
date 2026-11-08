use super::*;
use crate::browser::TabId;
use core::prelude::v1::test;

/// The workspace's Herdr 2 tabs close once the move stopped their
/// terminals, and only then: a move that failed earlier leaves them working.
#[test]
fn herdr_2_tabs_close_once_their_terminals_stopped() {
    let tabs = vec![TabId::test(1), TabId::test(2)];
    let mut early = Teleport::start(source(), "feature".into(), Vec::new()).closing(tabs.clone());
    assert!(early.closed_tabs(false).is_empty());
    assert_eq!(early.closed_tabs(true), tabs);
    // Taken once.
    assert!(early.closed_tabs(true).is_empty());

    let mut late = Teleport::start(source(), "feature".into(), Vec::new()).closing(tabs.clone());
    late.retired = true;
    assert_eq!(late.closed_tabs(false), tabs);
}
