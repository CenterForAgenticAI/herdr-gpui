//! Sheprd's own relays, listed in one static file on sheprd.org, so they
//! can be added or all removed without a release: [`URL`].
//!
//! ```json
//! {
//!   "version": 1,
//!   "relays": [
//!     { "endpoint": "https://fra1.relay.sheprd.org", "region": "fra1", "name": "Frankfurt" }
//!   ]
//! }
//! ```
//!
//! An empty `relays` list is a valid answer: no Sheprd relays are
//! available for now. Clients fetch it from the compiled-in [`URL`] only,
//! over HTTPS, and this parser is strict: bounded size and count, `https`
//! endpoints only, duplicates dropped, unknown fields ignored so the file
//! can grow. It is not signed: a relay sees only end-to-end encrypted
//! bytes, so a tampered list can send clients to relays that drop traffic,
//! never to ones that read it.

mod error;

pub use error::Error;

use crate::Endpoint;
use serde::Deserialize;

/// Where Sheprd's relays are listed. Compiled in, so a paired device or
/// a page cannot redirect a client's relay list.
pub const URL: &str = "https://sheprd.org/relays.json";
/// How to run a relay, with the one-click DigitalOcean deploy: where
/// clients link when no Sheprd relays are available, and next to the
/// private relay setting.
pub const SETUP_URL: &str = "https://sheprd.org/docs/relays/";
/// The format this client reads.
pub const VERSION: u32 = 1;
/// The largest list a client accepts.
pub const MAX_BYTES: usize = 64 * 1024;
/// The most relays a list may name.
pub const MAX_RELAYS: usize = 64;
/// The longest region or name.
const MAX_LABEL: usize = 64;

/// One of Sheprd's relays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relay {
    pub endpoint: Endpoint,
    /// A short region code, such as `fra1`, when the list gives one.
    pub region: Option<String>,
    /// A name to show, such as `Frankfurt`, when the list gives one.
    pub name: Option<String>,
}

/// Sheprd's relays as the list names them; possibly none.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Directory {
    pub relays: Vec<Relay>,
}

#[derive(Deserialize)]
struct Raw {
    version: u32,
    relays: Vec<RawRelay>,
}

#[derive(Deserialize)]
struct RawRelay {
    endpoint: String,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// A label to show, if it is short, printable, and not blank.
fn label(text: Option<String>, field: &'static str) -> Result<Option<String>, Error> {
    let Some(text) = text.map(|text| text.trim().to_owned()) else {
        return Ok(None);
    };
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > MAX_LABEL || text.chars().any(char::is_control) {
        return Err(Error::Label { field });
    }
    Ok(Some(text))
}

impl Directory {
    /// Reads the list from `bytes`, as served at [`URL`].
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let raw: Raw = serde_json::from_slice(bytes).map_err(|_| Error::Format)?;
        if raw.version != VERSION {
            return Err(Error::Version {
                version: raw.version,
            });
        }
        if raw.relays.len() > MAX_RELAYS {
            return Err(Error::TooMany);
        }
        let mut relays: Vec<Relay> = Vec::with_capacity(raw.relays.len());
        for (index, entry) in raw.relays.into_iter().enumerate() {
            let endpoint = Endpoint::parse(&entry.endpoint)
                .map_err(|source| Error::Endpoint { index, source })?;
            if !endpoint.is_secure() {
                return Err(Error::Insecure { index });
            }
            if relays.iter().any(|relay| relay.endpoint == endpoint) {
                continue;
            }
            relays.push(Relay {
                endpoint,
                region: label(entry.region, "region")?,
                name: label(entry.name, "name")?,
            });
        }
        Ok(Self { relays })
    }

    /// Whether Sheprd has no relays available for now.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.relays.is_empty()
    }
}
