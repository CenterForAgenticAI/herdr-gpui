use super::*;
use crate::review::view::review_recipient;

#[test]
fn a_review_of_a_herdr_2_pane_sends_to_no_herdr_agent() {
    let shown = snapshot(base(
        Some("w1:p1"),
        vec![agent("w1:p1", "w1", Some("Claude Code"))],
    ));
    // Herdr's agent works in the hidden workspace, maybe another checkout.
    assert_eq!(review_recipient(Some(&shown), true), None);
    // Without a herdr-core pane shown, Herdr's focused agent receives them.
    assert_eq!(
        review_recipient(Some(&shown), false).map(|agent| agent.pane_id),
        Some("w1:p1".into())
    );
    assert_eq!(review_recipient(None, false), None);
}
