//! The steps both transports share, in one place so QUIC and the WebSocket
//! fallback cannot drift apart: admitting a listener, applying what it
//! says about who may dial, and taking a dial from proof to accepted peer.
//! Each step counts its outcome and answers with a wire [`Status`].

use crate::{
    RouteId,
    hub::{Accepted, DialRefusal, Hub, Registration, Session},
    metrics::{Dial, Listen},
    route::prove,
    wire::{Key, Message, Status},
};
use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio::time::timeout;

/// Checks a listen request: the address's rate, the token, and the proof
/// over `signed`; then registers the listener. `token` is empty when none
/// was given.
pub fn listen(
    hub: &Arc<Hub>,
    address: IpAddr,
    token: &[u8],
    key: &[u8],
    signature: &[u8],
    signed: &[u8],
) -> Result<Registration, Status> {
    let refuse = |status, outcome| {
        hub.metrics.listen(outcome);
        status
    };
    if !hub.allow_listen(address) {
        return Err(refuse(Status::Limited, Listen::Limited));
    }
    if !hub.access.admit((!token.is_empty()).then_some(token)) {
        return Err(refuse(Status::Forbidden, Listen::Forbidden));
    }
    let Ok(route) = prove(key, signature, signed) else {
        return Err(refuse(Status::Refused, Listen::BadProof));
    };
    let registration = hub
        .register(route)
        .map_err(|_| refuse(Status::Full, Listen::Full))?;
    hub.metrics.listen(Listen::Ready);
    Ok(registration)
}

/// Applies what a listener says about who may dial it.
pub fn apply(registration: &Registration, message: Message) -> Status {
    match message {
        Message::DialKeys(keys) => {
            registration.set_dial_keys(&keys);
            Status::Ok
        }
        Message::PairingKey { key, seconds } => {
            let lifetime = Duration::from_secs(u64::from(seconds));
            if registration.add_pairing_key(key, lifetime) {
                Status::Ok
            } else {
                Status::TooMany
            }
        }
    }
}

/// Takes a dial from its proof to an open session: checks the address's
/// rate and the proof over `signed`, waits for the listener, and opens the
/// session if the key is still allowed. A key the listener does not allow,
/// or revoked while the dial waited, answers `Offline`, exactly as no
/// listener would.
pub async fn dial(
    hub: &Arc<Hub>,
    address: IpAddr,
    route: RouteId,
    key: &Key,
    signature: &[u8],
    signed: &[u8],
) -> Result<(Accepted, Session), Status> {
    let refuse = |status, outcome| {
        hub.metrics.dial(outcome);
        status
    };
    if !hub.allow_dial(address) {
        return Err(refuse(Status::Limited, Dial::Limited));
    }
    if prove(key, signature, signed).is_err() {
        return Err(refuse(Status::Offline, Dial::Unauthorized));
    }
    let waiting = match hub.dial(route, *key) {
        Ok(Ok(waiting)) => waiting,
        Ok(Err(DialRefusal::Offline)) => return Err(refuse(Status::Offline, Dial::Offline)),
        Ok(Err(DialRefusal::Unauthorized)) => {
            return Err(refuse(Status::Offline, Dial::Unauthorized));
        }
        Ok(Err(DialRefusal::Busy)) | Err(_) => return Err(refuse(Status::Busy, Dial::Busy)),
    };
    let accepted = match timeout(hub.limits.accept_timeout, waiting.accepted).await {
        Ok(Ok(accepted)) => accepted,
        Ok(Err(_)) | Err(_) => return Err(refuse(Status::Timeout, Dial::Timeout)),
    };
    drop(waiting.pending);
    let Some(session) = hub.open(waiting.slot) else {
        return Err(refuse(Status::Offline, Dial::Unauthorized));
    };
    hub.metrics.dial(Dial::Spliced);
    Ok((accepted, session))
}
