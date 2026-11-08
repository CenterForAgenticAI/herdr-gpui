use super::{Action, Bridge, Data, Feed, action};
use async_channel::bounded;

#[test]
fn each_message_becomes_its_action() {
    let of = |key: &'static str| action(&Data::map([(key, "x".into())]));
    assert_eq!(of("command"), Some(Action::Command("x".into())));
    assert_eq!(
        of("focus_workspace"),
        Some(Action::FocusWorkspace("x".into()))
    );
    assert_eq!(of("focus_pane"), Some(Action::FocusPane("x".into())));
    assert_eq!(of("panel"), Some(Action::Panel("x".into())));
}

#[test]
fn a_message_asking_for_nothing_known_is_no_action() {
    assert_eq!(action(&Data::map([("other", "x".into())])), None);
    assert_eq!(action(&Data::map([("command", 3.into())])), None);
}

#[test]
fn a_full_queue_drops_an_action_without_blocking() {
    let (actions, received) = bounded(1);
    let bridge = Bridge {
        actions,
        feed: Feed::default(),
    };
    let message = || Data::map([("command", "x".into())]);
    for _ in 0..3 {
        if let Some(action) = action(&message()) {
            let _ = bridge.actions.try_send(action);
        }
    }
    assert_eq!(received.len(), 1);
}

#[test]
fn state_a_provider_not_started_yet_is_offered_again() {
    let feed = Feed::default();
    let mut last = None;
    assert!(!feed.offer(&mut last, serde_json::json!({ "a": 1 })));
    assert_eq!(last, None);
}

#[test]
fn state_already_published_is_not_published_again() {
    let feed = Feed::default();
    let state = serde_json::json!({ "a": 1 });
    let mut last = Some(state.clone());
    assert!(feed.offer(&mut last, state.clone()));
    assert_eq!(last, Some(state));
}
