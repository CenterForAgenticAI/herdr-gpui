//! A phone holds a Herdr 2 terminal at its size through the pane's shared
//! screen, and letting go hands the size back to the driver before.

use super::*;

fn next(fake: &Fake) -> Value {
    fake.attaches.recv_timeout(Duration::from_secs(2)).unwrap()
}

#[test]
fn a_hold_drives_at_the_phone_size_and_letting_go_observes_again() {
    let (source, fake) = fake();
    source.poll().unwrap();
    let hold = source.hold(&id("t1"), 46, 30).unwrap();
    let watching = next(&fake);
    assert_eq!(watching["observe"], true);
    assert_eq!(watching["history"], screen::HISTORY);
    let driving = next(&fake);
    assert!(driving.get("observe").is_none());
    assert!(driving.get("history").is_none(), "history is painted once");
    assert_eq!(
        (&driving["cols"], &driving["rows"]),
        (&json!(46), &json!(30))
    );

    hold.resize(50, 32).unwrap();
    let resized = fake.resizes.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(resized, (50, 32));
    assert!(
        fake.attaches.try_recv().is_err(),
        "a resize keeps the attach"
    );

    drop(hold);
    let observing = next(&fake);
    assert_eq!(observing["observe"], true);
    drop(source);
    fake.server.join().unwrap();
}

#[test]
fn the_latest_hold_wins_until_it_lets_go() {
    let (source, fake) = fake();
    source.poll().unwrap();
    let first = source.hold(&id("t1"), 46, 30).unwrap();
    next(&fake);
    next(&fake);
    let second = source.hold(&id("t1"), 60, 40).unwrap();
    assert_eq!(
        fake.resizes.recv_timeout(Duration::from_secs(2)).unwrap(),
        (60, 40)
    );
    drop(second);
    assert_eq!(
        fake.resizes.recv_timeout(Duration::from_secs(2)).unwrap(),
        (46, 30)
    );
    assert!(fake.attaches.try_recv().is_err(), "one hold still drives");
    drop(first);
    assert_eq!(next(&fake)["observe"], true);
    drop(source);
    fake.server.join().unwrap();
}

#[test]
fn a_held_screen_neither_expires_nor_makes_way() {
    let (source, fake) = fake();
    source.poll().unwrap();
    let hold = source.hold(&id("t1"), 46, 30).unwrap();
    {
        let state = lock(&source.state);
        let mut screen = lock(&state.screens["t1"]);
        screen.age(Duration::from_secs(600));
        assert!(!screen.expired());
        assert_eq!(screen.idle(), Duration::ZERO);
    }
    source.poll().unwrap();
    assert!(lock(&source.state).screens.contains_key("t1"));
    drop(hold);
    drop(source);
    fake.server.join().unwrap();
}

#[test]
fn an_unknown_pane_or_size_holds_nothing() {
    let (source, fake) = fake();
    source.poll().unwrap();
    assert!(source.hold(&id("t404"), 46, 30).is_err());
    assert!(source.hold(&id("t1"), 0, 30).is_err());
    assert!(fake.attaches.try_recv().is_err());
    drop(source);
    fake.server.join().unwrap();
}
