//! Durable restart launches shared by desktop and phone workers. A recovery
//! starts a NEW terminal; it never sends a command to the old terminal's PTY.
//! Documents belong to the connected core, so remote and named cores cannot
//! accidentally share claims. ACP also holds its host-local endpoint lock.

mod error;
mod sweep;

pub use error::Error;
pub use sweep::Sweep;

use crate::{
    Core, Result,
    wire::{Run, Terminal},
};
use serde::{Deserialize, Serialize};
use sheprd_chat_protocol::{Agent, ENDPOINT_ENV, EndpointId, TOKEN, restore};
use std::collections::BTreeMap;

pub const GENERATION_ENV: &str = "HERDR_ACP_RECOVERY_GENERATION";
pub const OVERRIDES_ENV: &str = "HERDR_ACP_RECOVERY_OVERRIDES";
pub const REPLACES_LABEL: &str = "sheprd.chat.replaces";
pub const REPLACED_BY_LABEL: &str = "sheprd.chat.replaced_by";
const MAX_BYTES: usize = 32 * 1024;
const MAX_PREVIOUS: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Record {
    adapter: bool,
    #[serde(default)]
    args: Vec<String>,
    agent: String,
    cwd: String,
    env: BTreeMap<String, String>,
    generation: u32,
    labels: BTreeMap<String, String>,
    previous: Vec<String>,
    program: String,
    root_argv: Vec<String>,
    root_pid: Option<i32>,
    session: String,
    state: State,
    terminal: String,
    version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Live,
    Launching,
    Closed,
}

/// Exact session and launch policy supplied by the ACP process on its host.
pub struct Session {
    pub adapter: bool,
    pub args: Vec<String>,
    pub agent: Agent,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub program: String,
    pub session: String,
}

fn document(endpoint: &EndpointId) -> String {
    format!("sheprd-chat-recovery-{endpoint}")
}

fn read(core: &Core, endpoint: &EndpointId) -> Result<(Option<Record>, u64)> {
    let doc = core.doc_get(&document(endpoint))?;
    if doc.value.is_null() {
        return Ok((None, doc.version));
    }
    if serde_json::to_vec(&doc.value)?.len() > MAX_BYTES {
        return Err(Error::Limit.into());
    }
    let record: Record = serde_json::from_value(doc.value)?;
    validate(&record)?;
    Ok((Some(record), doc.version))
}

fn validate(record: &Record) -> Result<()> {
    if record.version == 1 && record.state == State::Closed {
        return Ok(());
    }
    if record.version != 1
        || record.agent.parse::<Agent>().is_err()
        || !restore::session_id_is_plain(&record.session)
        || !absolute(&record.program)
        || record.program.contains('\0')
        || !absolute(&record.cwd)
        || record.cwd.contains('\0')
        || record.terminal.is_empty()
        || record.terminal.contains('/')
        || record
            .env
            .iter()
            .any(|(key, value)| key.is_empty() || key.contains(['=', '\0']) || value.contains('\0'))
    {
        return Err(Error::Invalid.into());
    }
    if record.args.iter().any(|arg| arg.contains('\0'))
        || record.previous.iter().any(|id| {
            id.is_empty() || id.len() > 128 || id.contains('/') || id.chars().any(char::is_control)
        })
    {
        return Err(Error::Invalid.into());
    }
    if record.previous.len() > MAX_PREVIOUS || record.generation as usize > MAX_PREVIOUS {
        return Err(Error::Limit.into());
    }
    Ok(())
}

// Paths belong to the core's host, not necessarily this client's OS.
fn absolute(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with("\\\\")
        || (path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && path.as_bytes().get(1) == Some(&b':')
            && path
                .as_bytes()
                .get(2)
                .is_some_and(|byte| matches!(byte, b'/' | b'\\')))
}

fn write(core: &Core, endpoint: &EndpointId, record: &Record, version: u64) -> Result<()> {
    validate(record)?;
    let value = serde_json::to_value(record)?;
    if serde_json::to_vec(&value)?.len() > MAX_BYTES {
        return Err(Error::Limit.into());
    }
    core.doc_set(&document(endpoint), &value, version)?;
    Ok(())
}

/// Capture only the explicit launch overrides, never the core's full inherited
/// environment. Core already persists these overrides for its kept terminals.
pub fn run(core: &Core, run: &Run<'_>) -> Result<Terminal> {
    if !run
        .labels
        .get(TOKEN)
        .is_some_and(|id| id.parse::<EndpointId>().is_ok())
    {
        return core.run(run);
    }
    let overrides: BTreeMap<&str, &str> = run
        .env
        .iter()
        .filter(|(key, _)| {
            ![OVERRIDES_ENV, GENERATION_ENV, ENDPOINT_ENV, "HERDR_PANE_ID"].contains(key)
        })
        .map(|(key, value)| (*key, *value))
        .collect();
    let encoded = serde_json::to_string(&overrides)?;
    if encoded.len() > MAX_BYTES / 2 {
        return Err(Error::Limit.into());
    }
    let mut run = run.clone();
    run.env.insert(GENERATION_ENV, "");
    run.env.insert(OVERRIDES_ENV, &encoded);
    core.run(&run)
}

/// Publish only an actual opened session, never an inferred recent session.
pub fn opened(core: &Core, endpoint: &EndpointId, terminal: &str, session: Session) -> Result<()> {
    let raw = core
        .list(false)?
        .terminals
        .into_iter()
        .find(|raw| raw.address == terminal)
        .ok_or(Error::Moved)?;
    let (previous, version) = read(core, endpoint)?;
    let (generation, lineage) = match previous {
        Some(record) if record.state == State::Closed => return Err(Error::Closed.into()),
        Some(record) if record.state == State::Live && record.terminal == terminal => {
            (record.generation, record.previous)
        }
        Some(_) => return Err(Error::Moved.into()),
        None => (0, Vec::new()),
    };
    let record = Record {
        adapter: session.adapter,
        args: session.args,
        agent: session.agent.to_string(),
        cwd: session.cwd,
        env: session.env,
        generation,
        labels: raw.labels,
        previous: lineage,
        program: session.program,
        root_argv: raw.argv,
        root_pid: raw.pid,
        session: session.session,
        state: State::Live,
        terminal: terminal.to_owned(),
        version: 1,
    };
    write(core, endpoint, &record, version)
}

/// Called before a replacement ACP spawns any agent, while holding its endpoint
/// lock. A closed or superseded launch must stop even if its run reply was lost.
pub fn claim(core: &Core, endpoint: &EndpointId, terminal: &str, generation: u32) -> Result<()> {
    let (record, version) = read(core, endpoint)?;
    let mut record = record.ok_or(Error::Moved)?;
    if record.state == State::Closed {
        return Err(Error::Closed.into());
    }
    if record.state != State::Launching || record.generation != generation {
        return Err(Error::Moved.into());
    }
    let raw = core
        .list(false)?
        .terminals
        .into_iter()
        .find(|raw| raw.address == terminal)
        .ok_or(Error::Moved)?;
    if raw.name.as_deref() != Some(&launch_name(endpoint, generation))
        || raw.labels.get(TOKEN).map(String::as_str) != Some(endpoint.as_str())
    {
        return Err(Error::Moved.into());
    }
    record.terminal = terminal.to_owned();
    record.root_pid = raw.pid;
    record.root_argv = raw.argv;
    record.state = State::Live;
    write(core, endpoint, &record, version)?;
    publish_mapping(core, &record)
}

/// Durable expected-close tombstone, written BEFORE terminal.kill. A late
/// launch can no longer claim the chat; workers also end a claimed replacement.
pub fn closed(core: &Core, endpoint: &EndpointId) -> Result<()> {
    let (record, version) = read(core, endpoint)?;
    let mut record = record.unwrap_or_else(|| Record {
        adapter: false,
        args: Vec::new(),
        agent: String::new(),
        cwd: String::new(),
        env: BTreeMap::new(),
        generation: 0,
        labels: BTreeMap::new(),
        previous: Vec::new(),
        program: String::new(),
        root_argv: Vec::new(),
        root_pid: None,
        session: String::new(),
        state: State::Live,
        terminal: String::new(),
        version: 1,
    });
    if record.state == State::Closed {
        return Ok(());
    }
    record.state = State::Closed;
    write(core, endpoint, &record, version)
}

pub fn close_terminal(core: &Core, terminal: &Terminal) -> Result<()> {
    if let Some(endpoint) = terminal.labels.get(TOKEN).and_then(|id| id.parse().ok()) {
        closed(core, &endpoint)?;
    }
    core.kill(&terminal.address).map(drop)
}

fn launch_name(endpoint: &EndpointId, generation: u32) -> String {
    format!("{}-r{generation:x}", &endpoint.as_str()[..24])
}

fn publish_mapping(core: &Core, record: &Record) -> Result<()> {
    if record.previous.is_empty() {
        return Ok(());
    }
    let encoded = serde_json::to_string(&record.previous)?;
    core.label(
        &record.terminal,
        &BTreeMap::from([(REPLACES_LABEL, encoded.as_str())]),
    )?;
    if let Some(previous) = record.previous.last() {
        core.label(
            previous,
            &BTreeMap::from([(REPLACED_BY_LABEL, record.terminal.as_str())]),
        )?;
    }
    Ok(())
}

fn chat_program(terminal: &Terminal) -> bool {
    terminal.argv.first().is_some_and(|program| {
        let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
        matches!(
            name,
            "sheprd-acp" | "herdr-acp" | "sheprd-acp.exe" | "herdr-acp.exe"
        )
    }) || terminal.foreground.as_ref().is_some_and(|front| {
        matches!(
            front.name.as_str(),
            "sheprd-acp" | "herdr-acp" | "sheprd-acp.exe" | "herdr-acp.exe"
        )
    })
}

/// Validated old addresses carried by a replacement's durable core labels.
pub fn replaces(labels: &BTreeMap<String, String>) -> Vec<String> {
    labels
        .get(REPLACES_LABEL)
        .filter(|text| text.len() <= MAX_BYTES)
        .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .filter(|ids| {
            ids.len() <= MAX_PREVIOUS
                && ids.iter().all(|id| {
                    !id.is_empty()
                        && id.len() <= 128
                        && !id.contains('/')
                        && !id.chars().any(char::is_control)
                })
        })
        .unwrap_or_default()
}

/// Host-local aliases, collapsed to the latest surviving replacement. A
/// conflicting claim is ignored rather than rebinding an unrelated pane.
pub fn mapping<'a>(
    terminals: impl IntoIterator<Item = (&'a str, &'a BTreeMap<String, String>)>,
) -> BTreeMap<sheprd_backend::TerminalId, sheprd_backend::TerminalId> {
    use sheprd_backend::TerminalId;
    let terminals: BTreeMap<_, _> = terminals.into_iter().take(4096).collect();
    let candidates: Vec<_> = terminals
        .iter()
        .filter_map(|(id, labels)| {
            if id.contains('/') {
                return None;
            }
            let endpoint = labels.get(TOKEN)?.parse::<EndpointId>().ok()?;
            let old = replaces(labels);
            (!old.is_empty() && !old.iter().any(|old| old == id)).then_some((*id, endpoint, old))
        })
        .collect();
    let mut found = BTreeMap::<TerminalId, Option<TerminalId>>::new();
    for (id, endpoint, previous) in &candidates {
        if candidates
            .iter()
            .any(|(_, other, old)| other == endpoint && old.iter().any(|old| old == id))
        {
            continue;
        }
        let Ok(new) = TerminalId::new(*id) else {
            continue;
        };
        for old in previous {
            if found.len() >= 4096 {
                break;
            }
            if terminals.get(old.as_str()).is_some_and(|labels| {
                labels.get(TOKEN).map(String::as_str) != Some(endpoint.as_str())
            }) {
                continue;
            }
            let Ok(old) = TerminalId::new(old.clone()) else {
                continue;
            };
            found
                .entry(old)
                .and_modify(|value| {
                    if value.as_ref() != Some(&new) {
                        *value = None;
                    }
                })
                .or_insert_with(|| Some(new.clone()));
        }
    }
    // Forward receipts on retired shells survive a replacement that ends and
    // disappears from the live listing. They prevent re-adopting the old shell
    // after an app crash or a failed replacement startup.
    for (old, labels) in &terminals {
        let Some(next) = labels.get(REPLACED_BY_LABEL) else {
            continue;
        };
        let Some(endpoint) = labels
            .get(TOKEN)
            .filter(|id| id.parse::<EndpointId>().is_ok())
        else {
            continue;
        };
        if old.contains('/') || next.contains('/') || *old == next {
            continue;
        }
        if terminals
            .get(next.as_str())
            .is_some_and(|other| other.get(TOKEN) != Some(endpoint))
        {
            continue;
        }
        if let (Ok(old), Ok(next)) = (TerminalId::new(*old), TerminalId::new(next.clone()))
            && found.len() < 4096
        {
            found.entry(old).or_insert(Some(next));
        }
    }
    let direct: BTreeMap<_, _> = found
        .into_iter()
        .filter_map(|(old, new)| new.map(|new| (old, new)))
        .collect();
    direct
        .iter()
        .filter_map(|(old, new)| {
            let mut next = new;
            let mut visited = std::collections::BTreeSet::from([old]);
            for _ in 0..=MAX_PREVIOUS {
                if !visited.insert(next) {
                    return None;
                }
                match direct.get(next) {
                    Some(further) => next = further,
                    None => return Some((old.clone(), next.clone())),
                }
            }
            None
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
