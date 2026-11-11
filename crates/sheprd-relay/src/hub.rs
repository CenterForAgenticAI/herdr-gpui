//! The relay's state: who listens and who may dial them, which dials wait,
//! which sessions run, and how many sockets each address holds. Everything
//! is in memory and bounded by [`Limits`]. Every slot is held by a guard
//! (`guards`) whose `Drop` gives it back, so an early return or a dropped
//! task can never leak one.
//!
//! The relay keeps users apart without accounts: a route belongs to whoever
//! proves its key, only the dial keys its listener names may reach it, and
//! what crosses is end-to-end encrypted.

pub mod access;
mod guards;

pub use access::Access;
pub use guards::{Permit, Registration, Session, Slot, Unclaimed, Waiting};

use crate::{
    Limits, RouteId, SessionId,
    bucket::{Limiter, Rate, Throttle, source_key},
    guard::Guard,
    metrics::{Direction, Gauge, Kind, Metrics, Offense, Refused},
    splice::Peer,
    wire::{Key, MAX_DIAL_KEYS},
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering::Relaxed},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Notify, mpsc, oneshot};

/// Incoming dials a listener may have unread before more are refused.
const INCOMING_QUEUE: usize = 16;
/// The most sessions the dashboard lists.
const MAX_ROWS: usize = 50;
/// Pairing keys one listener may hold at once: a pairing dialog or two.
pub const MAX_PAIRING_KEYS: usize = 4;
/// The longest a session opened with a pairing key may last.
pub const PAIRING_SESSION: Duration = Duration::from_secs(10 * 60);

/// Everything the relay's handlers share.
#[derive(Debug)]
pub struct Hub {
    pub access: Access,
    /// Wakes every QUIC connection when an address is banned, so the
    /// banned one closes itself; each checks its own address.
    bans: tokio::sync::broadcast::Sender<()>,
    dials: Mutex<Limiter<IpAddr>>,
    /// Bans, earned and manual.
    pub guard: Guard,
    pub limits: Limits,
    listens: Mutex<Limiter<IpAddr>>,
    pub metrics: Metrics,
    next: AtomicU64,
    state: Mutex<State>,
    /// One throttle each way, as `total_bandwidth` promises.
    total: [Option<Throttle>; 2],
}

#[derive(Debug, Default)]
struct State {
    addresses: HashMap<IpAddr, usize>,
    connections: usize,
    listeners: HashMap<RouteId, Listener>,
    pending: HashMap<SessionId, Pending>,
    /// Dials and sessions held against each route, for its limit.
    routes: HashMap<RouteId, usize>,
    sessions: HashMap<u64, Live>,
    /// Dials and sessions in total.
    slots: usize,
}

#[derive(Debug)]
struct Listener {
    /// Where it connects from, in full: bans match it by range, and by
    /// the address limits key it under.
    address: IpAddr,
    dial_keys: HashSet<Key>,
    generation: u64,
    incoming: mpsc::Sender<Incoming>,
    pairing: Vec<(Key, Instant)>,
}

/// A dial the listener is told about: its id, to accept it over a
/// WebSocket, and the key it was signed with, so the computer knows which
/// device calls before the end-to-end handshake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Incoming {
    pub key: Key,
    pub session: SessionId,
}

#[derive(Debug)]
struct Pending {
    accepted: oneshot::Sender<Accepted>,
}

/// The listener's side of one dial. A WebSocket accept holds its own
/// connection slot; a QUIC stream shares its connection's.
#[derive(Debug)]
pub struct Accepted {
    pub peer: Peer,
    pub permit: Option<Permit>,
}

#[derive(Debug)]
struct Live {
    /// Ends the session when either side is banned.
    ban: Arc<Notify>,
    /// Both sides' addresses, in full, so a manual ban of one IPv6 host
    /// matches; automatic bans compare the keys limits use.
    dialer: IpAddr,
    key: Key,
    listener: IpAddr,
    pairing: bool,
    revoke: Arc<Notify>,
    route: RouteId,
    started: Instant,
    traffic: Arc<Traffic>,
}

/// Bytes one session has carried each way.
#[derive(Debug, Default)]
pub struct Traffic {
    pub from_dialer: AtomicU64,
    pub from_listener: AtomicU64,
}

/// Why a dial could not wait for its listener.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialRefusal {
    /// The route or the relay holds as many sessions as it may.
    Busy,
    /// No one listens on the route.
    Offline,
    /// Someone listens, but not for this key. Peers are told `Offline`.
    Unauthorized,
}

/// The relay holds as many listeners as it may.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Full;

/// One row of the dashboard's session table.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionRow {
    pub bytes_from_dialer: u64,
    pub bytes_from_listener: u64,
    pub pairing: bool,
    pub route: String,
    pub seconds: u64,
}

impl Hub {
    #[must_use]
    pub fn new(limits: Limits, access: Access) -> Self {
        Self {
            access,
            bans: tokio::sync::broadcast::channel(16).0,
            dials: Mutex::new(Limiter::new(Rate::per_minute(limits.dials_per_minute))),
            guard: Guard::new(limits.auto_ban, limits.max_transfer_per_hour),
            limits,
            listens: Mutex::new(Limiter::new(Rate::per_minute(limits.listens_per_minute))),
            metrics: Metrics::default(),
            next: AtomicU64::new(1),
            state: Mutex::default(),
            total: [
                Throttle::new(limits.total_bandwidth),
                Throttle::new(limits.total_bandwidth),
            ],
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// The throttle every session shares, unless bandwidth is unlimited.
    #[must_use]
    pub fn total_throttle(&self, direction: Direction) -> Option<&Throttle> {
        let index = match direction {
            Direction::FromDialer => 0,
            Direction::FromListener => 1,
        };
        self.total[index].as_ref()
    }

    /// Whether `address` may start another dial now.
    pub fn allow_dial(&self, address: IpAddr) -> bool {
        allow(&self.dials, address)
    }

    /// Whether `address` may start another listen now.
    pub fn allow_listen(&self, address: IpAddr) -> bool {
        allow(&self.listens, address)
    }

    /// Takes a connection slot for `address`, or says which limit is full.
    pub fn admit(self: &Arc<Self>, address: IpAddr, kind: Kind) -> Result<Permit, Refused> {
        let key = source_key(address);
        let mut state = self.state();
        if state.connections >= self.limits.max_connections {
            drop(state);
            self.metrics.refuse(Refused::Total);
            return Err(Refused::Total);
        }
        let held = state.addresses.get(&key).copied().unwrap_or(0);
        if held >= self.limits.max_connections_per_ip {
            drop(state);
            self.metrics.refuse(Refused::PerAddress);
            self.offend(address, Offense::ConnectionCap);
            return Err(Refused::PerAddress);
        }
        state.connections += 1;
        state.addresses.insert(key, held + 1);
        drop(state);
        self.metrics.raise(Gauge::Connections(kind));
        Ok(Permit::new(Arc::clone(self), key, kind))
    }

    /// Makes this socket the listener for `route`, replacing any other,
    /// unless the relay is full. No one may dial it until it names its dial
    /// keys, and its keys go with it when it leaves.
    pub fn register(
        self: &Arc<Self>,
        route: RouteId,
        address: IpAddr,
    ) -> Result<Registration, Full> {
        let (sender, incoming) = mpsc::channel(INCOMING_QUEUE);
        let generation = self.next.fetch_add(1, Relaxed);
        let mut state = self.state();
        // Checked under the lock `kick` sweeps with: a ban either comes
        // first and refuses this listener, or comes after and removes it.
        if self.guard.banned(address, Instant::now()) {
            return Err(Full);
        }
        let replacing = state.listeners.contains_key(&route);
        if !replacing && state.listeners.len() >= self.limits.max_listeners {
            return Err(Full);
        }
        state.listeners.insert(
            route,
            Listener {
                address,
                dial_keys: HashSet::new(),
                generation,
                incoming: sender,
                pairing: Vec::new(),
            },
        );
        drop(state);
        if !replacing {
            self.metrics.raise(Gauge::Listeners);
        }
        Ok(Registration::new(
            Arc::clone(self),
            generation,
            incoming,
            route,
        ))
    }

    /// Replaces the dial keys of `route`'s listener `generation`, ending
    /// sessions of keys it no longer names. Pairing sessions are left to
    /// end on their own: the computer replaces a pairing key with the new
    /// device's own key while that session is still open.
    fn set_dial_keys(&self, route: RouteId, generation: u64, keys: &[Key]) {
        let mut state = self.state();
        let Some(listener) = state.listeners.get_mut(&route) else {
            return;
        };
        if listener.generation != generation {
            return;
        }
        listener.dial_keys = keys.iter().copied().take(MAX_DIAL_KEYS).collect();
        let allowed = listener.dial_keys.clone();
        for live in state.sessions.values() {
            if live.route == route && !live.pairing && !allowed.contains(&live.key) {
                live.revoke.notify_one();
            }
        }
    }

    /// Allows `key` one dial to `route` within `lifetime`, for pairing.
    fn add_pairing_key(
        &self,
        route: RouteId,
        generation: u64,
        key: Key,
        lifetime: Duration,
    ) -> bool {
        let mut state = self.state();
        let Some(listener) = state.listeners.get_mut(&route) else {
            return false;
        };
        if listener.generation != generation {
            return false;
        }
        let now = Instant::now();
        listener.pairing.retain(|(_, expires)| *expires > now);
        if listener.pairing.len() >= MAX_PAIRING_KEYS {
            return false;
        }
        listener.pairing.push((key, now + lifetime));
        true
    }

    /// Tells `route`'s listener that `key` dials, holding a slot for it, if
    /// the listener allows `key`. A pairing key is used up here.
    pub fn dial(
        self: &Arc<Self>,
        route: RouteId,
        key: Key,
        dialer: IpAddr,
    ) -> crate::Result<Result<Waiting, DialRefusal>> {
        let session = SessionId::random()?;
        let (accepted, receiver) = oneshot::channel();
        let mut state = self.state();
        let Some(listener) = state.listeners.get_mut(&route) else {
            return Ok(Err(DialRefusal::Offline));
        };
        let now = Instant::now();
        listener.pairing.retain(|(_, expires)| *expires > now);
        let pairing = !listener.dial_keys.contains(&key);
        let pairing_index = listener
            .pairing
            .iter()
            .position(|(allowed, _)| *allowed == key);
        if pairing && pairing_index.is_none() {
            return Ok(Err(DialRefusal::Unauthorized));
        }
        let incoming = listener.incoming.clone();
        let listener_address = listener.address;
        let on_route = state.routes.get(&route).copied().unwrap_or(0);
        if on_route >= self.limits.max_sessions_per_route || state.slots >= self.limits.max_sessions
        {
            return Ok(Err(DialRefusal::Busy));
        }
        if incoming.try_send(Incoming { key, session }).is_err() {
            return Ok(Err(DialRefusal::Busy));
        }
        if let (true, Some(index), Some(listener)) =
            (pairing, pairing_index, state.listeners.get_mut(&route))
        {
            listener.pairing.swap_remove(index);
        }
        state.pending.insert(session, Pending { accepted });
        state.routes.insert(route, on_route + 1);
        state.slots += 1;
        // Raised before the lock goes, so an `accept` racing in cannot
        // lower it first and leave it a count too high.
        self.metrics.raise(Gauge::Pending);
        drop(state);
        Ok(Ok(Waiting {
            accepted: receiver,
            pending: Unclaimed::new(Arc::clone(self), session),
            slot: Slot::new(
                Arc::clone(self),
                route,
                key,
                pairing,
                (dialer, listener_address),
            ),
        }))
    }

    /// Claims the dial `session`, once: where to hand the accepting socket.
    pub fn accept(&self, session: SessionId) -> Option<oneshot::Sender<Accepted>> {
        let pending = self.state().pending.remove(&session)?;
        self.metrics.lower(Gauge::Pending);
        Some(pending.accepted)
    }

    /// Records a spliced session on `slot`'s route until the guard drops,
    /// unless its key was revoked since the dial was checked: the check and
    /// the record happen under one lock, so a revocation either sees the
    /// session and ends it, or the session never starts. A pairing slot
    /// already spent its one-time key and goes ahead.
    #[must_use]
    pub fn open(self: &Arc<Self>, slot: Slot) -> Option<Session> {
        let id = self.next.fetch_add(1, Relaxed);
        let traffic = Arc::new(Traffic::default());
        let revoke = Arc::new(Notify::new());
        let ban = Arc::new(Notify::new());
        let mut state = self.state();
        // A ban on either side, checked under the lock `kick` sweeps with,
        // so a session either is refused here or is there to be ended.
        let now = Instant::now();
        if self.guard.banned(slot.dialer, now) || self.guard.banned(slot.listener, now) {
            return None;
        }
        let allowed = slot.pairing
            || state
                .listeners
                .get(&slot.route)
                .is_some_and(|listener| listener.dial_keys.contains(&slot.key));
        if !allowed {
            return None;
        }
        state.sessions.insert(
            id,
            Live {
                ban: Arc::clone(&ban),
                dialer: slot.dialer,
                key: slot.key,
                listener: slot.listener,
                pairing: slot.pairing,
                revoke: Arc::clone(&revoke),
                route: slot.route,
                started: Instant::now(),
                traffic: Arc::clone(&traffic),
            },
        );
        drop(state);
        self.metrics.raise(Gauge::Sessions);
        Some(Session::new(
            Arc::clone(self),
            id,
            slot,
            traffic,
            [revoke, ban],
        ))
    }

    /// Counts `offense` against `address`, banning it, and ending its
    /// sessions and listeners, if that was one too many.
    pub fn offend(self: &Arc<Self>, address: IpAddr, offense: Offense) {
        self.metrics.strike(offense);
        if let Some(length) = self.guard.offend(address, offense, Instant::now()) {
            self.banned(address, offense, length);
        }
    }

    /// Counts `bytes` a session carried for `address` against its hourly
    /// budget, banning it if that goes over.
    pub fn charge(self: &Arc<Self>, address: IpAddr, bytes: u64) {
        if let Some(length) = self.guard.charge(address, bytes, Instant::now()) {
            self.metrics.strike(Offense::Bandwidth);
            self.banned(address, Offense::Bandwidth, length);
        }
    }

    /// Told after each ban, to close connections a ban now covers.
    #[must_use]
    pub fn ban_notices(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.bans.subscribe()
    }

    /// Whether `address` is banned now.
    #[must_use]
    pub fn is_banned(&self, address: IpAddr) -> bool {
        self.guard.banned(address, Instant::now())
    }

    /// `address` was just banned: count it and end what it holds. The log
    /// line, like every other, names no address.
    fn banned(&self, address: IpAddr, offense: Offense, length: Duration) {
        self.metrics.ban(offense);
        tracing::warn!(
            reason = offense.label(),
            minutes = length.as_secs() / 60,
            "an address was banned"
        );
        self.kick(address);
    }

    /// Ends every session and listener whose address falls in `ranges`:
    /// a manual ban takes effect at once. Addresses are kept as limits key
    /// them, so IPv6 ranges match from /64 up.
    pub fn kick_ranges(&self, ranges: &[crate::guard::Cidr]) {
        let banned = |address: IpAddr| ranges.iter().any(|range| range.contains(address));
        let mut state = self.state();
        for live in state.sessions.values() {
            if banned(live.dialer) || banned(live.listener) {
                live.ban.notify_one();
            }
        }
        let before = state.listeners.len();
        state
            .listeners
            .retain(|_, listener| !banned(listener.address));
        let removed = before - state.listeners.len();
        drop(state);
        for _ in 0..removed {
            self.metrics.lower(Gauge::Listeners);
        }
        let _ = self.bans.send(());
    }

    /// Ends every session and listener of `address`.
    pub fn kick(&self, address: IpAddr) {
        let key = source_key(address);
        let mut state = self.state();
        for live in state.sessions.values() {
            if source_key(live.dialer) == key || source_key(live.listener) == key {
                live.ban.notify_one();
            }
        }
        let before = state.listeners.len();
        // A removed listener's channel closes, which ends its socket.
        state
            .listeners
            .retain(|_, listener| source_key(listener.address) != key);
        let removed = before - state.listeners.len();
        drop(state);
        for _ in 0..removed {
            self.metrics.lower(Gauge::Listeners);
        }
        let _ = self.bans.send(());
    }

    /// The busiest sessions, for the dashboard.
    #[must_use]
    pub fn sessions(&self) -> Vec<SessionRow> {
        let state = self.state();
        let mut rows: Vec<SessionRow> = state
            .sessions
            .values()
            .map(|live| SessionRow {
                bytes_from_dialer: live.traffic.from_dialer.load(Relaxed),
                bytes_from_listener: live.traffic.from_listener.load(Relaxed),
                pairing: live.pairing,
                route: live.route.short(),
                seconds: live.started.elapsed().as_secs(),
            })
            .collect();
        drop(state);
        rows.sort_by_key(|row| std::cmp::Reverse(row.bytes_from_dialer + row.bytes_from_listener));
        rows.truncate(MAX_ROWS);
        rows
    }
}

fn allow(limiter: &Mutex<Limiter<IpAddr>>, address: IpAddr) -> bool {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
    match limiter.lock() {
        Ok(mut limiter) => limiter.allow(source_key(address), now_ms),
        Err(poisoned) => poisoned.into_inner().allow(source_key(address), now_ms),
    }
}
