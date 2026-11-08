//! Moving an agent in a herdr-core terminal into a chat: the policy of
//! `sheprd_client::herdr::convert`, asked of herdr-core instead of Herdr 1.
//! The app and the remote control daemon share it.
//!
//! Only an idle agent moves, and everything that could refuse is checked
//! before it is touched. It stops on Ctrl+C (`terminal.keys`) until the shell
//! that started it is back in front, and the `sheprd-acp --resume` line is
//! then typed into that shell (`terminal.send`).
//!
//! herdr-core reports no agent session, unlike Herdr 1's integrations. Claude
//! Code records its own in `~/.claude/sessions/<pid>.json`, by the process
//! herdr-core reports in front, so Claude moves; the other agents wait for
//! herdr-core to carry a session reference (for instance in their OSC 7501
//! records).

use crate::{Core, Result, agent_kind, backend::info};
use serde::Deserialize;
use sheprd_backend::{AgentStatus, Process, TerminalId, TerminalInfo};
use sheprd_chat_protocol::{Agent, EndpointId, TOKEN, restore};
use sheprd_client::herdr::{
    AgentKind,
    convert::{self, Resume},
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// Whether the terminal's agent is one that can move at all, however busy
/// it is.
pub fn movable(info: &TerminalInfo) -> Result<Agent, convert::Error> {
    let chat = info
        .labels
        .get(TOKEN)
        .is_some_and(|value| value.parse::<EndpointId>().is_ok());
    if chat {
        return Err(convert::Error::AlreadyChat);
    }
    let kind = agent_kind(info).ok_or(convert::Error::NoAgent)?;
    let agent: Agent = kind
        .name()
        .parse()
        .map_err(|_| convert::Error::Unknown(kind.name().to_owned()))?;
    if !convert::convertible(agent) {
        return Err(convert::Error::Unsupported(agent));
    }
    Ok(agent)
}

/// Whether the terminal's agent may move now: movable, and between turns.
pub fn check(info: &TerminalInfo) -> Result<Agent, convert::Error> {
    let agent = movable(info)?;
    match info.agent {
        AgentStatus::Idle | AgentStatus::Done => Ok(agent),
        AgentStatus::Working | AgentStatus::Blocked | AgentStatus::Unknown => {
            Err(convert::Error::Busy(agent))
        }
    }
}

/// The process in front when it runs `agent`.
fn agent_process(info: &TerminalInfo, agent: Agent) -> Result<&Process, convert::Error> {
    info.foreground
        .as_ref()
        .filter(|process| runs(process, agent))
        .ok_or(convert::Error::NotRunning(agent))
}

/// Whether `process` is `agent` itself, not a shell or another program.
fn runs(process: &Process, agent: Agent) -> bool {
    AgentKind::identify(None, &process.name, &process.argv)
        .is_some_and(|kind| kind.name() == agent.name())
}

/// Whether the agent has left and the shell that started it is in front.
pub fn shell_in_front(info: &TerminalInfo, agent: Agent) -> bool {
    info.foreground
        .as_ref()
        .is_some_and(|process| !runs(process, agent))
}

/// What Claude Code writes about one running CLI.
#[derive(Deserialize)]
struct ClaudeRecord {
    pid: u32,
    #[serde(rename = "sessionId")]
    session_id: String,
}

/// Where Claude Code records the CLI with process ID `pid`.
pub fn claude_record(home: &Path, pid: u32) -> PathBuf {
    home.join(".claude")
        .join("sessions")
        .join(format!("{pid}.json"))
}

/// The session the agent in `process` runs, from what `read` finds in the
/// record the agent keeps of itself. Only Claude Code keeps one; a record
/// of another process, or a session no line can carry, is none.
pub fn session(
    agent: Agent,
    process: &Process,
    home: &Path,
    read: impl FnOnce(&Path) -> io::Result<String>,
) -> Result<String, convert::Error> {
    let none = || convert::Error::NoCoreSession(agent);
    if agent != Agent::Claude {
        return Err(none());
    }
    let text = read(&claude_record(home, process.pid)).map_err(|_| none())?;
    let record: ClaudeRecord = serde_json::from_str(&text).map_err(|_| none())?;
    (record.pid == process.pid && restore::session_id_is_plain(&record.session_id))
        .then_some(record.session_id)
        .ok_or_else(none)
}

/// What continues the conversation, read from the running agent.
pub fn resume(
    info: &TerminalInfo,
    agent: Agent,
    home: &Path,
    read: impl FnOnce(&Path) -> io::Result<String>,
) -> Result<Resume, convert::Error> {
    let process = agent_process(info, agent)?;
    let session = session(agent, process, home, read)?;
    let cwd = info
        .cwd
        .as_deref()
        .and_then(Path::to_str)
        .filter(|cwd| !cwd.is_empty())
        .ok_or(convert::Error::NotRunning(agent))?
        .to_owned();
    let flags = match agent {
        Agent::Claude => AgentKind::Claude.kept_flags(&process.argv),
        _ => Vec::new(),
    };
    Ok(Resume {
        agent,
        cwd,
        flags,
        session,
    })
}

/// What the core reports of `terminal` now.
fn current(core: &Core, terminal: &TerminalId) -> Result<TerminalInfo> {
    let listed = core.list(false)?;
    let record = listed
        .terminals
        .iter()
        .find(|record| record.address == terminal.as_str())
        .ok_or(convert::Error::Moved)?;
    info(record)
}

/// Blocking: moves the agent in `terminal`, which a client last saw as
/// `agent`, into a chat run by `program`, a path on this machine. `home`
/// holds the agent's own records. Refusals before the agent stops leave it
/// running; `cancelled` ends the move between rounds.
pub fn run(
    core: &Core,
    terminal: &TerminalId,
    agent: Agent,
    program: &Path,
    home: &Path,
    cancelled: &AtomicBool,
) -> Result<()> {
    let found = current(core, terminal)?;
    if check(&found)? != agent {
        return Err(convert::Error::Moved.into());
    }
    let resume = resume(&found, agent, home, |path| std::fs::read_to_string(path))?;
    let line = resume.local_line(program)?;
    convert::stop(
        agent,
        || current(core, terminal).map(|now| shell_in_front(&now, agent)),
        || core.keys(terminal.as_str(), &["ctrl+c"]),
        || std::thread::sleep(convert::POLL),
        cancelled,
    )?;
    core.send(terminal.as_str(), &line)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
