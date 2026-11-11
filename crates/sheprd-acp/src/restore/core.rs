//! Explicit recovery in the shell core kept for this chat. Never types a
//! command into another process, and never searches for a likely session.

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};
use sheprd_chat_protocol::{ENDPOINT_ENV, EndpointId, restore};

use crate::cli::{AgentChoice, Options};

/// Held for the entire lifetime of a core chat, including agent startup.
/// Unlike a timed claim, an OS lock cannot expire while a slow starter is
/// still running. The lock file is never unlinked (which would split owners).
pub(crate) fn lock() -> anyhow::Result<Option<File>> {
    if crate::herdr::Target::from_env().is_some() || std::env::var_os("HC_TERMINAL_ID").is_none() {
        return Ok(None);
    }
    let Some(endpoint) = std::env::var_os(ENDPOINT_ENV) else {
        return Ok(None);
    };
    let endpoint: EndpointId = endpoint
        .to_str()
        .context("chat endpoint is not UTF-8")?
        .parse()
        .context("invalid chat endpoint")?;
    let dir = restore::restore_dir()
        .context("no chat state directory")?
        .join("core");
    lock_in(&dir, &endpoint).map(Some)
}

fn lock_in(dir: &Path, endpoint: &EndpointId) -> anyhow::Result<File> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(dir.join(format!("{endpoint}.lock")))?;
    file.try_lock().context("this chat already has an owner")?;
    Ok(file)
}

pub(crate) fn run(endpoint: EndpointId) -> anyhow::Result<()> {
    let _owner = lock()?.context("restore must run inside the chat's kept core terminal")?;
    let dir = restore::restore_dir()
        .context("no chat state directory")?
        .join("core");
    let record = restore::Record::read(&restore::record_path(&dir, &endpoint))?;
    let cwd = std::env::current_dir()?.canonicalize()?;
    ensure!(
        matches_context(&record, &endpoint, &cwd, |name| std::env::var_os(name)),
        "this is not the terminal, endpoint, socket and folder that kept the chat"
    );
    // The endpoint lock proves the old chat program gone, not the agent it
    // started: that runs in a process group of its own, as may its tools.
    ensure!(
        previous_agent_gone(&dir, &endpoint)?,
        "the chat's previous agent, or a tool it started, is still running; end it before restoring"
    );
    crate::app::run_owned(Options {
        adapter: record.adapter,
        agent: AgentChoice::Builtin {
            agent: record.agent,
            args: Vec::new(),
        },
        attach: true,
        cwd: Some(record.cwd),
        headless: false,
        load: Some(record.session),
        name: None,
        require_load: true,
    })
}

/// The agent a core chat started, kept beside its restore record so an
/// explicit restore can refuse while it, or a tool in its group, lives on.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct Started {
    pid: u32,
    /// The OS birth token, so a reused PID is never mistaken for it.
    started: String,
}

fn started_path(dir: &Path, endpoint: &EndpointId) -> PathBuf {
    dir.join(format!("{endpoint}.agent"))
}

/// Records the agent `pid` a core chat just started, readable only by this
/// user. Without it a later restore refuses, so a failure is only reported.
pub(crate) fn agent_started(dir: &Path, endpoint: &EndpointId, pid: u32) -> anyhow::Result<()> {
    let started = Started {
        pid,
        started: sheprd_core_client::sessions::process_birth(pid)?
            .context("the agent ended as it started")?,
    };
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    serde_json::to_writer(options.open(started_path(dir, endpoint))?, &started)?;
    Ok(())
}

/// Forgets the agent once its chat ended on its own.
pub(crate) fn agent_ended(dir: &Path, endpoint: &EndpointId) {
    let _ = std::fs::remove_file(started_path(dir, endpoint));
}

/// Whether the agent the chat last started is provably gone. An unknown
/// agent is not: doubt refuses rather than risk two agents on one session.
fn previous_agent_gone(dir: &Path, endpoint: &EndpointId) -> anyhow::Result<bool> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    File::open(started_path(dir, endpoint))
        .context("the chat's previous agent is unknown")?
        .take(4096)
        .read_to_end(&mut bytes)?;
    let started: Started = serde_json::from_slice(&bytes)?;
    Ok(agent_gone(started.pid, &started.started)?)
}

/// Proof, without signalling anything, that the agent `pid` born at
/// `started` and its process group are gone. Nothing here kills or adopts a
/// process from a stored PID.
pub(crate) fn agent_gone(pid: u32, started: &str) -> sheprd_core_client::Result<bool> {
    if sheprd_core_client::sessions::process_birth(pid)?.as_deref() == Some(started) {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        // spawn() made the agent the leader of its own group. Probe signal
        // zero only: an exited leader may still have live tools in that group.
        let Some(group) = i32::try_from(pid)
            .ok()
            .and_then(rustix::process::Pid::from_raw)
        else {
            return Ok(false);
        };
        match rustix::process::test_kill_process_group(group) {
            Err(rustix::io::Errno::SRCH) => Ok(true),
            Ok(()) | Err(rustix::io::Errno::PERM) => Ok(false),
            Err(error) => Err(std::io::Error::from(error).into()),
        }
    }
    // Windows has no process group to probe, and stop() there ends only the
    // agent itself, so a tool it started may outlive it. The exact agent is
    // gone; anything it left is the same gap a normal Windows close has.
    #[cfg(not(unix))]
    {
        Ok(true)
    }
}

fn matches_context(
    record: &restore::Record,
    endpoint: &EndpointId,
    cwd: &Path,
    var: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> bool {
    record.endpoint == *endpoint
        && record.cwd == cwd
        && var(ENDPOINT_ENV).as_deref() == Some(std::ffi::OsStr::new(endpoint.as_str()))
        && var("HC_TERMINAL_ID").as_deref() == Some(std::ffi::OsStr::new(&record.pane))
        && var("HC_TERMINAL_SOCKET").as_deref() == Some(record.socket.as_os_str())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
