//! The QUIC transport, the relay's main one. A peer holds one connection,
//! which keeps working when it moves between networks, and each session is
//! one bidirectional stream on it. A listener gets each dial as a stream
//! the relay opens on the listener's own connection, so accepting costs no
//! new connection and no extra handshake. `wire` has the protocol.

mod error;
pub mod tls;

pub use error::Error;

use crate::{
    broker,
    hub::{Accepted, Hub, Registration},
    metrics::{Kind, Listen},
    splice::{Peer, splice},
    wire::{
        self, EXPORTER_LABEL, EXPORTER_LEN, INCOMING, MAX_MESSAGE, MAX_REQUEST, Message, Parse,
        Request, Status,
    },
};
use quinn::{Connection, Endpoint, RecvStream, SendStream, ServerConfig, TransportConfig, VarInt};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tls::Identity;
use tokio::time::timeout;

/// How long a new stream has to send its request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// A connection with nothing in flight and no keepalive is dropped after
/// this. Clients send QUIC keepalives more often.
const IDLE: Duration = Duration::from_secs(60);
/// How much one stream, and one connection, may have in flight. quinn's
/// defaults suit a single stream on a short path; a relay hop carries bulk
/// transfers over long ones, where a small window caps throughput at
/// window ÷ round trip.
const STREAM_WINDOW: u32 = 8 * 1024 * 1024;
const CONNECTION_WINDOW: u32 = 32 * 1024 * 1024;
/// Streams one connection may have open at once: a phone's sessions to a
/// few computers, or a computer's listen stream and its sessions.
const MAX_STREAMS: u32 = 64;

/// Binds the QUIC endpoint on `address` with `identity`.
pub fn bind(address: SocketAddr, identity: &Identity) -> Result<Endpoint, Error> {
    let mut transport = TransportConfig::default();
    transport
        .max_concurrent_bidi_streams(VarInt::from_u32(MAX_STREAMS))
        .max_concurrent_uni_streams(VarInt::from_u32(0))
        .datagram_receive_buffer_size(None)
        .max_idle_timeout(IDLE.try_into().ok());
    windows(&mut transport);
    let mut config = ServerConfig::with_crypto(Arc::new(identity.server_crypto()?));
    config.transport_config(Arc::new(transport));
    Endpoint::server(config, address).map_err(|source| Error::Bind { address, source })
}

/// The transport settings clients should use: keepalives under the relay's
/// idle timeout, so NAT mappings and the connection stay open.
#[must_use]
pub fn client_transport() -> TransportConfig {
    let mut transport = TransportConfig::default();
    transport
        .keep_alive_interval(Some(Duration::from_secs(15)))
        .max_idle_timeout(IDLE.try_into().ok())
        .max_concurrent_uni_streams(VarInt::from_u32(0))
        .max_concurrent_bidi_streams(VarInt::from_u32(MAX_STREAMS));
    windows(&mut transport);
    transport
}

fn windows(transport: &mut TransportConfig) {
    transport
        .stream_receive_window(VarInt::from_u32(STREAM_WINDOW))
        .receive_window(VarInt::from_u32(CONNECTION_WINDOW))
        .send_window(u64::from(CONNECTION_WINDOW));
}

/// Accepts connections until the endpoint closes.
pub async fn run(endpoint: Endpoint, hub: Arc<Hub>) {
    while let Some(incoming) = endpoint.accept().await {
        // A source address on a UDP packet can be forged. A stateless retry
        // makes the peer prove it receives at that address before anything
        // counts against it, so forged packets cannot use up another
        // address's connections. It costs one round trip, on a new
        // connection only: migration keeps an existing one.
        if !incoming.remote_address_validated() {
            if incoming.may_retry() {
                let _ = incoming.retry();
            } else {
                incoming.refuse();
            }
            continue;
        }
        let address = incoming.remote_address().ip();
        // Counted before the handshake, so a flood from a proven address
        // costs no cryptography beyond what the per-address limit allows.
        let Ok(permit) = hub.admit(address, Kind::Quic) else {
            incoming.refuse();
            continue;
        };
        let hub = Arc::clone(&hub);
        tokio::spawn(async move {
            let Ok(connection) = incoming.await else {
                return;
            };
            serve(connection, hub).await;
            drop(permit);
        });
    }
}

/// Serves each stream the peer opens until the connection ends.
async fn serve(connection: Connection, hub: Arc<Hub>) {
    while let Ok((send, recv)) = connection.accept_bi().await {
        tokio::spawn(stream(connection.clone(), send, recv, Arc::clone(&hub)));
    }
}

async fn stream(connection: Connection, mut send: SendStream, mut recv: RecvStream, hub: Arc<Hub>) {
    let request = match timeout(REQUEST_TIMEOUT, read_request(&mut recv)).await {
        Ok(Some(request)) => request,
        Ok(None) | Err(_) => return reply_and_end(&mut send, Status::Malformed).await,
    };
    let Some(exported) = export(&connection) else {
        return reply_and_end(&mut send, Status::Refused).await;
    };
    let address = connection.remote_address().ip();
    match request {
        Request::Listen {
            key,
            signature,
            token,
        } => {
            let signed = wire::listen_signed(&exported);
            match broker::listen(&hub, address, &token, &key, &signature, &signed) {
                Ok(registration) => listen(connection, send, recv, hub, registration).await,
                Err(status) => reply_and_end(&mut send, status).await,
            }
        }
        Request::Dial {
            key,
            route,
            signature,
        } => {
            let signed = wire::dial_signed(&exported, &route);
            match broker::dial(&hub, address, route, &key, &signature, &signed).await {
                Ok((accepted, session)) => {
                    if !reply(&mut send, Status::Ok).await {
                        return;
                    }
                    let _permit = accepted.permit;
                    splice(Peer::Stream { recv, send }, accepted.peer, session).await;
                }
                Err(status) => reply_and_end(&mut send, status).await,
            }
        }
    }
}

/// The bytes this connection's proofs sign over.
fn export(connection: &Connection) -> Option<[u8; EXPORTER_LEN]> {
    let mut exported = [0; EXPORTER_LEN];
    connection
        .export_keying_material(&mut exported, EXPORTER_LABEL, b"")
        .ok()?;
    Some(exported)
}

/// Reads exactly one request, never past it: what follows belongs to the
/// session. Not cancel-safe, so it runs alone, under a timeout.
async fn read_request(recv: &mut RecvStream) -> Option<Request> {
    let mut bytes = Vec::with_capacity(MAX_REQUEST);
    loop {
        match Request::parse(&bytes).ok()? {
            Parse::Done(request, _) => return Some(request),
            Parse::Need(total) => {
                let have = bytes.len();
                bytes.resize(total, 0);
                recv.read_exact(&mut bytes[have..]).await.ok()?;
            }
        }
    }
}

/// What a listener's stream has sent so far, kept across the listen loop's
/// turns: a message cut off when a dial arrives is finished on the next
/// turn, never dropped, so the stream cannot fall out of step.
#[derive(Debug, Default)]
pub(crate) struct Messages {
    bytes: Vec<u8>,
}

impl Messages {
    /// Adds `chunk`, then takes every whole message now buffered. `None`
    /// when the bytes are not messages, or more than one could hold.
    pub(crate) fn take(&mut self, chunk: &[u8]) -> Option<Vec<Message>> {
        self.bytes.extend_from_slice(chunk);
        let mut messages = Vec::new();
        loop {
            match Message::parse(&self.bytes).ok()? {
                Parse::Done(message, used) => {
                    self.bytes.drain(..used);
                    messages.push(message);
                }
                Parse::Need(total) if total <= MAX_MESSAGE => return Some(messages),
                Parse::Need(_) => return None,
            }
        }
    }
}

async fn reply_and_end(send: &mut SendStream, status: Status) {
    if reply(send, status).await {
        let _ = send.finish();
    }
}

async fn reply(send: &mut SendStream, status: Status) -> bool {
    send.write_all(&[status as u8]).await.is_ok()
}

/// Holds a listener's stream: applies its messages about who may dial, and
/// opens a stream to it for each accepted dial, until the stream or the
/// connection ends or another listener takes the route.
async fn listen(
    connection: Connection,
    mut send: SendStream,
    mut recv: RecvStream,
    hub: Arc<Hub>,
    mut registration: Registration,
) {
    if !reply(&mut send, Status::Ok).await {
        return;
    }
    let mut messages = Messages::default();
    loop {
        tokio::select! {
            incoming = registration.incoming.recv() => {
                let Some(incoming) = incoming else {
                    hub.metrics.listen(Listen::Replaced);
                    let _ = send.reset(VarInt::from_u32(wire::code::REPLACED));
                    return;
                };
                let Some(accepted) = hub.accept(incoming.session) else {
                    continue;
                };
                let connection = connection.clone();
                let limit = hub.limits.accept_timeout;
                tokio::spawn(async move {
                    // Bounded by the dial's own wait, and dropped as soon
                    // as the dial gives up, so a listener that grants no
                    // stream credit cannot pile up waiting tasks.
                    let open = async {
                        let (mut send, recv) = connection.open_bi().await.ok()?;
                        let header = [&[INCOMING][..], &incoming.key].concat();
                        send.write_all(&header).await.ok()?;
                        Some((send, recv))
                    };
                    let mut accepted = accepted;
                    let opened = tokio::select! {
                        opened = timeout(limit, open) => opened.ok().flatten(),
                        () = accepted.closed() => None,
                    };
                    if let Some((send, recv)) = opened {
                        let _ = accepted.send(Accepted {
                            peer: Peer::Stream { recv, send },
                            permit: None,
                        });
                    }
                });
            }
            // `read_chunk` is cancel-safe: when a dial wins the race, no
            // byte is lost, and `messages` keeps any partial message.
            chunk = recv.read_chunk(MAX_MESSAGE, true) => {
                // The stream ended or broke: stop listening.
                let Ok(Some(chunk)) = chunk else {
                    return;
                };
                // Bytes that are not messages: stop listening.
                let Some(taken) = messages.take(&chunk.bytes) else {
                    return;
                };
                for message in taken {
                    if !reply(&mut send, broker::apply(&registration, message)).await {
                        return;
                    }
                }
            }
            _ = connection.closed() => return,
        }
    }
}
