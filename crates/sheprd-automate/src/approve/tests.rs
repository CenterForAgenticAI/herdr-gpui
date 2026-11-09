use super::Pane;

#[test]
fn a_herdr_2_terminal_is_the_callers_pane_before_an_inherited_herdr_one() {
    assert_eq!(Pane::of(Some("w1p1"), None), Some(Pane::Herdr("w1p1")));
    assert_eq!(Pane::of(None, Some("t3")), Some(Pane::Core("t3")));
    // A terminal Sheprd starts clears HERDR_PANE_ID; one started from a
    // Herdr pane inherits it, and runs in the terminal all the same.
    assert_eq!(Pane::of(Some(""), Some("t3")), Some(Pane::Core("t3")));
    assert_eq!(Pane::of(Some("w1p1"), Some("t3")), Some(Pane::Core("t3")));
    assert_eq!(Pane::of(Some(""), Some("")), None);
    assert_eq!(Pane::of(None, None), None);
}
