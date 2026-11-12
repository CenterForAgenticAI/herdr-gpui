//! Blocking addresses that guess the dashboard token: three wrong tries
//! within the window block that address for [`BLOCK`]. While blocked, its
//! tries are refused without being checked, so a blocked guesser learns
//! nothing. Addresses are keyed as the rate limits key them (an IPv4
//! address, or an IPv6 /64) and live only in memory.

use crate::bucket::source_key;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

/// Wrong tries an address may make before it is blocked.
pub const TRIES: u32 = 3;
/// How long a block lasts, and how long a wrong try is remembered.
pub const BLOCK: Duration = Duration::from_secs(15 * 60);
/// The most addresses tracked at once, which bounds memory.
pub const MAX_ADDRESSES: usize = 16_384;

#[derive(Clone, Copy, Debug)]
struct Record {
    blocked_until: Option<Instant>,
    failures: u32,
    last: Instant,
}

/// The lockout's verdict on an address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    Allowed,
    /// Blocked for about this long still.
    Blocked(Duration),
    /// The table is full of recent guessers and this address is not among
    /// them: check the token, but there is no try to count or clear. The
    /// token's length, not the lockout, keeps guessing hopeless then.
    Untracked,
}

/// Wrong tries per address, and the blocks they caused.
#[derive(Debug, Default)]
pub struct Lockout {
    records: Mutex<HashMap<IpAddr, Record>>,
}

impl Lockout {
    fn records(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, Record>> {
        match self.records.lock() {
            Ok(records) => records,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Whether `address` is blocked at `now`, and for how long still.
    pub fn check(&self, address: IpAddr, now: Instant) -> Verdict {
        let records = self.records();
        match records.get(&source_key(address)) {
            Some(Record {
                blocked_until: Some(until),
                ..
            }) if *until > now => Verdict::Blocked(*until - now),
            Some(_) | None => Verdict::Allowed,
        }
    }

    /// Reserves one try for `address` before its token is checked, under
    /// the same lock that refuses it: each try counts as wrong until
    /// [`Lockout::succeed`] clears it, so requests sent at once cannot
    /// slip more than [`TRIES`] guesses between a check and a failure.
    /// `Allowed` or `Untracked` means check the token; `Blocked`, refuse it
    /// unchecked.
    pub fn attempt(&self, address: IpAddr, now: Instant) -> Verdict {
        let key = source_key(address);
        let mut records = self.records();
        if records.len() >= MAX_ADDRESSES && !records.contains_key(&key) {
            records.retain(|_, record| now.saturating_duration_since(record.last) < BLOCK);
            if records.len() >= MAX_ADDRESSES {
                // Full of recent guessers: track no one new, rather than
                // forget one, or refuse everyone, the holder of the right
                // token included.
                return Verdict::Untracked;
            }
        }
        let record = records.entry(key).or_insert(Record {
            blocked_until: None,
            failures: 0,
            last: now,
        });
        if let Some(until) = record.blocked_until.filter(|until| *until > now) {
            return Verdict::Blocked(until - now);
        }
        // Wrong tries older than the window, or before a block that has
        // run out, are forgotten.
        let expired = record.blocked_until.is_some_and(|until| until <= now)
            || now.saturating_duration_since(record.last) >= BLOCK;
        if expired {
            record.failures = 0;
            record.blocked_until = None;
        }
        record.failures += 1;
        record.last = now;
        if record.failures >= TRIES {
            // This try is still checked; the block covers every one after.
            record.blocked_until = Some(now + BLOCK);
        }
        Verdict::Allowed
    }

    /// Whether `address`'s tries have now used up its allowance: true
    /// after the try that set the block, which the caller reports.
    pub fn exhausted(&self, address: IpAddr, now: Instant) -> bool {
        matches!(self.check(address, now), Verdict::Blocked(_))
    }

    /// Forgets `address`'s wrong tries after it signs in.
    pub fn succeed(&self, address: IpAddr) {
        self.records().remove(&source_key(address));
    }
}
