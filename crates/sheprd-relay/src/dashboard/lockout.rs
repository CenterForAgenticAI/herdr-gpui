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
const MAX_ADDRESSES: usize = 16_384;

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

    /// Whether `address` may try the token at `now`.
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

    /// Records a wrong try from `address`; whether it is now blocked.
    pub fn fail(&self, address: IpAddr, now: Instant) -> Verdict {
        let key = source_key(address);
        let mut records = self.records();
        if records.len() >= MAX_ADDRESSES && !records.contains_key(&key) {
            records.retain(|_, record| now.saturating_duration_since(record.last) < BLOCK);
            if records.len() >= MAX_ADDRESSES {
                // Full of recent guessers: refuse rather than forget one.
                return Verdict::Blocked(BLOCK);
            }
        }
        let record = records.entry(key).or_insert(Record {
            blocked_until: None,
            failures: 0,
            last: now,
        });
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
            record.blocked_until = Some(now + BLOCK);
            return Verdict::Blocked(BLOCK);
        }
        Verdict::Allowed
    }

    /// Forgets `address`'s wrong tries after it signs in.
    pub fn succeed(&self, address: IpAddr) {
        self.records().remove(&source_key(address));
    }
}
