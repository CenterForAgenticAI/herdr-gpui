//! The dashboard's login: one access token, a signed session cookie, and
//! the login form's field.
//!
//! The token, not a password, is what keeps guessing hopeless: at least 24
//! characters, and `deploy/relay` generates 32 random bytes. The lockout in
//! `lockout` then stops an address after three wrong tries.

use crate::secret::Secret;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::percent_decode_str;
use sha2::Sha256;
use std::{fmt, time::Duration};

/// The cookie's name.
pub const COOKIE: &str = "sheprd_relay";
/// How long a login lasts.
pub const SESSION: Duration = Duration::from_secs(12 * 60 * 60);
/// The shortest token the relay accepts. `openssl rand -base64 32` or
/// cloud-init's own gives 43 or more.
pub const MIN_TOKEN: usize = 24;
/// The longest field the login form may carry.
const MAX_FIELD: usize = 256;
const CONTEXT: &[u8] = b"sheprd-relay/dashboard/v1\0";

/// Who may read the dashboard: whoever holds the token. Also the key its
/// cookies are signed with.
pub struct Account {
    /// Drawn at start, so a restart signs everyone out.
    signing: [u8; 64],
    token: Secret,
}

impl fmt::Debug for Account {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Account(…)")
    }
}

impl Account {
    /// An account for `token`, refusing a short one.
    pub fn new(token: &str) -> crate::Result<Self> {
        if token.chars().count() < MIN_TOKEN {
            return Err(crate::Error::WeakSecret {
                min: MIN_TOKEN,
                name: "the dashboard token",
            });
        }
        let mut signing = [0; 64];
        getrandom::fill(&mut signing).map_err(crate::Error::Random)?;
        Ok(Self {
            signing,
            token: Secret::new(token.as_bytes())?,
        })
    }

    /// Whether `token` is the dashboard's, in constant time.
    #[must_use]
    pub fn check(&self, token: &str) -> bool {
        self.token.matches(token.as_bytes())
    }

    fn mac(&self, expires: u64) -> Hmac<Sha256> {
        let mut mac = <Hmac<Sha256> as KeyInit>::new(&self.signing.into());
        mac.update(CONTEXT);
        mac.update(&expires.to_be_bytes());
        mac
    }

    /// A cookie value good until `now_secs` plus [`SESSION`].
    #[must_use]
    pub fn issue(&self, now_secs: u64) -> String {
        let expires = now_secs.saturating_add(SESSION.as_secs());
        let tag = self.mac(expires).finalize().into_bytes();
        format!("{expires}.{}", URL_SAFE_NO_PAD.encode(tag))
    }

    /// When a cookie this relay issued expires, if it has not yet.
    #[must_use]
    pub fn expiry(&self, value: &str, now_secs: u64) -> Option<u64> {
        let (expires, _) = value.split_once('.')?;
        let expires = expires.parse().ok()?;
        self.valid(value, now_secs).then_some(expires)
    }

    /// Whether `value` is a cookie this relay issued that has not expired.
    #[must_use]
    pub fn valid(&self, value: &str, now_secs: u64) -> bool {
        let Some((expires, tag)) = value.split_once('.') else {
            return false;
        };
        let (Ok(expires), Ok(tag)) = (expires.parse::<u64>(), URL_SAFE_NO_PAD.decode(tag)) else {
            return false;
        };
        expires > now_secs && self.mac(expires).verify_slice(&tag).is_ok()
    }
}

/// The session cookie's value in a `Cookie` header, if present.
#[must_use]
pub fn cookie(header: &str) -> Option<&str> {
    header.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == COOKIE).then_some(value)
    })
}

/// The `Set-Cookie` value that stores `value`, or clears it when `None`.
#[must_use]
pub fn set_cookie(value: Option<&str>) -> String {
    let attributes = "Path=/dashboard; HttpOnly; Secure; SameSite=Strict";
    match value {
        Some(value) => format!(
            "{COOKIE}={value}; Max-Age={}; {attributes}",
            SESSION.as_secs()
        ),
        None => format!("{COOKIE}=; Max-Age=0; {attributes}"),
    }
}

/// The token from a urlencoded login form.
#[must_use]
pub fn login_form(body: &[u8]) -> Option<String> {
    let body = std::str::from_utf8(body).ok()?;
    let mut token = None;
    for pair in body.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if value.len() > MAX_FIELD * 3 {
            return None;
        }
        if name == "token" {
            let value = value.replace('+', " ");
            token = Some(
                percent_decode_str(&value)
                    .decode_utf8()
                    .ok()?
                    .trim()
                    .to_owned(),
            );
        }
    }
    token
}
