//! The `custom_headers` the GitLab CLI sends to an instance, such as the
//! service token an access proxy like Cloudflare Access asks for in front of
//! a self-managed GitLab. Each is `name` and one of `value`, `valueFromEnv`
//! or `valueFromCommand`; as glab does, a header that cannot be made fails
//! every request rather than sending the rest. A command is never run: that
//! header refuses and asks for `valueFromEnv` instead.
//!
//! Header values are secrets. They are only sent over HTTPS, to the
//! instance's API host, by an agent that follows no redirect, and are marked
//! sensitive so `Debug` shows `Sensitive`; no log, error or diagnostic names a
//! value, only the header's and the variable's names.

use super::Error;
use std::ffi::OsString;
use ureq::http::{HeaderName, HeaderValue};

/// One `custom_headers` item, as written.
#[derive(Default)]
pub(super) struct CustomHeader {
    name: Option<String>,
    value: Option<String>,
    from_env: Option<String>,
    from_command: bool,
}

impl CustomHeader {
    /// Takes one of the item's keys; others are ignored.
    pub(super) fn set(&mut self, key: &str, value: Option<String>) {
        match key {
            "name" => self.name = value,
            "value" => self.value = value,
            "valueFromEnv" => self.from_env = value,
            "valueFromCommand" => self.from_command = true,
            _ => {}
        }
    }
}

/// An instance's headers, resolved when glab's config is read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers {
    sent: Vec<(HeaderName, HeaderValue)>,
    /// Why requests to the instance are refused, when a header could not be
    /// made.
    refused: Option<Refusal>,
}

/// A header that could not be made; names only, never a value.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Refusal {
    Unset { header: String, variable: String },
    Command { header: String },
    Invalid { header: String },
}

impl Headers {
    /// `list`'s headers, with `valueFromEnv` looked up with `variable`. The
    /// first that cannot be made refuses the instance's requests.
    pub(super) fn resolve(
        list: Vec<CustomHeader>,
        variable: &dyn Fn(&str) -> Option<OsString>,
    ) -> Self {
        let mut headers = Self::default();
        for item in list {
            match header(item, variable) {
                Ok(pair) => headers.sent.push(pair),
                Err(refusal) => {
                    headers.refused = Some(refusal);
                    headers.sent.clear();
                    break;
                }
            }
        }
        headers
    }

    /// The header names sent, for display and tests.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.sent.iter().map(|(name, _)| name.as_str())
    }

    /// Whether `name` is sent with `value`. For tests, which cannot read a
    /// sensitive value back out of `Debug`.
    pub fn sends(&self, name: &str, value: &str) -> bool {
        self.sent.iter().any(|(sent, sent_value)| {
            sent.as_str().eq_ignore_ascii_case(name) && sent_value == value
        })
    }

    /// `request` with the headers, over HTTPS only: over plain HTTP they are
    /// withheld, as a token is. A header that could not be made refuses the
    /// request.
    pub(crate) fn apply<B>(
        &self,
        mut request: ureq::RequestBuilder<B>,
        https: bool,
    ) -> Result<ureq::RequestBuilder<B>, Error> {
        if let Some(refusal) = &self.refused {
            return Err(refusal.error());
        }
        if https {
            for (name, value) in &self.sent {
                request = request.header(name.clone(), value.clone());
            }
        }
        Ok(request)
    }
}

impl Refusal {
    fn error(&self) -> Error {
        match self {
            Self::Unset { header, variable } => Error::HeaderUnset {
                header: header.clone(),
                variable: variable.clone(),
            },
            Self::Command { header } => Error::HeaderCommand {
                header: header.clone(),
            },
            Self::Invalid { header } => Error::HeaderInvalid {
                header: header.clone(),
            },
        }
    }
}

fn header(
    item: CustomHeader,
    variable: &dyn Fn(&str) -> Option<OsString>,
) -> Result<(HeaderName, HeaderValue), Refusal> {
    let label = item.name.clone().unwrap_or_default();
    let invalid = || Refusal::Invalid {
        header: label.clone(),
    };
    let name = item
        .name
        .as_deref()
        .and_then(|name| HeaderName::from_bytes(name.as_bytes()).ok())
        .ok_or_else(invalid)?;
    let sources = usize::from(item.value.is_some())
        + usize::from(item.from_env.is_some())
        + usize::from(item.from_command);
    if sources != 1 {
        return Err(invalid());
    }
    if item.from_command {
        return Err(Refusal::Command { header: label });
    }
    let text = match (item.value, item.from_env) {
        (Some(value), _) => value,
        (None, Some(name)) => match variable(&name).filter(|value| !value.is_empty()) {
            Some(value) => value.into_string().map_err(|_| invalid())?,
            None => {
                return Err(Refusal::Unset {
                    header: label,
                    variable: name,
                });
            }
        },
        (None, None) => return Err(invalid()),
    };
    let mut value = HeaderValue::from_str(&text).map_err(|_| invalid())?;
    value.set_sensitive(true);
    Ok((name, value))
}

/// The error for a reply that redirected elsewhere, as an access proxy's
/// sign-in page does, naming only the target's scheme and host: its query
/// may carry the proxy's state.
pub(super) fn redirect(
    status: u16,
    headers: &ureq::http::HeaderMap,
    origin: &str,
) -> Option<Error> {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) {
        return None;
    }
    let location = headers.get("location")?.to_str().ok()?;
    let target = url::Url::parse(origin).ok()?.join(location).ok()?;
    let host = target.host_str()?;
    let location = match target.port() {
        Some(port) => format!("{}://{host}:{port}", target.scheme()),
        None => format!("{}://{host}", target.scheme()),
    };
    Some(Error::Redirect {
        origin: origin.to_owned(),
        location,
        status,
    })
}
