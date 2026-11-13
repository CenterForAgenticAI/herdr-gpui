//! One relay: connect over QUIC, listen on this computer's route, name the
//! paired devices' dial keys, and hand each dial to [`super::serve`];
//! reconnect with backoff when anything breaks.

use super::{Error, Relays, Result, Status, serve};
use crate::web::Shared;
use serde::Deserialize;
use sheprd_relay::{
    Endpoint,
    client::{self, Listener},
    wire::Key,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::watch;

/// Reconnects wait this long at first, doubling to [`LONGEST`].
const FIRST: Duration = Duration::from_secs(1);
const LONGEST: Duration = Duration::from_secs(60);
/// How long reading `/v1/info` may take.
const INFO_TIMEOUT: Duration = Duration::from_secs(10);

/// Keeps listening on `endpoint` until `stopped`.
pub(super) async fn run(
    relays: Arc<Relays>,
    endpoint: Endpoint,
    state: Shared,
    router: axum::Router,
    mut stopped: watch::Receiver<bool>,
) {
    let mut wait = FIRST;
    loop {
        relays.set_status(&endpoint, Status::Connecting);
        let result = tokio::select! {
            result = session(&relays, &endpoint, &state, &router, &mut wait) => result,
            _ = stopped.wait_for(|stop| *stop) => return,
        };
        if let Err(error) = result {
            tracing::warn!(relay = %endpoint, error = %error, "relay listening failed");
            relays.set_status(
                &endpoint,
                Status::Failed {
                    error: error.to_string(),
                },
            );
        }
        // Jitter, so a relay restarting does not see every computer at once.
        let jitter = Duration::from_millis(u64::from(rand_byte()) * 4);
        tokio::select! {
            () = tokio::time::sleep(wait + jitter) => {}
            _ = stopped.wait_for(|stop| *stop) => return,
        }
        wait = (wait * 2).min(LONGEST);
    }
}

fn rand_byte() -> u8 {
    let mut byte = [0];
    let _ = getrandom::fill(&mut byte);
    byte[0]
}

/// One connection's life: listen, name keys, serve dials, until it ends.
async fn session(
    relays: &Arc<Relays>,
    endpoint: &Endpoint,
    state: &Shared,
    router: &axum::Router,
    wait: &mut Duration,
) -> Result<()> {
    let (address, pin) = quic(endpoint).await?;
    let connection = client::connect(address, pin).await?;
    let mut listener = Listener::listen(connection, &relays.keys.route, b"").await?;
    let mut changed = relays.changed.subscribe();
    let mut revoked = state.auth().watch_revocations();
    listener.set_dial_keys(dial_keys(state)).await?;
    relays.set_status(endpoint, Status::Listening);
    tracing::info!(relay = %endpoint, "listening on a relay");
    // Listening worked: the next failure starts the backoff over.
    *wait = FIRST;
    loop {
        tokio::select! {
            incoming = listener.accept() => {
                let incoming = incoming?;
                tokio::spawn(serve::session(
                    Arc::clone(relays),
                    incoming,
                    listener.route(),
                    state.clone(),
                    router.clone(),
                ));
            }
            _ = changed.changed() => listener.set_dial_keys(dial_keys(state)).await?,
            _ = revoked.changed() => listener.set_dial_keys(dial_keys(state)).await?,
            error = listener.closed() => return Err(Error::Client(error.into())),
        }
    }
}

/// The dial keys of every paired device that gave one.
fn dial_keys(state: &Shared) -> Vec<Key> {
    state
        .auth()
        .devices()
        .records()
        .iter()
        .filter_map(|record| record.relay.as_ref()?.dial_key())
        .collect()
}

#[derive(Deserialize)]
struct Info {
    quic: Option<Quic>,
}

#[derive(Deserialize)]
struct Quic {
    pin: String,
    port: u16,
}

/// Where the relay's QUIC listens, and its certificate's pin, from its
/// `/v1/info` over HTTPS.
async fn quic(endpoint: &Endpoint) -> Result<(SocketAddr, sheprd_relay::pin::Pin)> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let url = endpoint.info().to_string();
    let info = tokio::task::spawn_blocking(move || -> Result<Info> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(INFO_TIMEOUT))
            .max_redirects(0)
            .build()
            .into();
        let mut response = agent.get(&url).call().map_err(Error::Info)?;
        let body = response
            .body_mut()
            .with_config()
            .limit(16 * 1024)
            .read_to_vec()
            .map_err(Error::Info)?;
        serde_json::from_slice(&body).map_err(|_| Error::InfoFormat)
    })
    .await
    .map_err(|_| Error::InfoFormat)??;
    let quic = info.quic.ok_or(Error::NoQuic)?;
    let pin = URL_SAFE_NO_PAD
        .decode(quic.pin)
        .ok()
        .and_then(|pin| pin.try_into().ok())
        .ok_or(Error::InfoFormat)?;
    let host = endpoint.hostname();
    let address = tokio::net::lookup_host((host.as_str(), quic.port))
        .await
        .map_err(Error::Resolve)?
        .next()
        .ok_or_else(|| Error::Resolve(std::io::Error::other("no address")))?;
    Ok((address, pin))
}
