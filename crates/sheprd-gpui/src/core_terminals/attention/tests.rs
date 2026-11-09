use super::{Heard, News};
use gpui::WindowId;
use sheprd_backend::{AgentStatus, TerminalId};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

const DELAY: Duration = Duration::from_secs(1);

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

fn after(start: Instant, millis: u64) -> Instant {
    start + Duration::from_millis(millis)
}

fn names(news: &[(TerminalId, News)]) -> Vec<(&str, News)> {
    news.iter().map(|(t, n)| (t.as_str(), *n)).collect()
}

/// `heard` with `t1` known as `before`, then as `now`, the news queued at
/// `start`.
fn heard_change(start: Instant, before: AgentStatus, now: AgentStatus) -> Heard {
    let mut heard = Heard::default();
    let t1 = id("t1");
    heard.hear(start, [(&t1, before)], |_| true);
    for (terminal, news) in heard.hear(after(start, 10), [(&t1, now)], |_| true) {
        heard.push(start, terminal, news);
    }
    heard
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
    let start = Instant::now();
    let mut heard = Heard::default();
    let t1 = id("t1");
    let known = |_: &TerminalId| true;
    assert!(
        heard
            .hear(start, [(&t1, AgentStatus::Working)], known)
            .is_empty()
    );
    assert_eq!(
        names(&heard.hear(after(start, 10), [(&t1, AgentStatus::Blocked)], known)),
        [("t1", News::NeedsYou)]
    );
    // A second window lists the same core a moment later.
    assert!(
        heard
            .hear(after(start, 20), [(&t1, AgentStatus::Blocked)], known)
            .is_empty()
    );
}

#[test]
fn a_list_older_than_one_taken_is_not_news() {
    let start = Instant::now();
    let mut heard = Heard::default();
    let t1 = id("t1");
    let known = |_: &TerminalId| true;
    heard.hear(start, [(&t1, AgentStatus::Working)], known);
    heard.hear(after(start, 20), [(&t1, AgentStatus::Blocked)], known);
    // Another window's slower read, asked for before the newest, arrives
    // late: it neither undoes the block nor makes the next one news again.
    assert!(
        heard
            .hear(after(start, 10), [(&t1, AgentStatus::Working)], known)
            .is_empty()
    );
    assert!(
        heard
            .hear(after(start, 30), [(&t1, AgentStatus::Blocked)], known)
            .is_empty()
    );
}

#[test]
fn terminals_the_core_dropped_are_forgotten() {
    let start = Instant::now();
    let mut heard = Heard::default();
    let t1 = id("t1");
    heard.hear(start, [(&t1, AgentStatus::Working)], |_| true);
    heard.hear(after(start, 10), [], |_| false);
    // Heard of afresh, so a blocked agent reusing the address is quiet.
    assert!(
        heard
            .hear(after(start, 20), [(&t1, AgentStatus::Blocked)], |_| true)
            .is_empty()
    );
}

#[test]
fn a_newer_change_replaces_the_one_waiting() {
    let mut heard = Heard::default();
    let now = Instant::now();
    heard.push(now, id("t1"), News::NeedsYou);
    heard.push(now, id("t1"), News::Finished);
    heard.push(now, id("t2"), News::Finished);
    let due: Vec<_> = heard.due.iter().map(|(_, t, n)| (t.as_str(), *n)).collect();
    assert_eq!(due, [("t1", News::Finished), ("t2", News::Finished)]);
}

#[test]
fn news_waits_out_the_delay_and_must_still_hold() {
    let start = Instant::now();
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Blocked);
    assert!(
        heard.take_due(after(start, 500), DELAY).is_empty(),
        "too soon"
    );
    assert_eq!(
        names(&heard.take_due(after(start, 1_000), DELAY)),
        [("t1", News::NeedsYou)]
    );
    assert!(heard.due.is_empty());
    // Answered before the delay was over.
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Blocked);
    heard.hear(
        after(start, 500),
        [(&id("t1"), AgentStatus::Working)],
        |_| true,
    );
    assert!(heard.take_due(after(start, 1_000), DELAY).is_empty());
}

#[test]
fn news_no_window_could_play_in_time_is_dropped() {
    let start = Instant::now();
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Blocked);
    assert!(heard.take_due(after(start, 10_000), DELAY).is_empty());
    assert!(heard.due.is_empty());
}

#[test]
fn news_waits_app_wide_for_whichever_window_plays_it() {
    // Heard in one window, which then closed: the news is not the window's.
    let start = Instant::now();
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Done);
    heard.show(WindowId::from(1), HashSet::new(), |_| false);
    assert_eq!(
        names(&heard.take_due(after(start, 1_000), DELAY)),
        [("t1", News::Finished)]
    );
}

#[test]
fn a_finished_turn_shown_in_the_active_window_is_quiet() {
    let start = Instant::now();
    let shown = HashSet::from([id("t1")]);
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Done);
    heard.show(WindowId::from(1), shown.clone(), |_| true);
    assert!(heard.take_due(after(start, 1_000), DELAY).is_empty());
    // A request still sounds.
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Blocked);
    heard.show(WindowId::from(1), shown.clone(), |_| true);
    assert_eq!(heard.take_due(after(start, 1_000), DELAY).len(), 1);
    // The window that showed it closed, or no longer shows it.
    let mut heard = heard_change(start, AgentStatus::Working, AgentStatus::Done);
    heard.show(WindowId::from(1), shown, |_| true);
    heard.show(WindowId::from(2), HashSet::new(), |window| {
        window != WindowId::from(1)
    });
    assert_eq!(heard.take_due(after(start, 1_000), DELAY).len(), 1);
}
