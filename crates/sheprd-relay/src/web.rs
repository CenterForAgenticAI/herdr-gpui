//! The relay's HTTP routes: the WebSocket fallback for networks that block
//! UDP (`socket` has its protocol), and what clients read before choosing
//! a transport.
//!
//! - `GET /v1/listen`: a computer proves its key, names who may dial it,
//!   and waits for dials.
//! - `GET /v1/dial/{route}`: a device proves its dial key and reaches that
//!   computer, or hears `offline`.
//! - `GET /v1/accept/{session}`: the listener's socket for one dial.
//! - `GET /v1/info`: the protocol version, whether listening needs a token,
//!   and how to reach QUIC.
//! - `GET /healthz`: for uptime checks.
//!
//! The dashboard is merged in from [`crate::dashboard`].

use crate::{
    RouteId, SessionId, broker,
    endpoint::{Info, PROTOCOL},
    hub::{Accepted, Hub},
    metrics::{Dial, Kind, Listen, Refused},
    socket::{self, FromListener, Hello, NONCE_LEN, ToPeer},
    splice::{Peer, code, splice},
    wire::Status,
};
use axum::{
    Json, Router,
    extract::{
        ConnectInfo, Path, State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::time::timeout;

/// Control messages are small: a full set of dial keys is under 4 KiB.
const MAX_CONTROL: usize = 8 * 1024;
/// How long a peer has to answer the challenge.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// How often an idle listener is pinged, which also keeps NAT entries open;
/// more often when the silence limit is shorter.
const PING_INTERVAL: Duration = Duration::from_secs(25);

/// How to tell who a request came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// The connection's peer.
    Peer,
    /// The last `X-Forwarded-For` entry, which the proxy in front appended.
    /// Only for a relay no one can reach except through that proxy.
    Proxy,
}

/// What the handlers share.
#[derive(Clone, Debug)]
pub struct Shared {
    pub hub: Arc<Hub>,
    /// The relay's own name, which WebSocket proofs must cover. Never the
    /// request's `Host`: a client chooses that, so a relay passing another
    /// relay's challenge through could claim its own name in it. `None`
    /// turns the WebSocket fallback off.
    pub public_host: Option<Arc<str>>,
    /// What `/v1/info` says about QUIC, when it runs.
    pub quic: Option<crate::endpoint::QuicInfo>,
    pub source: Source,
}

impl Shared {
    /// The address limits apply to.
    #[must_use]
    pub fn address(&self, peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
        if self.source == Source::Proxy
            && let Some(forwarded) = headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.rsplit(',').next())
                .and_then(|value| value.trim().parse().ok())
        {
            return forwarded;
        }
        peer.ip()
    }
}

/// The relay's own routes, without the dashboard.
pub fn router(shared: Shared) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/info", get(info))
        .route("/v1/listen", get(listen))
        .route("/v1/dial/{route}", get(dial))
        .route("/v1/accept/{session}", get(accept))
        .with_state(shared)
}

async fn info(State(shared): State<Shared>) -> Json<Info> {
    Json(Info {
        listen_token: shared.hub.access.requires_token(),
        protocol: PROTOCOL,
        quic: shared.quic.clone(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        websocket: shared.public_host.is_some(),
    })
}

fn refused(reason: Refused) -> Response {
    match reason {
        Refused::PerAddress => StatusCode::TOO_MANY_REQUESTS.into_response(),
        Refused::Total => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// Sends a challenge and reads the peer's proof: the nonce it signed over,
/// and its hello. `None` when the peer does not answer in time or sends
/// something else.
async fn challenge(socket: &mut WebSocket) -> Option<([u8; NONCE_LEN], Hello)> {
    let mut nonce = [0; NONCE_LEN];
    getrandom::fill(&mut nonce).ok()?;
    let challenge = ToPeer::Challenge {
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        version: PROTOCOL,
    };
    if !send(socket, &challenge).await {
        return None;
    }
    match timeout(HELLO_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => Some((nonce, serde_json::from_str(&text).ok()?)),
        _ => None,
    }
}

async fn listen(
    State(shared): State<Shared>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(host) = shared.public_host.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let address = shared.address(peer, &headers);
    let permit = match shared.hub.admit(address, Kind::Listen) {
        Ok(permit) => permit,
        Err(reason) => return refused(reason),
    };
    let hub = Arc::clone(&shared.hub);
    upgrade
        .max_message_size(MAX_CONTROL)
        .max_frame_size(MAX_CONTROL)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            serve_listener(socket, hub, address, host).await;
        })
}

async fn serve_listener(mut socket: WebSocket, hub: Arc<Hub>, address: IpAddr, host: Arc<str>) {
    let Some((nonce, hello)) = challenge(&mut socket).await else {
        return close(socket, code::PROTOCOL).await;
    };
    let token = hello.token.as_deref().unwrap_or_default().as_bytes();
    let Some((key, signature)) = hello.decode() else {
        return close(socket, code::REFUSED).await;
    };
    let signed = socket::listen_signed(&host, &nonce);
    let mut registration = match broker::listen(&hub, address, token, &key, &signature, &signed) {
        Ok(registration) => registration,
        Err(Status::Full) => return close(socket, code::FULL).await,
        Err(_) => return close(socket, code::REFUSED).await,
    };
    let ready = ToPeer::Ready {
        route: registration.route.to_string(),
    };
    if !send(&mut socket, &ready).await {
        return;
    }
    // A listener that sends nothing, not even a pong, for the silence limit
    // is gone: its route and keys are dropped, as QUIC's idle timeout drops
    // a silent connection.
    let silence = hub.limits.listener_silence;
    let mut ping = tokio::time::interval(PING_INTERVAL.min(silence / 3));
    ping.reset();
    let mut heard = tokio::time::Instant::now();
    loop {
        tokio::select! {
            incoming = registration.incoming.recv() => {
                let Some(incoming) = incoming else {
                    hub.metrics.listen(Listen::Replaced);
                    return close(socket, code::REPLACED).await;
                };
                let message = ToPeer::Incoming {
                    key: URL_SAFE_NO_PAD.encode(incoming.key),
                    session: incoming.session.to_string(),
                };
                if !send(&mut socket, &message).await {
                    return;
                }
            }
            frame = socket.recv() => match frame {
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => heard = tokio::time::Instant::now(),
                Some(Ok(Message::Text(text))) => {
                    heard = tokio::time::Instant::now();
                    let message = serde_json::from_str::<FromListener>(&text)
                        .ok()
                        .and_then(|message| message.decode());
                    let Some(message) = message else {
                        return close(socket, code::PROTOCOL).await;
                    };
                    let status = broker::apply(&registration, message);
                    if !send(&mut socket, &ToPeer::Status { status }).await {
                        return;
                    }
                }
                Some(Ok(Message::Binary(_))) => return close(socket, code::PROTOCOL).await,
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
            },
            _ = ping.tick() => {
                if heard.elapsed() >= silence {
                    return close(socket, code::IDLE).await;
                }
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn dial(
    State(shared): State<Shared>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(route): Path<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let hub = &shared.hub;
    let Some(host) = shared.public_host.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(route) = RouteId::parse(&route) else {
        hub.metrics.dial(Dial::BadRoute);
        return StatusCode::NOT_FOUND.into_response();
    };
    let address = shared.address(peer, &headers);
    let permit = match hub.admit(address, Kind::Dial) {
        Ok(permit) => permit,
        Err(reason) => return refused(reason),
    };
    let hub = Arc::clone(hub);
    // The hello is a few hundred bytes, under the smallest frame allowed.
    let max_frame = hub.limits.max_frame;
    upgrade
        .max_message_size(max_frame)
        .max_frame_size(max_frame)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            serve_dialer(socket, hub, address, host, route).await;
        })
}

async fn serve_dialer(
    mut socket: WebSocket,
    hub: Arc<Hub>,
    address: IpAddr,
    host: Arc<str>,
    route: RouteId,
) {
    let Some((nonce, hello)) = challenge(&mut socket).await else {
        return close(socket, code::PROTOCOL).await;
    };
    let Some((key, signature)) = hello.decode() else {
        return answer(socket, Status::Offline).await;
    };
    let signed = socket::dial_signed(&host, &nonce, &route);
    let (accepted, slot) = match broker::dial(&hub, address, route, &key, &signature, &signed).await
    {
        Ok(dialed) => dialed,
        Err(status) => return answer(socket, status).await,
    };
    if !send(&mut socket, &ToPeer::Status { status: Status::Ok }).await {
        return;
    }
    let session = hub.open(slot);
    let _permit = accepted.permit;
    splice(Peer::Socket(Box::new(socket)), accepted.peer, session).await;
}

/// Answers a dial that will not go through, then closes.
async fn answer(mut socket: WebSocket, status: Status) {
    let _ = send(&mut socket, &ToPeer::Status { status }).await;
    let code = match status {
        Status::Offline => code::OFFLINE,
        _ => code::PEER_GONE,
    };
    close(socket, code).await;
}

async fn accept(
    State(shared): State<Shared>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(session): Path<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let hub = &shared.hub;
    let permit = match hub.admit(shared.address(peer, &headers), Kind::Accept) {
        Ok(permit) => permit,
        Err(reason) => return refused(reason),
    };
    let Some(accepted) = SessionId::parse(&session).and_then(|session| hub.accept(session)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let max_frame = hub.limits.max_frame;
    upgrade
        .max_message_size(max_frame)
        .max_frame_size(max_frame)
        .on_upgrade(move |socket| async move {
            // The dialer's task owns both for the session's life.
            let accepted_peer = Accepted {
                peer: Peer::Socket(Box::new(socket)),
                permit: Some(permit),
            };
            if let Err(Accepted {
                peer: Peer::Socket(socket),
                ..
            }) = accepted.send(accepted_peer)
            {
                close(*socket, code::PEER_GONE).await;
            }
        })
}

async fn send(socket: &mut WebSocket, message: &ToPeer) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    socket.send(Message::Text(text.into())).await.is_ok()
}

async fn close(mut socket: WebSocket, code: u16) {
    let frame = CloseFrame {
        code,
        reason: "".into(),
    };
    let _ = socket.send(Message::Close(Some(frame))).await;
}
