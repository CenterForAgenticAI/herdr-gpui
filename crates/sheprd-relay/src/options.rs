//! The relay's settings. Every one is optional: a flag, or the same name as
//! a `RELAY_*` variable (`--max-connections` is `RELAY_MAX_CONNECTIONS`),
//! with flags winning. Secrets come only from variables or files, never
//! from flags, so they stay out of the process list.

use crate::{
    Error, Limits, Result,
    limits::{parse_bytes, parse_duration},
};
use secrecy::SecretString;
use std::{
    ffi::{OsStr, OsString},
    fs,
    net::SocketAddr,
    path::PathBuf,
};

pub const USAGE: &str = "\
sheprd-relay: joins end-to-end encrypted streams between Sheprd peers.

Usage: sheprd-relay [OPTIONS]

Every option is optional, and each can also be set as a RELAY_* variable
(--max-connections is RELAY_MAX_CONNECTIONS). Flags win over variables.

  --listen ADDRESS               HTTP: the WebSocket fallback, /v1/info, and the
                                 dashboard (default 127.0.0.1:8080)
  --quic ADDRESS|off             QUIC, the main transport, on UDP
                                 (default 127.0.0.1:8443)
  --quic-port PORT               The UDP port clients reach QUIC on, when a
                                 mapping changes it (default: --quic's)
  --metrics ADDRESS|off          Prometheus /metrics (default 127.0.0.1:9090)
  --public-host HOST[:PORT]      The name clients reach this relay by, as in
                                 their endpoint (relay.example.org). WebSocket
                                 proofs are checked against it, never against a
                                 request's Host header; without it the
                                 WebSocket fallback is off. QUIC needs nothing.
  --behind-proxy                 Take the caller's address from X-Forwarded-For.
                                 Only when nothing reaches the relay but that proxy.
  --max-connections N            Open sockets in total (default 10000)
  --max-connections-per-ip N     Per IPv4 address or IPv6 /64 (default 64)
  --max-listeners N              Computers listening (default 5000)
  --max-sessions N               Sessions and waiting dials (default 5000)
  --max-sessions-per-route N     Per computer (default 32)
  --session-bandwidth BYTES      Per session, each way, per second; 0 is
                                 unlimited (default 4MiB)
  --total-bandwidth BYTES        Across sessions, each way, per second
                                 (default 0, unlimited)
  --max-frame BYTES              Largest frame (default 64KiB)
  --idle-timeout DURATION        Close a silent session (default 5m)
  --max-session-duration DURATION  Close any session after (default 24h)
  --accept-timeout DURATION      Wait for a listener to accept (default 10s)
  --dials-per-minute N           Per address (default 120)
  --listens-per-minute N         Per address (default 30)
  --dashboard-user NAME          Dashboard login (default admin)
  --dashboard-password-file FILE The dashboard password, at least 12
                                 characters; or RELAY_DASHBOARD_PASSWORD.
                                 Without one, the dashboard is off.
  --listen-token-file FILE       Only listeners presenting this token may
                                 listen; or RELAY_LISTEN_TOKEN. Dialers never
                                 need it.
  --metrics-token-file FILE      Require `Authorization: Bearer TOKEN` on
                                 /metrics; or RELAY_METRICS_TOKEN.
  --help                         Show this.

BYTES takes k, M, or G (binary): 64k, 4MiB. DURATION takes ms, s, m, h, or d.";

/// What a setting's value is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Address,
    Bytes,
    Count,
    Duration,
    File,
    Flag,
    Port,
    Text,
}

/// Every setting, once: its flag name and kind. The variable is derived.
pub const SETTINGS: &[(&str, Kind)] = &[
    // keep-sorted start
    ("accept-timeout", Kind::Duration),
    ("behind-proxy", Kind::Flag),
    ("dashboard-password-file", Kind::File),
    ("dashboard-user", Kind::Text),
    ("dials-per-minute", Kind::Count),
    ("idle-timeout", Kind::Duration),
    ("listen", Kind::Address),
    ("listen-token-file", Kind::File),
    ("listens-per-minute", Kind::Count),
    ("max-connections", Kind::Count),
    ("max-connections-per-ip", Kind::Count),
    ("max-frame", Kind::Bytes),
    ("max-listeners", Kind::Count),
    ("max-session-duration", Kind::Duration),
    ("max-sessions", Kind::Count),
    ("max-sessions-per-route", Kind::Count),
    ("metrics", Kind::Address),
    ("metrics-token-file", Kind::File),
    ("public-host", Kind::Text),
    ("quic", Kind::Address),
    ("quic-port", Kind::Port),
    ("session-bandwidth", Kind::Bytes),
    ("total-bandwidth", Kind::Bytes),
    // keep-sorted end
];

/// Secrets read only from variables, beside their `*_FILE` settings.
const SECRET_VARIABLES: [&str; 3] = [
    "RELAY_DASHBOARD_PASSWORD",
    "RELAY_LISTEN_TOKEN",
    "RELAY_METRICS_TOKEN",
];

/// The smallest frame a peer can work with, and the largest one the relay
/// will buffer.
const FRAME_RANGE: std::ops::RangeInclusive<u64> = 1024..=(16 * 1024 * 1024);

/// What the relay was started with. `Debug` redacts the secrets.
#[derive(Clone, Debug)]
pub struct Options {
    pub behind_proxy: bool,
    pub dashboard_password: Option<SecretString>,
    pub dashboard_user: String,
    pub limits: Limits,
    pub listen: SocketAddr,
    pub listen_token: Option<SecretString>,
    /// Where `/metrics` is served, or `None` when it is off.
    pub metrics: Option<SocketAddr>,
    pub metrics_token: Option<SecretString>,
    /// The relay's own name, `HOST[:PORT]` as clients' endpoints give it.
    /// WebSocket proofs are checked against it; without it, the WebSocket
    /// fallback is off.
    pub public_host: Option<String>,
    /// Where QUIC listens, or `None` when it is off.
    pub quic: Option<SocketAddr>,
    /// The port clients reach QUIC on, when it differs from `quic`'s.
    pub quic_port: Option<u16>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            behind_proxy: false,
            dashboard_password: None,
            dashboard_user: "admin".to_owned(),
            limits: Limits::default(),
            listen: SocketAddr::from(([127, 0, 0, 1], 8080)),
            listen_token: None,
            metrics: Some(SocketAddr::from(([127, 0, 0, 1], 9090))),
            metrics_token: None,
            public_host: None,
            quic: Some(SocketAddr::from(([127, 0, 0, 1], 8443))),
            quic_port: None,
        }
    }
}

/// The variable a setting reads: `max-frame` is `RELAY_MAX_FRAME`.
#[must_use]
pub fn variable(name: &str) -> String {
    format!("RELAY_{}", name.to_ascii_uppercase().replace('-', "_"))
}

impl Options {
    /// The options from `variables`, then `arguments`, or `None` when they
    /// ask for help. `variables` is `std::env::var_os` outside tests.
    pub fn parse(
        arguments: impl IntoIterator<Item = OsString>,
        variables: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Option<Self>> {
        let mut options = Self::default();
        for (name, kind) in SETTINGS {
            if let Some(value) = variables(&variable(name)) {
                options.apply(name, *kind, Some(&value))?;
            }
        }
        for name in SECRET_VARIABLES {
            if let Some(value) = variables(name) {
                options.secret(name, value)?;
            }
        }
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let text = argument.to_str().unwrap_or_default();
            if text == "--help" || text == "-h" {
                return Ok(None);
            }
            let Some((name, kind)) = text
                .strip_prefix("--")
                .and_then(|name| SETTINGS.iter().find(|(known, _)| *known == name))
            else {
                return Err(Error::Argument { argument });
            };
            let value = match kind {
                Kind::Flag => None,
                _ => Some(arguments.next().ok_or(Error::Argument {
                    argument: argument.clone(),
                })?),
            };
            options.apply(name, *kind, value.as_deref())?;
        }
        Ok(Some(options))
    }

    fn secret(&mut self, name: &'static str, value: OsString) -> Result<()> {
        let text = value
            .into_string()
            .map_err(|value| Error::Invalid { name, value })?;
        let text = Some(text.trim())
            .filter(|text| !text.is_empty())
            .map(SecretString::from);
        match name {
            "RELAY_DASHBOARD_PASSWORD" => self.dashboard_password = text,
            "RELAY_LISTEN_TOKEN" => self.listen_token = text,
            _ => self.metrics_token = text,
        }
        Ok(())
    }

    /// Sets `name` from `value`, which is `None` only for a flag.
    pub fn apply(&mut self, name: &str, kind: Kind, value: Option<&OsStr>) -> Result<()> {
        let name: &'static str = SETTINGS
            .iter()
            .find(|(known, _)| *known == name)
            .map_or("setting", |(known, _)| known);
        let invalid = || Error::Invalid {
            name,
            value: value.map(OsStr::to_owned).unwrap_or_default(),
        };
        let text = value.and_then(OsStr::to_str);
        if kind == Kind::Flag {
            // As a variable, a flag is on unless it says otherwise.
            let on = text.is_none_or(|text| !matches!(text, "0" | "false" | "no" | ""));
            self.behind_proxy = on;
            return Ok(());
        }
        let text = text.ok_or_else(invalid)?;
        let count = || {
            text.parse::<usize>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(invalid)
        };
        let per_minute = || {
            text.parse::<u32>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(invalid)
        };
        let bytes = || parse_bytes(text).ok_or_else(invalid);
        let duration = || {
            parse_duration(text)
                .filter(|duration| !duration.is_zero())
                .ok_or_else(invalid)
        };
        let limits = &mut self.limits;
        match name {
            "accept-timeout" => limits.accept_timeout = duration()?,
            "dashboard-password-file" => self.dashboard_password = Some(read(name, text)?),
            "dashboard-user" if !text.trim().is_empty() => {
                self.dashboard_user = text.trim().to_owned()
            }
            "dials-per-minute" => limits.dials_per_minute = per_minute()?,
            "idle-timeout" => limits.idle_timeout = duration()?,
            "listen" => self.listen = text.parse().map_err(|_| invalid())?,
            "listen-token-file" => self.listen_token = Some(read(name, text)?),
            "listens-per-minute" => limits.listens_per_minute = per_minute()?,
            "max-connections" => limits.max_connections = count()?,
            "max-connections-per-ip" => limits.max_connections_per_ip = count()?,
            "max-frame" => {
                let frame = bytes()?;
                if !FRAME_RANGE.contains(&frame) {
                    return Err(invalid());
                }
                limits.max_frame = usize::try_from(frame).map_err(|_| invalid())?;
            }
            "max-listeners" => limits.max_listeners = count()?,
            "max-session-duration" => limits.max_session_duration = duration()?,
            "max-sessions" => limits.max_sessions = count()?,
            "max-sessions-per-route" => limits.max_sessions_per_route = count()?,
            "metrics" if text == "off" => self.metrics = None,
            "metrics" => self.metrics = Some(text.parse().map_err(|_| invalid())?),
            "metrics-token-file" => self.metrics_token = Some(read(name, text)?),
            "public-host" => self.public_host = Some(public_host(text).ok_or_else(invalid)?),
            "quic" if text == "off" => self.quic = None,
            "quic" => self.quic = Some(text.parse().map_err(|_| invalid())?),
            "quic-port" => {
                self.quic_port = Some(
                    text.parse()
                        .ok()
                        .filter(|port| *port > 0)
                        .ok_or_else(invalid)?,
                );
            }
            "session-bandwidth" => limits.session_bandwidth = bytes()?,
            "total-bandwidth" => limits.total_bandwidth = bytes()?,
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

/// `text` as clients sign it: what [`crate::Endpoint::host`] gives for the
/// same address, so `RELAY.EXAMPLE.ORG:443` becomes `relay.example.org`.
#[must_use]
pub fn public_host(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains('/') || text.contains('@') {
        return None;
    }
    crate::Endpoint::parse(&format!("https://{text}"))
        .ok()
        .map(|endpoint| endpoint.host())
}

/// A secret file's text, trimmed; refuses an empty one.
fn read(name: &'static str, path: &str) -> Result<SecretString> {
    let path = PathBuf::from(path);
    let text = fs::read_to_string(&path).map_err(|source| Error::SecretRead {
        path: path.clone(),
        source,
    })?;
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Invalid {
            name,
            value: path.into_os_string(),
        });
    }
    Ok(SecretString::from(text))
}
