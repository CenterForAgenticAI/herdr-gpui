//! A phone's split or zoom of a Herdr 2 tab, made by the window holding it.

use crate::{
    browser::TabId,
    core_terminals::{
        CoreTerminals, Purpose, TabPanes,
        panes::{Axis, Panes},
        phone::Change,
    },
};
use sheprd_backend::TerminalId;
use sheprd_remote_control::control::{
    changes::AskId,
    layout::{Answer, Ask, Failure, Op},
};

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

/// Tab 1 drives t1 and t2; tab 2 mirrors t3.
fn terminals() -> CoreTerminals {
    let mut terminals = CoreTerminals::default();
    let mut panes = Panes::single(id("t1"));
    panes.split(&id("t1"), Axis::Row, id("t2")).unwrap();
    terminals
        .tabs
        .insert(TabId::test(1), TabPanes::new(panes, false));
    terminals
        .tabs
        .insert(TabId::test(2), TabPanes::new(Panes::single(id("t3")), true));
    terminals
}

fn ask(tab: u64, terminal: &str, op: Op) -> Ask {
    Ask {
        id: AskId(1),
        tab,
        terminal: terminal.to_owned(),
        op,
    }
}

#[test]
fn a_split_starts_a_shell_beside_the_phones_pane() {
    let mut terminals = terminals();
    let Some(Change::Split(Purpose::Split { tab, at, axis })) =
        terminals.phone_change(&ask(1, "t2", Op::SplitDown))
    else {
        panic!("a split");
    };
    assert_eq!((tab, at, axis), (TabId::test(1), id("t2"), Axis::Column));
}

#[test]
fn a_zoom_fills_the_tab_and_a_second_shows_the_split() {
    let mut terminals = terminals();
    assert!(matches!(
        terminals.phone_change(&ask(1, "t2", Op::Zoom)),
        Some(Change::Answered(Answer::Done))
    ));
    assert!(terminals.tabs[&TabId::test(1)].zoom.is_open());
    terminals.phone_change(&ask(1, "t2", Op::Zoom));
    assert!(!terminals.tabs[&TabId::test(1)].zoom.is_open());
}

#[test]
fn a_mirror_never_splits_and_a_pane_elsewhere_is_gone() {
    let mut terminals = terminals();
    assert!(matches!(
        terminals.phone_change(&ask(2, "t3", Op::SplitRight)),
        Some(Change::Answered(Answer::Failed(Failure::Watching)))
    ));
    assert!(matches!(
        terminals.phone_change(&ask(1, "t3", Op::SplitRight)),
        Some(Change::Answered(Answer::Failed(Failure::Gone)))
    ));
    // Another window may hold a tab this one does not.
    assert!(terminals.phone_change(&ask(9, "t1", Op::Zoom)).is_none());
}
