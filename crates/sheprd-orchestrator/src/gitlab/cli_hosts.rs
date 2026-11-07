//! The GitLab CLI's `hosts` table, read for where an instance's API is when
//! its remotes use another SSH host: glab's `ssh_host`, `api_host`, and
//! `api_protocol` for each instance it is signed in to.
//!
//! glab keeps them in YAML. Only the block form glab writes is read, one
//! `key: value` per line under each host; anything else is skipped, and a
//! remote it does not map keeps its own host as before.
//!
//! Each host's `custom_headers` list is read too, for an instance behind an
//! access proxy such as Cloudflare Access (`headers`).

use super::headers::{CustomHeader, Headers};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// A few instances take a few kilobytes; the rest is room for comments.
const LIMIT: u64 = 256 * 1024;

/// One instance in glab's `hosts` table, with the fields that place its API.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliHost {
    /// The instance's name, which glab saves its token under.
    pub host: String,
    pub api_host: Option<String>,
    pub api_protocol: Option<String>,
    pub ssh_host: Option<String>,
    /// The instance's `custom_headers`, resolved: sent with every request to
    /// its API over HTTPS, and never to another host.
    pub headers: Headers,
}

/// glab's config file, where glab looks for it: `GLAB_CONFIG_DIR`, else the
/// legacy `~/.config/glab-cli` when its file exists, else `glab-cli` in the
/// platform's config folder, which `XDG_CONFIG_HOME` overrides.
pub fn config_path(
    variable: impl Fn(&str) -> Option<OsString>,
    home: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let set = |name| variable(name).filter(|value| !value.is_empty());
    if let Some(dir) = set("GLAB_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("config.yml"));
    }
    let legacy = home.map(|home| home.join(".config/glab-cli/config.yml"));
    if let Some(legacy) = legacy.filter(|path| exists(path)) {
        return Some(legacy);
    }
    let root = match set("XDG_CONFIG_HOME") {
        Some(root) => PathBuf::from(root),
        None if cfg!(target_os = "macos") => home?.join("Library/Application Support"),
        None if cfg!(windows) => PathBuf::from(set("LOCALAPPDATA")?),
        None => home?.join(".config"),
    };
    Some(root.join("glab-cli/config.yml"))
}

/// The instances glab's config on this machine lists, or none when it has
/// no readable one. The variables come from the login shell, as they do for
/// the `glab` that `cli_token` runs, so a launch from Finder or a
/// desktop entry finds the same file. Reads a file and may probe the shell:
/// call it from a worker.
pub fn read() -> Vec<CliHost> {
    let home = sheprd_client::login_env::var("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(sheprd_client::local_path::home_dir);
    let Some(path) = config_path(
        sheprd_client::login_env::var,
        home.as_deref(),
        Path::is_file,
    ) else {
        return Vec::new();
    };
    let mut text = String::new();
    let read = File::open(&path).and_then(|file| file.take(LIMIT + 1).read_to_string(&mut text));
    match read {
        Ok(size) if size as u64 <= LIMIT => parse_in(&text, Some(&sheprd_client::login_env::var)),
        Ok(_) => Vec::new(),
        Err(error) => {
            tracing::debug!(
                category = "orchestrator_gitlab",
                %error,
                "The GitLab CLI's config could not be read"
            );
            Vec::new()
        }
    }
}

/// The `hosts` table of glab's config. Each host is a key one level in, and
/// its fields the `key: value` lines one level further; nested lists such as
/// `custom_headers` are deeper and skipped.
pub fn parse(text: &str) -> Vec<CliHost> {
    parse_in(text, None)
}

/// [`parse`], with each host's `custom_headers` list read too and its
/// `valueFromEnv` names looked up with `variable`.
pub fn parse_with(text: &str, variable: &Variable<'_>) -> Vec<CliHost> {
    parse_in(text, Some(variable))
}

/// Looks up an environment variable by name.
pub type Variable<'a> = dyn Fn(&str) -> Option<OsString> + 'a;

fn parse_in(text: &str, variable: Option<&Variable<'_>>) -> Vec<CliHost> {
    let mut hosts = Vec::new();
    // Each host's `custom_headers`, as written, beside `hosts`.
    let mut lists: Vec<Vec<CustomHeader>> = Vec::new();
    // Inside the last host's `custom_headers` list: its items' state.
    let mut list: Option<Items> = None;
    let mut in_hosts = false;
    // The indentation of host names and of their fields, from their first line.
    let mut host_indent = None;
    let mut field_indent = None;
    // Whether the last host name began a block whose fields belong to it.
    let mut open = false;
    for line in text.lines() {
        let content = line.trim_start_matches(' ');
        let indent = line.len() - content.len();
        let content = content.trim_end();
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        if indent == 0 {
            in_hosts = matches!(entry(content), Some((key, None)) if key == "hosts");
            list = None;
            continue;
        }
        if let (Some(items), Some(fields), Some(headers)) =
            (list.as_mut(), field_indent, lists.last_mut())
            // A list's items may sit at its key's own indentation.
            && (indent > fields || (indent == fields && content.starts_with('-')))
        {
            items.read(content, indent, headers);
            continue;
        }
        list = None;
        let entry = entry(content);
        let (true, Some((key, value))) = (in_hosts, entry) else {
            continue;
        };
        let host_level = *host_indent.get_or_insert(indent);
        if indent <= host_level {
            open = indent == host_level && value.is_none();
            if open {
                hosts.push(CliHost {
                    host: key,
                    ..CliHost::default()
                });
                lists.push(Vec::new());
            }
            continue;
        }
        let (true, Some(host)) = (open, hosts.last_mut()) else {
            continue;
        };
        if indent != *field_indent.get_or_insert(indent) {
            continue;
        }
        match key.as_str() {
            "api_host" => host.api_host = value,
            "api_protocol" => host.api_protocol = value,
            "ssh_host" => host.ssh_host = value,
            "custom_headers" if value.is_none() && variable.is_some() => {
                list = Some(Items::default());
            }
            _ => {}
        }
    }
    if let Some(variable) = variable {
        for (host, list) in hosts.iter_mut().zip(lists) {
            host.headers = Headers::resolve(list, variable);
        }
    }
    hosts
}

/// The items of a `custom_headers` list: `- name: ...` lines, each followed
/// by its other keys at the indentation of `name`.
#[derive(Default)]
struct Items {
    /// The indentation of the current item's keys.
    keys: Option<usize>,
}

impl Items {
    fn read(&mut self, content: &str, indent: usize, list: &mut Vec<CustomHeader>) {
        let (content, indent) = match content.strip_prefix('-') {
            Some(rest) if rest.is_empty() || rest.starts_with(' ') => {
                let item = rest.trim_start_matches(' ');
                list.push(CustomHeader::default());
                // `-` alone leaves the keys to the lines below it.
                let keys = indent + (content.len() - item.len());
                self.keys = (!item.is_empty()).then_some(keys);
                (item, keys)
            }
            Some(_) => return,
            None => (content, indent),
        };
        if content.is_empty() || *self.keys.get_or_insert(indent) != indent {
            return;
        }
        let (Some(header), Some((key, value))) = (list.last_mut(), entry(content)) else {
            return;
        };
        header.set(&key, value);
    }
}

/// A mapping line's key and its scalar value, if it has one. A list item or
/// a line without a `key:` is none.
fn entry(content: &str) -> Option<(String, Option<String>)> {
    if content.starts_with('-') {
        return None;
    }
    let (key, rest) = match content.chars().next()? {
        quote @ ('"' | '\'') => {
            let (key, rest) = content[1..].split_once(quote)?;
            (key, rest.strip_prefix(':')?)
        }
        // A host may carry a port, so only a colon that ends the line or is
        // followed by a space ends the key.
        _ => match content.split_once(": ") {
            Some((key, rest)) => (key, rest),
            None => (content.strip_suffix(':')?, ""),
        },
    };
    let key = key.trim();
    (!key.is_empty()).then(|| (key.to_owned(), scalar(rest)))
}

/// A plain or quoted scalar, without a trailing comment; none when empty.
fn scalar(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let value = match raw.chars().next()? {
        quote @ ('"' | '\'') => raw[1..].split_once(quote)?.0,
        '#' => return None,
        _ => raw
            .split_once(" #")
            .map_or(raw, |(value, _)| value)
            .trim_end(),
    };
    (!value.is_empty()).then(|| value.to_owned())
}

/// The headers glab sends to `instance`, from this machine's config. Reads a
/// file and may probe the shell: call it from a worker.
pub fn headers_for(instance: &str) -> Headers {
    read()
        .into_iter()
        .find(|host| host.host.eq_ignore_ascii_case(instance))
        .map(|host| host.headers)
        .unwrap_or_default()
}

/// The instance glab reaches over SSH at `remote_host` when that is not an
/// instance of its own: the one whose `ssh_host` it is.
pub fn by_ssh_host<'a>(hosts: &'a [CliHost], remote_host: &str) -> Option<&'a CliHost> {
    if hosts
        .iter()
        .any(|host| host.host.eq_ignore_ascii_case(remote_host))
    {
        return None;
    }
    let remote = without_port(remote_host);
    hosts.iter().find(|host| {
        host.ssh_host
            .as_deref()
            .is_some_and(|ssh| without_port(ssh).eq_ignore_ascii_case(remote))
    })
}

/// Whether glab knows `remote_host`, as an instance or as an instance's
/// `ssh_host`.
pub fn knows(hosts: &[CliHost], remote_host: &str) -> bool {
    hosts
        .iter()
        .any(|host| host.host.eq_ignore_ascii_case(remote_host))
        || by_ssh_host(hosts, remote_host).is_some()
}

/// `host` without a `:port`; a bracketed IPv6 address keeps its colons.
fn without_port(host: &str) -> &str {
    match host.strip_prefix('[') {
        Some(rest) => rest.find(']').map_or(host, |end| &host[..end + 2]),
        None => match host.split_once(':') {
            Some((name, port)) if !port.contains(':') => name,
            _ => host,
        },
    }
}
