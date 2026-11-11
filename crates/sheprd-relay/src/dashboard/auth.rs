//! The dashboard's login: one operator account, a signed session cookie,
//! and the login form's fields.

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
/// The shortest password the relay accepts.
pub const MIN_PASSWORD: usize = 12;
/// The longest field the login form may carry.
const MAX_FIELD: usize = 256;
const CONTEXT: &[u8] = b"sheprd-relay/dashboard/v1\0";

/// Who may read the dashboard, and the key its cookies are signed with.
pub struct Account {
    password: Secret,
    /// Drawn at start, so a restart signs everyone out.
    signing: [u8; 64],
    user: Secret,
}

impl fmt::Debug for Account {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Account(…)")
    }
}

impl Account {
    /// An account for `user` and `password`, refusing a short password.
    pub fn new(user: &str, password: &str) -> crate::Result<Self> {
        if password.chars().count() < MIN_PASSWORD {
            return Err(crate::Error::WeakSecret {
                min: MIN_PASSWORD,
                name: "the dashboard password",
            });
        }
        let mut signing = [0; 64];
        getrandom::fill(&mut signing).map_err(crate::Error::Random)?;
        Ok(Self {
            password: Secret::new(password.as_bytes())?,
            signing,
            user: Secret::new(user.as_bytes())?,
        })
    }

    /// Whether the pair is right. Both are always checked, so timing does
    /// not tell which one was wrong.
    #[must_use]
    pub fn check(&self, user: &str, password: &str) -> bool {
        let user = self.user.matches(user.as_bytes());
        let password = self.password.matches(password.as_bytes());
        user & password
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

/// The user and password from a urlencoded login form.
#[must_use]
pub fn login_form(body: &[u8]) -> Option<(String, String)> {
    let body = std::str::from_utf8(body).ok()?;
    let mut user = None;
    let mut password = None;
    for pair in body.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if value.len() > MAX_FIELD * 3 {
            return None;
        }
        let value = value.replace('+', " ");
        let value = percent_decode_str(&value).decode_utf8().ok()?.into_owned();
        match name {
            "user" => user = Some(value),
            "password" => password = Some(value),
            _ => {}
        }
    }
    Some((user?, password?))
}
