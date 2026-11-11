//! Explicit app-owned adapters for link-handler commands. No daemon registry
//! or terminal-provided command is imported. Only a completed click calls `run`.

mod error;
mod run;
pub(crate) use error::Error;
pub(crate) use run::run;

use regex::Regex;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Handler {
    pub(crate) plugin: String,
    pub(crate) id: String,
    /// Saved device scope, or `local`. Never a wildcard.
    pub(crate) host: String,
    pub(crate) pattern: String,
    pub(crate) command: Vec<String>,
    /// Trusted plugin directory on the host, not the terminal cwd.
    pub(crate) directory: String,
    #[serde(default = "enabled")]
    pub(crate) enabled: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Handlers(Vec<(Handler, Regex)>);

impl Handlers {
    pub(crate) fn compile(mut handlers: Vec<Handler>) -> Result<Self, Error> {
        if handlers.len() > 64 {
            return Err(Error::Invalid("at most 64 handlers are allowed"));
        }
        // Stable sorting preserves manifest order inside each plugin.
        handlers.sort_by(|a, b| a.plugin.cmp(&b.plugin));
        let mut seen = std::collections::HashSet::new();
        let mut compiled = Vec::new();
        for handler in handlers {
            if [&handler.plugin, &handler.id, &handler.host]
                .into_iter()
                .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
                || !absolute_directory(&handler.directory)
                || handler.directory.len() > 4096
                || handler.directory.chars().any(char::is_control)
                || handler.command.is_empty()
                || handler.command.len() > 64
                || handler.command[0].is_empty()
                || handler
                    .command
                    .iter()
                    .any(|s| s.len() > 8192 || s.contains('\0'))
                || handler.pattern.len() > 4096
            {
                return Err(Error::Invalid(
                    "invalid identity, directory, command, or pattern bounds",
                ));
            }
            if !seen.insert((
                handler.host.clone(),
                handler.plugin.clone(),
                handler.id.clone(),
            )) {
                return Err(Error::Invalid("duplicate handler ID for a plugin and host"));
            }
            let pattern = Regex::new(&handler.pattern)?;
            compiled.push((handler, pattern));
        }
        Ok(Self(compiled))
    }

    pub(crate) fn matching(&self, host: &str, url: &str) -> Option<&Handler> {
        if url.is_empty() || url.len() > 32768 || url.chars().any(char::is_control) {
            return None;
        }
        self.0
            .iter()
            .find(|(handler, pattern)| {
                handler.enabled && handler.host == host && pattern.is_match(url)
            })
            .map(|(handler, _)| handler)
    }
}

fn absolute_directory(path: &str) -> bool {
    path.starts_with('/')
        || (path.as_bytes().get(1) == Some(&b':')
            && path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && matches!(path.as_bytes().get(2), Some(b'/' | b'\\')))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
