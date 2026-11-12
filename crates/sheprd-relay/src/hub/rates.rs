//! Rate limits per address, and per wide block. An attacker holding a
//! routed IPv6 /48 has 65,536 /64s, each a fresh address to the narrow
//! limit; the wide one, [`WIDE_SHARE`] times as loose, holds the whole
//! block back while leaving room for the many users a /48 or an IPv4 /24
//! can hold.

use crate::{
    bucket::{Limiter, Outcome, Rate, source_key},
    guard::wide_key,
    limits::WIDE_SHARE,
};
use std::{net::IpAddr, sync::Mutex, time::Instant};

/// One rate, applied to each address and to its wide block.
#[derive(Debug)]
pub struct PerAddress {
    narrow: Mutex<Limiter<IpAddr>>,
    /// Where the limiters' milliseconds count from: a monotonic clock, so a
    /// wall clock stepping back cannot stop refills.
    origin: Instant,
    wide: Mutex<Limiter<IpAddr>>,
}

impl PerAddress {
    #[must_use]
    pub fn new(per_minute: u32, origin: Instant) -> Self {
        let share = u32::try_from(WIDE_SHARE).unwrap_or(u32::MAX);
        Self {
            narrow: Mutex::new(Limiter::new(Rate::per_minute(per_minute))),
            origin,
            wide: Mutex::new(Limiter::new(Rate::per_minute(
                per_minute.saturating_mul(share),
            ))),
        }
    }

    /// Whether `address` may go ahead now, spending from both buckets.
    pub fn check(&self, address: IpAddr) -> Outcome {
        let now_ms = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        let narrow = check(&self.narrow, source_key(address), now_ms);
        if narrow != Outcome::Allowed {
            return narrow;
        }
        check(&self.wide, wide_key(address), now_ms)
    }
}

fn check(limiter: &Mutex<Limiter<IpAddr>>, key: IpAddr, now_ms: u64) -> Outcome {
    match limiter.lock() {
        Ok(mut limiter) => limiter.check(key, now_ms),
        Err(poisoned) => poisoned.into_inner().check(key, now_ms),
    }
}
