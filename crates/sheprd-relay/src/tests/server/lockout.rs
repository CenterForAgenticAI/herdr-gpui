//! The dashboard lockout: three wrong tries block an address for a while,
//! a block runs out, and memory stays bounded.

use crate::dashboard::lockout::{BLOCK, Lockout, TRIES, Verdict};
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

fn address(last: u8) -> IpAddr {
    IpAddr::from([192, 0, 2, last])
}

#[test]
fn the_third_wrong_try_blocks_for_the_block_time() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 1..TRIES {
        assert_eq!(lockout.fail(address(1), now), Verdict::Allowed);
        assert_eq!(lockout.check(address(1), now), Verdict::Allowed);
    }
    assert_eq!(lockout.fail(address(1), now), Verdict::Blocked(BLOCK));
    assert_eq!(lockout.check(address(1), now), Verdict::Blocked(BLOCK));
    let later = now + Duration::from_secs(60);
    assert_eq!(
        lockout.check(address(1), later),
        Verdict::Blocked(BLOCK - Duration::from_secs(60))
    );
    // Others are untouched.
    assert_eq!(lockout.check(address(2), now), Verdict::Allowed);
}

#[test]
fn a_block_runs_out_and_starts_the_count_again() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 0..TRIES {
        lockout.fail(address(3), now);
    }
    let after = now + BLOCK;
    assert_eq!(lockout.check(address(3), after), Verdict::Allowed);
    assert_eq!(lockout.fail(address(3), after), Verdict::Allowed);
}

#[test]
fn old_wrong_tries_are_forgotten() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 1..TRIES {
        lockout.fail(address(4), now);
    }
    // Long after, one more wrong try starts a fresh count.
    assert_eq!(lockout.fail(address(4), now + BLOCK), Verdict::Allowed);
}

#[test]
fn signing_in_clears_the_count() {
    let lockout = Lockout::default();
    let now = Instant::now();
    for _ in 1..TRIES {
        lockout.fail(address(5), now);
    }
    lockout.succeed(address(5));
    for _ in 1..TRIES {
        assert_eq!(lockout.fail(address(5), now), Verdict::Allowed);
    }
}
