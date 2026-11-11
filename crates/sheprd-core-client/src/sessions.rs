//! Owning-host session reporters. No Herdr 1 installation or GUI is involved.
//! Records live in the core's versioned document store and are scoped to the
//! terminal's live foreground process, including its OS birth token.

mod error;
mod process;

use crate::{Core, Result, wire::Terminal};
pub use error::Error;
use sheprd_client::herdr::{
    AgentKind,
    sessions::{self, Binding, Identity},
    snapshot::SessionKind,
};
use std::collections::BTreeMap;

/// Legacy Claude PID files need a birth fence too. A PID alone is reusable.
pub(crate) fn claude_record(pid: u32, home: &std::path::Path) -> Result<String> {
    use std::io::Read;
    #[derive(serde::Deserialize)]
    struct Record {
        pid: u32,
        #[serde(rename = "sessionId")]
        session: String,
        #[serde(rename = "procStart")]
        started: String,
    }
    let before = process::read(pid)?;
    // The legacy file's birth stamp uses ps's calendar representation, even
    // on Linux where the binding protocol uses exact kernel start ticks.
    let mut stamp = Vec::new();
    sheprd_client::run_script(
        sheprd_client::ScriptHost::Local,
        &format!("LC_ALL=C TZ=UTC ps -o lstart= -p {pid}"),
        &[][..],
        &mut stamp,
        sheprd_client::ScriptLimits {
            output: 1024,
            idle: std::time::Duration::from_secs(2),
        },
        &std::sync::atomic::AtomicBool::new(false),
    )
    .map_err(Error::Probe)?;
    let mut bytes = Vec::new();
    std::fs::File::open(crate::convert::claude_record(home, pid))?
        .take(sessions::MAX_RECORD as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > sessions::MAX_RECORD {
        return Err(Error::Evidence(sessions::Error::TooLarge).into());
    }
    let record: Record = serde_json::from_slice(&bytes)?;
    let normalize = |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
    if record.pid != pid
        || normalize(&record.started) != normalize(&String::from_utf8_lossy(&stamp))
        || before.started != process::read(pid)?.started
        || !sheprd_chat_protocol::restore::session_id_is_plain(&record.session)
    {
        return Err(Error::Identity.into());
    }
    Ok(record.session)
}

fn record(core: &Core, terminal: &str, agent: AgentKind) -> Result<Terminal> {
    if terminal.contains('/') || terminal.is_empty() {
        return Err(Error::Identity.into());
    }
    let record = core
        .list(false)?
        .terminals
        .into_iter()
        .find(|record| record.address == terminal)
        .ok_or(Error::Identity)?;
    let foreground = record.foreground.as_ref().ok_or(Error::Identity)?;
    if AgentKind::identify(None, &foreground.name, &foreground.argv) != Some(agent) {
        return Err(Error::Identity.into());
    }
    Ok(record)
}

fn identity(core: &Core, record: &Terminal) -> Result<Identity> {
    let pid = record
        .foreground
        .as_ref()
        .and_then(|front| u32::try_from(front.pid).ok())
        .filter(|pid| *pid > 0)
        .ok_or(Error::Identity)?;
    Ok(Identity {
        core: core.hello().core.id.clone(),
        terminal: record.address.clone(),
        pid,
        started: process::read(pid)?.started,
    })
}

fn document(terminal: &str) -> String {
    format!("sheprd-agent-session-{terminal}")
}

/// Blocking, owning host only. A helper on a remote host calls this there,
/// never against a locally numbered process from another host's terminal.
pub fn discover(core: &Core, terminal: &str, agent: AgentKind) -> Result<Binding> {
    let before = record(core, terminal, agent)?;
    let value = core.doc_get(&document(terminal))?.value;
    if value.is_null() {
        return Err(Error::Absent.into());
    }
    let expected = identity(core, &before)?;
    let bytes = serde_json::to_vec(&value)?;
    let binding = Binding::parse(&bytes, &expected, agent).map_err(Error::Evidence)?;
    let after = record(core, terminal, agent)?;
    if before.foreground != after.foreground || expected != identity(core, &after)? {
        return Err(Error::Identity.into());
    }
    if binding.selected().is_none() {
        return Err(Error::Cleared.into());
    }
    Ok(binding)
}

/// Blocking, owning host only. Called by the separately installed session hook
/// or TUI plugin. Inherited HC_TERMINAL_ID only routes the report: the reporter
/// must also descend from the current foreground agent process.
pub fn report(core: &Core, terminal: &str, agent: AgentKind, input: &[u8]) -> Result<()> {
    if input.len() > sessions::MAX_RECORD {
        return Err(Error::Evidence(sessions::Error::TooLarge).into());
    }
    let payload: serde_json::Value = serde_json::from_slice(input)?;
    let mut selection = selection(agent, &payload)?;
    let before = record(core, terminal, agent)?;
    let expected = identity(core, &before)?;
    if let Some(pid) = payload.get("pid").and_then(serde_json::Value::as_u64)
        && pid != u64::from(expected.pid)
    {
        return Err(Error::Identity.into());
    }
    if !process::descends(std::process::id(), expected.pid)? {
        return Err(Error::Identity.into());
    }
    let name = document(terminal);
    let previous = core.doc_get(&name)?;
    let previous_binding = serde_json::from_value::<Binding>(previous.value).ok();
    let sequence = payload
        .get("sequence")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if agent == AgentKind::Opencode
        && (sequence == 0
            || previous_binding
                .as_ref()
                .is_some_and(|old| old.identity == expected && old.sequence >= sequence))
    {
        return Err(Error::Sequence.into());
    }
    // SessionStart hooks have no monotonic selection revision. Conflicting
    // events in one process cannot be ordered safely: permanently clear that
    // process's evidence, rather than letting a delayed hook restore an old ID.
    if agent != AgentKind::Opencode
        && previous_binding.as_ref().is_some_and(|old| {
            old.identity == expected && (old.session.is_none() || old.session != selection)
        })
    {
        selection = None;
    }
    let environment = sessions::environment_keys(agent)
        .iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.is_empty())
                .map(|value| ((*key).to_owned(), value))
        })
        .collect::<BTreeMap<_, _>>();
    let binding = Binding {
        version: 1,
        sequence,
        identity: expected.clone(),
        agent: agent.name().to_owned(),
        session: selection,
        environment,
    };
    binding
        .validate(&expected, agent)
        .map_err(Error::Evidence)?;
    let after = record(core, terminal, agent)?;
    if before.foreground != after.foreground || expected != identity(core, &after)? {
        return Err(Error::Identity.into());
    }
    // CAS refuses a delayed report rather than overwriting a newer selection.
    core.doc_set(&name, &serde_json::to_value(binding)?, previous.version)?;
    Ok(())
}

fn selection(
    agent: AgentKind,
    payload: &serde_json::Value,
) -> Result<Option<sheprd_client::herdr::snapshot::AgentSession>> {
    let id = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"));
    if agent == AgentKind::Opencode {
        if id == Some(&serde_json::Value::Null) {
            return Ok(None);
        }
    } else {
        let event = payload
            .get("hook_event_name")
            .or_else(|| payload.get("hookEventName"))
            .and_then(serde_json::Value::as_str)
            .ok_or(Error::Event)?;
        let event = event.replace(['_', '-'], "").to_ascii_lowercase();
        if event != "sessionstart"
            || !matches!(
                agent,
                AgentKind::Claude | AgentKind::Codex | AgentKind::Copilot
            )
        {
            return Err(Error::Event.into());
        }
    }
    let id = id.and_then(serde_json::Value::as_str).ok_or(Error::Event)?;
    if agent == AgentKind::Codex
        && (payload
            .get("transcript_path")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
            || std::env::var("CODEX_THREAD_ID").is_ok_and(|inherited| inherited != id))
    {
        return Err(Error::Event.into());
    }
    if agent == AgentKind::Claude && payload.get("agent_id").is_some() {
        return Err(Error::Event.into());
    }
    sessions::reference(agent, SessionKind::Id, id.to_owned())
        .map(Some)
        .map_err(|error| Error::Evidence(error).into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
