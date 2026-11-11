//! Starting the relay: QUIC, its HTTP routes and dashboard, and its
//! metrics port.

use crate::{
    Error, Options, Result,
    dashboard::{self, auth::Account},
    endpoint::QuicInfo,
    hub::{Access, Hub},
    quic::{self, tls::Identity},
    secret::Secret,
    web::{self, Shared, Source},
};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::ExposeSecret;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

/// The shortest listen or metrics token the relay accepts.
const MIN_TOKEN: usize = 16;

/// The relay's HTTP routes and, with a password, the dashboard. `quic` is
/// what `/v1/info` says about QUIC.
pub fn app(options: &Options, quic: Option<QuicInfo>) -> Result<(Router, Arc<Hub>)> {
    let hub = Arc::new(Hub::new(options.limits, access(options)?));
    let shared = Shared {
        hub: Arc::clone(&hub),
        public_host: options.public_host.as_deref().map(Arc::from),
        quic,
        source: source(options),
    };
    let mut router = web::router(shared.clone());
    if let Some(password) = &options.dashboard_password {
        let account = Account::new(&options.dashboard_user, password.expose_secret())?;
        router = router.merge(dashboard::router(shared, account));
    }
    Ok((router, hub))
}

/// Where callers' addresses come from, for per-address limits.
fn source(options: &Options) -> Source {
    let header = options
        .client_ip_header
        .as_deref()
        .and_then(|name| axum::http::HeaderName::from_bytes(name.as_bytes()).ok());
    match (header, options.behind_proxy) {
        (Some(header), _) => Source::Header(header),
        (None, true) => Source::Proxy,
        (None, false) => Source::Peer,
    }
}

/// Who may listen: anyone, unless the operator set a token.
fn access(options: &Options) -> Result<Access> {
    let Some(token) = &options.listen_token else {
        return Ok(Access::Open);
    };
    let token = token.expose_secret();
    strong(token, "the listen token")?;
    Ok(Access::Token(Secret::new(token.as_bytes())?))
}

/// The metrics port's routes.
pub fn metrics_app(hub: Arc<Hub>, token: Option<Secret>) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state((hub, token.map(Arc::new)))
}

async fn metrics(
    State((hub, token)): State<(Arc<Hub>, Option<Arc<Secret>>)>,
    headers: HeaderMap,
) -> Response {
    if let Some(token) = token {
        let given = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !token.matches(given.as_bytes()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        hub.metrics.render(),
    )
        .into_response()
}

fn strong(token: &str, name: &'static str) -> Result<()> {
    if token.chars().count() < MIN_TOKEN {
        return Err(Error::WeakSecret {
            min: MIN_TOKEN,
            name,
        });
    }
    Ok(())
}

async fn bind(address: SocketAddr) -> Result<TcpListener> {
    TcpListener::bind(address)
        .await
        .map_err(|source| Error::Bind { address, source })
}

/// Runs the relay until it is told to stop.
pub async fn serve(options: Options) -> Result<()> {
    let quic = options
        .quic
        .map(|address| bind_quic(address, options.quic_port))
        .transpose()?;
    let (router, hub) = app(&options, quic.as_ref().map(|(_, info)| info.clone()))?;
    if let Some((endpoint, info)) = &quic {
        tokio::spawn(quic::run(endpoint.clone(), Arc::clone(&hub)));
        tracing::info!(port = info.port, pin = %info.pin, "serving QUIC");
    } else {
        tracing::info!("QUIC is off: clients use the WebSocket fallback");
    }
    let metrics_token = options
        .metrics_token
        .as_ref()
        .map(ExposeSecret::expose_secret)
        .map(|token| {
            strong(token, "the metrics token").and_then(|()| Secret::new(token.as_bytes()))
        })
        .transpose()?;
    let listener = bind(options.listen).await?;
    if let Some(address) = options.metrics {
        let metrics = bind(address).await?;
        let app = metrics_app(Arc::clone(&hub), metrics_token);
        tokio::spawn(async move {
            if let Err(error) = axum::serve(metrics, app)
                .with_graceful_shutdown(stopped())
                .await
            {
                tracing::error!(%error, "the metrics server stopped");
            }
        });
        tracing::info!(%address, "serving /metrics");
    }
    if options.public_host.is_none() {
        tracing::warn!("the WebSocket fallback is off: set RELAY_PUBLIC_HOST to the relay's name");
    }
    if options.dashboard_password.is_some() {
        tracing::info!("the dashboard is at /dashboard");
    } else {
        tracing::info!("the dashboard is off: set RELAY_DASHBOARD_PASSWORD to turn it on");
    }
    tracing::info!(
        address = %options.listen,
        limits = ?options.limits,
        "the relay is listening"
    );
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(stopped())
    .await
    .map_err(Error::Serve)?;
    if let Some((endpoint, _)) = quic {
        endpoint.close(quinn::VarInt::from_u32(crate::wire::code::SHUTDOWN), b"");
        endpoint.wait_idle().await;
    }
    Ok(())
}

/// Binds QUIC on `address` with a fresh identity; what clients learn of it.
pub fn bind_quic(address: SocketAddr, port: Option<u16>) -> Result<(quinn::Endpoint, QuicInfo)> {
    let identity = Identity::generate()?;
    let endpoint = quic::bind(address, &identity)?;
    let bound = endpoint
        .local_addr()
        .map_err(|source| quic::Error::Bind { address, source })?;
    let info = QuicInfo {
        pin: URL_SAFE_NO_PAD.encode(identity.pin),
        port: port.unwrap_or(bound.port()),
    };
    Ok((endpoint, info))
}

/// Resolves on Ctrl-C or, where there is one, `SIGTERM`, which is what
/// `docker stop` sends.
async fn stopped() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
