//! Explicit recovery in the shell core kept for this chat. Never types a
//! command into another process, and never searches for a likely session.

use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use anyhow::{Context as _, ensure};
use sheprd_chat_protocol::{ENDPOINT_ENV, EndpointId, restore};

use crate::cli::{AgentChoice, Options};

use sheprd_core_client::{Core, Endpoint, chat_recovery};

fn connect(socket: std::path::PathBuf) -> sheprd_core_client::Result<Core> {
    Core::connect(&Endpoint {
        socket,
        session: "default".to_owned(),
    })
}

/// A replacement started by a desktop/phone worker must claim its durable
/// generation before any agent starts. Inherited caller HC_SOCKET is ignored.
pub(crate) fn claim_launch() -> anyhow::Result<()> {
    if crate::herdr::Target::from_env().is_some() {
        return Ok(());
    }
    let Some(generation) = std::env::var_os(chat_recovery::GENERATION_ENV) else {
        return Ok(());
    };
    if generation.is_empty() {
        return Ok(());
    }
    let generation = generation
        .to_str()
        .context("invalid recovery generation")?
        .parse::<u32>()?;
    let terminal = std::env::var("HC_TERMINAL_ID")?;
    let socket = std::env::var_os("HC_TERMINAL_SOCKET").context("no terminal socket")?;
    let endpoint = std::env::var(ENDPOINT_ENV)?.parse::<EndpointId>()?;
    chat_recovery::claim(&connect(socket.into())?, &endpoint, &terminal, generation)?;
    Ok(())
}

pub(super) fn opened(keeper: &super::Keeper, session: &str) -> anyhow::Result<()> {
    let core = connect(keeper.socket.clone())?;
    let mut env: std::collections::BTreeMap<String, String> =
        match std::env::var(chat_recovery::OVERRIDES_ENV) {
            Ok(encoded) if encoded.len() <= 16 * 1024 => serde_json::from_str(&encoded)?,
            Ok(_) => anyhow::bail!("chat environment overrides exceed the recovery bound"),
            Err(std::env::VarError::NotPresent) => std::collections::BTreeMap::new(),
            Err(error) => return Err(error.into()),
        };
    if let Some(agent) = sheprd_client::herdr::AgentKind::parse(keeper.agent.name()) {
        for key in sheprd_client::herdr::sessions::environment_keys(agent) {
            if let Ok(value) = std::env::var(key) {
                env.insert((*key).to_owned(), value);
            }
        }
    }
    chat_recovery::opened(
        &core,
        &keeper.endpoint,
        &keeper.pane,
        chat_recovery::Session {
            adapter: keeper.adapter,
            args: keeper.args.clone(),
            agent: keeper.agent,
            cwd: keeper
                .cwd
                .to_str()
                .context("chat cwd is not UTF-8")?
                .to_owned(),
            env,
            program: std::env::current_exe()?
                .to_str()
                .context("chat program path is not UTF-8")?
                .to_owned(),
            session: session.to_owned(),
        },
    )?;
    Ok(())
}

pub(super) fn closed(keeper: &super::Keeper) -> anyhow::Result<()> {
    chat_recovery::closed(&connect(keeper.socket.clone())?, &keeper.endpoint)?;
    Ok(())
}

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
