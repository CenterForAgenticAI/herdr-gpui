use super::{Attention, Heard, News};
use sheprd_backend::{AgentStatus, TerminalId};
use std::time::Instant;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

#[test]
fn only_a_change_into_blocked_or_out_of_a_turn_is_news() {
    use AgentStatus::*;
    assert_eq!(News::of(None, Blocked), None, "first heard of");
    assert_eq!(News::of(Some(Working), Blocked), Some(News::NeedsYou));
    assert_eq!(News::of(Some(Idle), Blocked), Some(News::NeedsYou));
    assert_eq!(News::of(Some(Blocked), Blocked), None);
    assert_eq!(News::of(Some(Working), Done), Some(News::Finished));
    assert_eq!(News::of(Some(Working), Idle), Some(News::Finished));
    assert_eq!(News::of(Some(Idle), Done), None);
    assert_eq!(News::of(Some(Blocked), Working), None);
    assert_eq!(News::of(Some(Working), Unknown), None);
}

#[test]
fn news_must_still_hold_after_the_delay() {
    assert!(News::NeedsYou.holds(AgentStatus::Blocked));
    assert!(!News::NeedsYou.holds(AgentStatus::Working));
    assert!(News::Finished.holds(AgentStatus::Idle));
    assert!(!News::Finished.holds(AgentStatus::Working));
}

#[test]
fn a_change_is_news_once_across_windows() {
    let mut heard = Heard::default();
    let t1 = id("t1");
    let known = |_: &TerminalId| true;
    assert!(heard.hear([(&t1, AgentStatus::Working)], known).is_empty());
    assert_eq!(
        heard.hear([(&t1, AgentStatus::Blocked)], known),
        vec![(t1.clone(), News::NeedsYou)]
    );
    // A second window lists the same core a moment later.
    assert!(heard.hear([(&t1, AgentStatus::Blocked)], known).is_empty());
}

#[test]
fn terminals_the_core_dropped_are_forgotten() {
    let mut heard = Heard::default();
    let t1 = id("t1");
    heard.hear([(&t1, AgentStatus::Working)], |_| true);
    heard.hear([], |_| false);
    // Heard of afresh, so a blocked agent reusing the address is quiet.
    assert!(
        heard
            .hear([(&t1, AgentStatus::Blocked)], |_| true)
            .is_empty()
    );
}

#[test]
fn a_newer_change_replaces_the_one_waiting() {
    let mut attention = Attention::default();
    let now = Instant::now();
    attention.push(now, id("t1"), News::NeedsYou);
    attention.push(now, id("t1"), News::Finished);
    attention.push(now, id("t2"), News::Finished);
    let due: Vec<_> = attention
        .due
        .iter()
        .map(|(_, t, n)| (t.as_str(), *n))
        .collect();
    assert_eq!(due, [("t1", News::Finished), ("t2", News::Finished)]);
}
