//! Banning toxic clients, with nothing to set up.
//!
//! Each address (an IPv4 address, or an IPv6 /64, as the rate limits key
//! them) earns points for each [`Offense`]: hitting limits, dialing
//! without an allowed key, forged proofs, malformed messages, dashboard
//! lockouts. Points fade by one a minute; at [`BAN_POINTS`] the address
//! is banned, for 10 minutes, then an hour, then a day if it keeps at it.
//! An address moving more than the hourly transfer budget, counted on both
//! sides of every session, is banned at once.
//!
//! A banned address is refused before any work (before the QUIC handshake,
//! before any HTTP route), and the hub ends its sessions and listeners.
//! Loopback and private addresses are never banned automatically: behind a
//! reverse proxy that is misconfigured, every caller can look like the
//! proxy, and banning it would lock everyone out. Operators can still ban
//! any range by hand ([`Cidr`]).

mod cidr;

pub use cidr::{Cidr, parse_list};

use crate::{bucket::source_key, metrics::Offense};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, MutexGuard, RwLock},
    time::{Duration, Instant},
};

/// Points that ban an address.
pub const BAN_POINTS: u32 = 10;
/// How long a ban lasts, by how many the address had lately.
pub const BANS: [Duration; 3] = [
    Duration::from_secs(10 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(24 * 60 * 60),
];
/// A clean day after a ban ends forgets the bans before it.
const FORGIVE: Duration = Duration::from_secs(24 * 60 * 60);
/// The window the transfer budget counts over.
const HOUR: Duration = Duration::from_secs(60 * 60);
/// Points fade by one each of these.
const FADE: Duration = Duration::from_secs(60);
/// The most addresses tracked at once, which bounds memory.
pub const MAX_RECORDS: usize = 65_536;
/// The most wide blocks banned when the table is full of bans.
pub const MAX_WIDE: usize = 4_096;
/// The most bans the dashboard lists.
const MAX_ROWS: usize = 50;

/// What one offense costs.
#[must_use]
pub const fn points(offense: Offense) -> u32 {
    match offense {
        // One point, fading one a minute: a revoked device, or a phone
        // whose computer is off, retrying about once a minute never adds
        // up, so it cannot get its household's shared address banned; a
        // burst does.
        Offense::ConnectionCap | Offense::Limited | Offense::RefusedDial => 1,
        Offense::Probe => 2,
        Offense::BadProof | Offense::Malformed => 3,
        Offense::Bandwidth | Offense::Lockout => BAN_POINTS,
    }
}

#[derive(Clone, Copy, Debug)]
struct Record {
    banned_until: Option<Instant>,
    /// Bans lately, which sets the next one's length.
    bans: u32,
    last_ban_end: Option<Instant>,
    points: u32,
    reason: Offense,
    updated: Instant,
    window_bytes: u64,
    window_start: Instant,
}

impl Record {
    fn new(now: Instant) -> Self {
        Self {
            banned_until: None,
            bans: 0,
            last_ban_end: None,
            points: 0,
            reason: Offense::Limited,
            updated: now,
            window_bytes: 0,
            window_start: now,
        }
    }

    /// Fades points and the transfer window to `now`.
    fn fade(&mut self, now: Instant) {
        let minutes = now.saturating_duration_since(self.updated).as_secs() / FADE.as_secs();
        if minutes > 0 {
            self.points = self
                .points
                .saturating_sub(u32::try_from(minutes).unwrap_or(u32::MAX));
            self.updated += FADE * u32::try_from(minutes).unwrap_or(u32::MAX);
        }
        if now.saturating_duration_since(self.window_start) >= HOUR {
            self.window_start = now;
            self.window_bytes = 0;
        }
        if self
            .last_ban_end
            .is_some_and(|end| now.saturating_duration_since(end) >= FORGIVE)
        {
            self.bans = 0;
            self.last_ban_end = None;
        }
    }

    fn banned(&self, now: Instant) -> bool {
        self.banned_until.is_some_and(|until| until > now)
    }

    /// Bans for `reason`; how long.
    fn ban(&mut self, reason: Offense, now: Instant) -> Duration {
        let index = usize::try_from(self.bans)
            .unwrap_or(usize::MAX)
            .min(BANS.len() - 1);
        let length = BANS[index];
        self.banned_until = Some(now + length);
        self.last_ban_end = Some(now + length);
        self.bans = self.bans.saturating_add(1);
        self.points = 0;
        self.reason = reason;
        length
    }

    /// Nothing worth keeping: no ban, no points, no recent transfer.
    fn idle(&self, now: Instant) -> bool {
        !self.banned(now) && self.points == 0 && self.bans == 0 && self.window_bytes == 0
    }
}

/// A ban the dashboard lists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BanRow {
    /// The address, or the /64 for IPv6; a manual entry's range.
    pub address: String,
    /// Set by the operator rather than earned.
    pub manual: bool,
    pub reason: &'static str,
    /// Seconds until it lifts; 0 for a manual ban, which lasts.
    pub seconds_left: u64,
}

/// Bans, earned and manual.
#[derive(Debug)]
pub struct Guard {
    auto: bool,
    manual: RwLock<Vec<Cidr>>,
    max_transfer_per_hour: u64,
    records: Mutex<HashMap<IpAddr, Record>>,
    /// Wide blocks (an IPv6 /48, an IPv4 /24) banned when every record is
    /// an active ban, and when each lifts.
    wide: Mutex<HashMap<IpAddr, (Instant, Offense)>>,
}

/// The wide block `address` falls in: its IPv6 /48 or IPv4 /24, where an
/// attacker's many addresses usually sit together.
#[must_use]
pub fn wide_key(address: IpAddr) -> IpAddr {
    match address.to_canonical() {
        IpAddr::V4(address) => {
            IpAddr::V4(std::net::Ipv4Addr::from(u32::from(address) & 0xffff_ff00))
        }
        IpAddr::V6(address) => IpAddr::V6(std::net::Ipv6Addr::from(
            u128::from(address) & !((1u128 << 80) - 1),
        )),
    }
}

/// Whether an address is one automatic bans leave alone.
#[must_use]
pub fn exempt(address: IpAddr) -> bool {
    match address.to_canonical() {
        IpAddr::V4(address) => {
            address.is_loopback()
                || address.is_private()
                || address.is_link_local()
                || address.is_unspecified()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || address.is_unique_local()
                || address.is_unicast_link_local()
                || address.is_unspecified()
        }
    }
}

impl Guard {
    /// A guard that bans automatically when `auto`, and bans an address
    /// moving more than `max_transfer_per_hour` bytes (0 for no budget).
    #[must_use]
    pub fn new(auto: bool, max_transfer_per_hour: u64) -> Self {
        Self {
            auto,
            manual: RwLock::default(),
            max_transfer_per_hour,
            records: Mutex::default(),
            wide: Mutex::default(),
        }
    }

    fn records(&self) -> MutexGuard<'_, HashMap<IpAddr, Record>> {
        match self.records.lock() {
            Ok(records) => records,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Replaces the operator's ranges.
    pub fn set_manual(&self, ranges: Vec<Cidr>) {
        match self.manual.write() {
            Ok(mut manual) => *manual = ranges,
            Err(poisoned) => *poisoned.into_inner() = ranges,
        }
    }

    fn manually_banned(&self, address: IpAddr) -> bool {
        let contains = |ranges: &Vec<Cidr>| ranges.iter().any(|range| range.contains(address));
        match self.manual.read() {
            Ok(manual) => contains(&manual),
            Err(poisoned) => contains(&poisoned.into_inner()),
        }
    }

    /// Whether `address` is banned at `now`, by hand or earned.
    pub fn banned(&self, address: IpAddr, now: Instant) -> bool {
        if self.manually_banned(address) {
            return true;
        }
        if self
            .records()
            .get(&source_key(address))
            .is_some_and(|record| record.banned(now))
        {
            return true;
        }
        self.wide()
            .get(&wide_key(address))
            .is_some_and(|(until, _)| *until > now)
    }

    fn wide(&self) -> MutexGuard<'_, HashMap<IpAddr, (Instant, Offense)>> {
        match self.wide.lock() {
            Ok(wide) => wide,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Runs `change` on `address`'s record; a ban it caused, newly.
    fn record(
        &self,
        address: IpAddr,
        now: Instant,
        change: impl FnOnce(&mut Record) -> Option<Duration>,
    ) -> Option<Duration> {
        if !self.auto || exempt(address) {
            return None;
        }
        let key = source_key(address);
        let mut records = self.records();
        if records.len() >= MAX_RECORDS && !records.contains_key(&key) {
            records.retain(|_, record| {
                record.fade(now);
                !record.idle(now)
            });
            if records.len() >= MAX_RECORDS {
                // Still full, as when one attacker holds many /64s. Never
                // lift a ban to make room: drop the unbanned record with
                // the fewest points. If every slot is an active ban, an
                // offense that would ban this address bans its wide block
                // instead.
                let least = records
                    .iter()
                    .filter(|(_, record)| !record.banned(now))
                    .min_by_key(|(_, record)| (record.points, record.updated))
                    .map(|(key, _)| *key);
                match least {
                    Some(least) => {
                        records.remove(&least);
                    }
                    None => {
                        // Only what would ban this address on its own
                        // bans its block: never a lone point, and never a
                        // transfer still under budget, which every session
                        // charges each second. Smaller offenses go
                        // uncounted until a slot frees.
                        drop(records);
                        let mut scratch = Record::new(now);
                        return change(&mut scratch)
                            .and_then(|_| self.ban_wide(address, scratch.reason, now));
                    }
                }
            }
        }
        let record = records.entry(key).or_insert_with(|| Record::new(now));
        record.fade(now);
        if record.banned(now) {
            return None;
        }
        change(record)
    }

    /// Bans `address`'s wide block for the longest ban, when the table
    /// has no room left to judge it alone.
    fn ban_wide(&self, address: IpAddr, reason: Offense, now: Instant) -> Option<Duration> {
        let mut wide = self.wide();
        wide.retain(|_, (until, _)| *until > now);
        let key = wide_key(address);
        if wide.contains_key(&key) {
            return None;
        }
        if wide.len() >= MAX_WIDE {
            // Thousands of wide blocks of bans at once: the rate limits
            // still hold every caller back.
            return None;
        }
        let length = BANS[BANS.len() - 1];
        wide.insert(key, (now + length, reason));
        Some(length)
    }

    /// Counts `offense` against `address`; a ban it caused, newly.
    pub fn offend(&self, address: IpAddr, offense: Offense, now: Instant) -> Option<Duration> {
        self.record(address, now, |record| {
            record.points = record.points.saturating_add(points(offense));
            (record.points >= BAN_POINTS).then(|| record.ban(offense, now))
        })
    }

    /// Counts `bytes` carried for `address`; a ban for going over the
    /// hourly budget, newly.
    pub fn charge(&self, address: IpAddr, bytes: u64, now: Instant) -> Option<Duration> {
        if self.max_transfer_per_hour == 0 || bytes == 0 {
            return None;
        }
        let budget = self.max_transfer_per_hour;
        self.record(address, now, |record| {
            record.window_bytes = record.window_bytes.saturating_add(bytes);
            (record.window_bytes > budget).then(|| record.ban(Offense::Bandwidth, now))
        })
    }

    /// Active bans, earned ones first by time left, then manual ranges.
    #[must_use]
    pub fn bans(&self, now: Instant) -> Vec<BanRow> {
        let mut rows: Vec<BanRow> = self
            .records()
            .iter()
            .filter_map(|(address, record)| {
                let until = record.banned_until.filter(|until| *until > now)?;
                let address = match address {
                    IpAddr::V6(_) => format!("{address}/64"),
                    IpAddr::V4(_) => address.to_string(),
                };
                Some(BanRow {
                    address,
                    manual: false,
                    reason: record.reason.label(),
                    seconds_left: (until - now).as_secs(),
                })
            })
            .collect();
        rows.extend(
            self.wide()
                .iter()
                .filter(|(_, (until, _))| *until > now)
                .map(|(block, (until, reason))| BanRow {
                    address: match block {
                        IpAddr::V6(_) => format!("{block}/48"),
                        IpAddr::V4(_) => format!("{block}/24"),
                    },
                    manual: false,
                    reason: reason.label(),
                    seconds_left: (*until - now).as_secs(),
                }),
        );
        rows.sort_by_key(|row| std::cmp::Reverse(row.seconds_left));
        let manual = match self.manual.read() {
            Ok(manual) => manual.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        rows.extend(manual.iter().map(|range| BanRow {
            address: range.to_string(),
            manual: true,
            reason: "manual",
            seconds_left: 0,
        }));
        rows.truncate(MAX_ROWS);
        rows
    }

    /// How many addresses are banned now, earned bans only.
    #[must_use]
    pub fn active(&self, now: Instant) -> usize {
        let wide = self
            .wide()
            .values()
            .filter(|(until, _)| *until > now)
            .count();
        self.records()
            .values()
            .filter(|record| record.banned(now))
            .count()
            + wide
    }
}
