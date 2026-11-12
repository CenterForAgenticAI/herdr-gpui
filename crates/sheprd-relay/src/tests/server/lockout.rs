//! The dashboard lockout: each try is reserved before its token is
//! checked, three block an address for a while, a block runs out, and
//! signing in clears the count.

use crate::dashboard::lockout::{BLOCK, Lockout, TRIES, Verdict};
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

fn address(last: u8) -> IpAddr {
    IpAddr::from([192, 0, 2, last])
}

#[test]
fn three_tries_are_checked_then_the_address_is_blocked() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 0..TRIES {
        assert_eq!(lockout.attempt(address(1), now), Verdict::Allowed);
    }
    // The third try was checked; it left the address blocked.
    assert!(lockout.exhausted(address(1), now));
    assert_eq!(lockout.attempt(address(1), now), Verdict::Blocked(BLOCK));
    let later = now + Duration::from_secs(60);
    assert_eq!(
        lockout.check(address(1), later),
        Verdict::Blocked(BLOCK - Duration::from_secs(60))
    );
    // Others are untouched.
    assert_eq!(lockout.attempt(address(2), now), Verdict::Allowed);
}

#[test]
fn tries_sent_at_once_get_three_guesses_between_them() {
    let lockout = std::sync::Arc::new(Lockout::default());
    let now = Instant::now();
    let checked: usize = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..64)
            .map(|_| {
                let lockout = std::sync::Arc::clone(&lockout);
                scope.spawn(move || lockout.attempt(address(3), now) == Verdict::Allowed)
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum()
    });
    assert_eq!(checked, TRIES as usize);
}

#[test]
fn a_block_runs_out_and_starts_the_count_again() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 0..TRIES {
        lockout.attempt(address(4), now);
    }
    let after = now + BLOCK;
    assert_eq!(lockout.check(address(4), after), Verdict::Allowed);
    assert_eq!(lockout.attempt(address(4), after), Verdict::Allowed);
    assert!(!lockout.exhausted(address(4), after));
}

#[test]
fn old_wrong_tries_are_forgotten() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 1..TRIES {
        lockout.attempt(address(5), now);
    }
    // Long after, one more try starts a fresh count.
    lockout.attempt(address(5), now + BLOCK);
    assert!(!lockout.exhausted(address(5), now + BLOCK));
}

#[test]
fn signing_in_clears_the_count() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 1..TRIES {
        lockout.attempt(address(6), now);
    }
    // The last allowed try is the right token.
    assert_eq!(lockout.attempt(address(6), now), Verdict::Allowed);
    lockout.succeed(address(6));
    for _ in 0..TRIES {
        assert_eq!(lockout.attempt(address(6), now), Verdict::Allowed);
    }
}

#[test]
fn a_table_full_of_guessers_still_lets_the_right_token_be_checked() {
    use crate::dashboard::lockout::MAX_ADDRESSES;
    let lockout = Lockout::default();
    let now = Instant::now();
    for index in 0..u32::try_from(MAX_ADDRESSES).unwrap() {
        let guesser = IpAddr::from(std::net::Ipv6Addr::from(
            (0x2001_0db8_u128 << 96) | (u128::from(index) << 64),
        ));
        lockout.attempt(guesser, now);
    }
    // A newcomer is not refused unchecked: its token decides.
    assert_eq!(
        lockout.attempt("198.51.100.94".parse().unwrap(), now),
        Verdict::Untracked
    );
}
