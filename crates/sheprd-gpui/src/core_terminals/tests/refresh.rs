//! Two windows showing one herdr-core tab: each reads what the other saved
//! before saving its own change, so neither undoes the other's.

use crate::{
    browser::Location,
    core_terminals::{
        Host, TabPanes,
        panes::{Axis, Panes},
    },
};
use sheprd_backend::TerminalId;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

fn panes(names: &[&str]) -> Panes {
    let mut panes = Panes::single(id(names[0]));
    for pair in names.windows(2) {
        panes.split(&id(pair[0]), Axis::Row, id(pair[1])).unwrap();
    }
    panes
}

/// The tab as a window first reads it, holding `names`.
fn shown(names: &[&str]) -> TabPanes {
    let location = Location::core_terminal(panes(names), false);
    let mut shown = TabPanes::new(Host::Local, panes(names), false);
    shown.saved = Some(location);
    shown
}

#[test]
fn a_focus_change_keeps_a_split_another_window_saved() {
    let mut here = shown(&["t1", "t2"]);
    // This window moves the keyboard; another saves a third pane first.
    here.active = id("t2");
    here.unsaved = true;
    let there = Location::core_terminal(panes(&["t1", "t2", "t3"]), false);
    here.refresh(&there);
    assert_eq!(here.panes, panes(&["t1", "t2", "t3"]));
    assert_eq!(here.active, id("t2"), "this window's focus still stands");
    assert!(here.unsaved);
    assert_eq!(here.saved.as_ref(), Some(&there));
    let Location::CoreTerminal { panes: saved, .. } = here.location() else {
        panic!("a terminal tab");
    };
    assert!(saved.contains(&id("t3")), "the next save keeps the split");
}

#[test]
fn a_pane_gone_elsewhere_takes_the_focus_and_zoom_saved_there() {
    let mut here = shown(&["t1", "t2"]);
    here.active = id("t2");
    here.set_zoom(Some(id("t2")));
    here.unsaved = true;
    let mut there = Location::core_terminal(panes(&["t1", "t3"]), false);
    if let Location::CoreTerminal { active, .. } = &mut there {
        *active = Some(id("t3"));
    }
    here.refresh(&there);
    assert_eq!(here.active, id("t3"));
    assert!(!here.zoom.is_open(), "the zoomed pane went");
}

#[test]
fn focus_stays_where_this_windows_keyboard_is() {
    let mut here = shown(&["t1", "t2"]);
    let mut there = Location::core_terminal(panes(&["t1", "t2"]), false);
    if let Location::CoreTerminal { active, .. } = &mut there {
        *active = Some(id("t2"));
    }
    here.refresh(&there);
    assert_eq!(here.active, id("t1"));
    assert!(!here.unsaved);
}

#[test]
fn a_name_another_window_gave_is_read() {
    let mut here = shown(&["t1", "t2"]);
    let mut there = Location::core_terminal(panes(&["t1", "t2"]), false);
    if let Location::CoreTerminal { name, names, .. } = &mut there {
        *name = Some("build".into());
        names.insert(id("t2"), "tests".into());
    }
    here.refresh(&there);
    assert_eq!(here.name.as_deref(), Some("build"));
    assert_eq!(here.names.get(&id("t2")).map(String::as_str), Some("tests"));
}

#[test]
fn a_split_placed_here_while_another_window_saved_stays() {
    let mut here = shown(&["t1", "t2"]);
    // A split this window just placed, not saved yet.
    here.panes.split(&id("t2"), Axis::Row, id("t3")).unwrap();
    here.active = id("t3");
    here.unsaved = true;
    // Another window saved a focus change on the old layout meanwhile.
    let mut there = Location::core_terminal(panes(&["t1", "t2"]), false);
    if let Location::CoreTerminal { active, .. } = &mut there {
        *active = Some(id("t1"));
    }
    here.refresh(&there);
    assert_eq!(here.panes, panes(&["t1", "t2", "t3"]));
    assert_eq!(here.active, id("t3"));
    assert!(here.unsaved, "the split still saves");
}

#[test]
fn layouts_both_windows_changed_merge() {
    let mut here = shown(&["t1", "t2"]);
    here.panes.split(&id("t1"), Axis::Row, id("t3")).unwrap();
    here.unsaved = true;
    // The other window closed t2 and opened t4.
    let there = Location::core_terminal(panes(&["t1", "t4"]), false);
    here.refresh(&there);
    let mut kept: Vec<String> = here
        .panes
        .terminals()
        .iter()
        .map(ToString::to_string)
        .collect();
    kept.sort();
    assert_eq!(kept, ["t1", "t3", "t4"]);
}

#[test]
fn a_zoom_saved_on_another_pane_never_hides_the_one_in_use() {
    let mut here = shown(&["t1", "t2"]);
    let mut there = Location::core_terminal(panes(&["t1", "t2"]), false);
    if let Location::CoreTerminal { zoomed, .. } = &mut there {
        *zoomed = Some(id("t2"));
    }
    here.refresh(&there);
    assert_eq!(here.active, id("t1"));
    assert!(!here.zoom.is_open(), "t1 in use is not hidden behind t2");
    // On the pane in use, the other window's zoom is taken.
    let mut there = Location::core_terminal(panes(&["t1", "t2"]), false);
    if let Location::CoreTerminal { zoomed, .. } = &mut there {
        *zoomed = Some(id("t1"));
    }
    here.refresh(&there);
    assert!(here.zoom.is_open());
    assert_eq!(here.zoomed, Some(id("t1")));
}

#[test]
fn a_name_given_here_stays_when_another_window_saves() {
    let mut here = shown(&["t1", "t2"]);
    here.name = Some("build".into());
    here.unsaved = true;
    let mut there = Location::core_terminal(panes(&["t1", "t2"]), false);
    if let Location::CoreTerminal { active, .. } = &mut there {
        *active = Some(id("t2"));
    }
    here.refresh(&there);
    assert_eq!(here.name.as_deref(), Some("build"));
}
