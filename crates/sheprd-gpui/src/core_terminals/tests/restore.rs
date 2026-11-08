//! A herdr-core tab comes back as it was left: the pane in use and the
//! zoomed pane are saved with its layout.

use crate::{
    browser::Location,
    core_terminals::{
        TabPanes,
        panes::{Axis, Panes},
    },
};
use sheprd_backend::TerminalId;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

fn split() -> Panes {
    let mut panes = Panes::single(id("t1"));
    panes.split(&id("t1"), Axis::Row, id("t2")).unwrap();
    panes
}

#[test]
fn the_pane_in_use_and_the_zoom_save_and_come_back() {
    let mut shown = TabPanes::new(split(), false);
    shown.active = id("t2");
    shown.set_zoom(Some(id("t2")));
    let location = shown.location();
    let saved = serde_json::to_string(&location).unwrap();
    let read: Location = serde_json::from_str(&saved).unwrap();
    assert_eq!(read, location);
    let Location::CoreTerminal {
        panes,
        observe,
        active,
        zoomed,
    } = read
    else {
        panic!("a terminal tab");
    };
    let restored = TabPanes::restore(panes, observe, active.as_ref(), zoomed.as_ref());
    assert_eq!(restored.active, id("t2"));
    assert_eq!(restored.zoomed, Some(id("t2")));
    // Already zoomed as the window opens, not growing.
    let now = crate::motion::now();
    assert!(restored.zoom.is_open() && !restored.zoom.moving(now));
    assert_eq!(restored.zoom_at(now).map(|(pane, _)| pane), Some(&id("t2")));
}

#[test]
fn an_unzoomed_tab_saves_no_zoom() {
    let mut shown = TabPanes::new(split(), false);
    shown.set_zoom(Some(id("t1")));
    shown.set_zoom(None);
    let Location::CoreTerminal { zoomed, .. } = shown.location() else {
        panic!("a terminal tab");
    };
    assert_eq!(zoomed, None);
}

#[test]
fn panes_the_layout_no_longer_holds_are_ignored() {
    let restored = TabPanes::restore(split(), false, Some(&id("t9")), Some(&id("t9")));
    assert_eq!(restored.active, id("t1"));
    assert!(!restored.zoom.is_open());
    // One pane has nothing to zoom over.
    let single = TabPanes::restore(Panes::single(id("t1")), false, None, Some(&id("t1")));
    assert!(!single.zoom.is_open());
}

#[test]
fn a_tab_saved_without_them_starts_on_its_first_pane() {
    let saved = r#"{"kind":"core_terminal","panes":{"axis":"row","first":"t1","second":"t2"}}"#;
    let Location::CoreTerminal {
        panes,
        observe,
        active,
        zoomed,
    } = serde_json::from_str(saved).unwrap()
    else {
        panic!("a terminal tab");
    };
    let restored = TabPanes::restore(panes, observe, active.as_ref(), zoomed.as_ref());
    assert_eq!(restored.active, id("t1"));
    assert!(!restored.zoom.is_open());
}
