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
    metrics::{Gauge, Kind, Metrics, Refused},
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
    dials: Mutex<Limiter<IpAddr>>,
    pub limits: Limits,
    listens: Mutex<Limiter<IpAddr>>,
    pub metrics: Metrics,
    next: AtomicU64,
    state: Mutex<State>,
    total: Option<Throttle>,
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
    key: Key,
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
            dials: Mutex::new(Limiter::new(Rate::per_minute(limits.dials_per_minute))),
            limits,
            listens: Mutex::new(Limiter::new(Rate::per_minute(limits.listens_per_minute))),
            metrics: Metrics::default(),
            next: AtomicU64::new(1),
            state: Mutex::default(),
            total: Throttle::new(limits.total_bandwidth),
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
    pub fn total_throttle(&self) -> Option<&Throttle> {
        self.total.as_ref()
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
    pub fn register(self: &Arc<Self>, route: RouteId) -> Result<Registration, Full> {
        let (sender, incoming) = mpsc::channel(INCOMING_QUEUE);
        let generation = self.next.fetch_add(1, Relaxed);
        let mut state = self.state();
        let replacing = state.listeners.contains_key(&route);
        if !replacing && state.listeners.len() >= self.limits.max_listeners {
            return Err(Full);
        }
        state.listeners.insert(
            route,
            Listener {
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
        drop(state);
        self.metrics.raise(Gauge::Pending);
        Ok(Ok(Waiting {
            accepted: receiver,
            pending: Unclaimed::new(Arc::clone(self), session),
            slot: Slot::new(Arc::clone(self), route, key, pairing),
        }))
    }

    /// Claims the dial `session`, once: where to hand the accepting socket.
    pub fn accept(&self, session: SessionId) -> Option<oneshot::Sender<Accepted>> {
        let pending = self.state().pending.remove(&session)?;
        self.metrics.lower(Gauge::Pending);
        Some(pending.accepted)
    }

    /// Records a spliced session on `slot`'s route until the guard drops.
    #[must_use]
    pub fn open(self: &Arc<Self>, slot: Slot) -> Session {
        let id = self.next.fetch_add(1, Relaxed);
        let traffic = Arc::new(Traffic::default());
        let revoke = Arc::new(Notify::new());
        self.state().sessions.insert(
            id,
            Live {
                key: slot.key,
                pairing: slot.pairing,
                revoke: Arc::clone(&revoke),
                route: slot.route,
                started: Instant::now(),
                traffic: Arc::clone(&traffic),
            },
        );
        self.metrics.raise(Gauge::Sessions);
        Session::new(Arc::clone(self), id, slot, traffic, revoke)
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
