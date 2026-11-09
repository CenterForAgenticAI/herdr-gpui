//! Kills that did not reach their core are the app's, not one window's, so
//! a window closed before its core answers leaves them for another.

use super::commands::{id, window_with_split};
use crate::core_terminals::{CoreTerminals, Host};
use gpui::TestAppContext;

#[gpui::test]
fn a_kill_left_by_one_window_waits_for_any_other(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2"]);
    // Closed while disconnected: the kill waits for the core.
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.close_core_pane(tab, id("t2"), window, cx)
        })
    });
    // A window opened afterwards, or one already open, holds the same list,
    // so the kill outlives the window that closed the pane.
    let other = cx.update(|_, cx| CoreTerminals::shared(cx));
    let unended = other.unended.lock().unwrap().clone();
    assert_eq!(unended, [(Host::Local, id("t2"))]);
    let theirs = cx.update(|_, cx| std::sync::Arc::clone(&view.read(cx).core_terminals.unended));
    assert!(std::sync::Arc::ptr_eq(&theirs, &other.unended));
}
