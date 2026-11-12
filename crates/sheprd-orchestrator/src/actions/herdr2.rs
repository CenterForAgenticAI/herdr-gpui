//! A dispatch on Herdr 2: herdr-core keeps no workspaces or worktrees, so
//! `sheprd_remote::work` makes the checkout with Git, in the folder Herdr 1
//! would use, and types the agent into a herdr-core shell there. The run
//! records that shell's terminal, which Stop, Send, and Remove then address.

use super::{
    super::model::{Pane, Workspace},
    Attempt, Destination, DispatchRequest, place,
};
use crate::{Backend, Error, HerdrSession, Result, Run, RunState, Store, error::script};
use chrono::Utc;
use sheprd_client::shell_quote;
use sheprd_remote::{
    teleport::Host,
    work::{self, NewWorktree, Repository, Started, Worktree},
};
use std::sync::atomic::AtomicBool;

/// Makes the run's worktree from `checkout`, starts its agent there, and
/// sends it the prompt once it is ready, recording each step in `store`.
/// The worktree made is left in `attempt` for the caller to undo on failure;
/// an agent that started is recorded before its prompt so the caller can
/// confirm it ended before removing its checkout on failure.
#[allow(clippy::too_many_arguments)]
pub(super) fn launch(
    destination: &Destination,
    checkout: &str,
    request: &DispatchRequest,
    name: &str,
    run: &mut Run,
    store: &mut Store,
    attempt: &mut Attempt,
    cancelled: &AtomicBool,
) -> Result<()> {
    let host = &destination.host;
    let new = NewWorktree {
        branch: &request.branch,
        base: &destination.commit,
        label: None,
    };
    let repository = Repository::Herdr2 {
        checkout: checkout.to_owned(),
    };
    let worktree = work::create(host, &repository, &new, cancelled)?;
    attempt.created = Some((host.clone(), worktree.clone()));
    let (ssh, session) = place(&destination.target);
    run.workspace = Some(Workspace {
        backend: Backend::Herdr,
        // No Herdr workspace: the app keeps one when the run is opened.
        id: String::new(),
        host: ssh.clone(),
        path: Some(worktree.path.clone().into()),
        branch: request.branch.clone(),
    });
    run.state = RunState::Starting;
    run.updated_at = Utc::now();
    let pending = HerdrSession {
        run_id: run.id.clone(),
        target: Some(destination.target.clone()),
        host: ssh.clone(),
        session: session.clone(),
        pane: Pane::Pending,
        agent_name: name.to_owned(),
        updated_at: run.updated_at,
    };
    store.update_run(run, Some(&pending))?;
    let agent = work::Agent {
        kind: request.kind,
        name,
        model: request.model.as_deref(),
        prompt: None,
    };
    // From the launch intent onward, a lost reply can mean the agent is
    // already working. Keep its checkout and receipt; never replay a prompt.
    attempt.created = None;
    let started = work::owned_agent::start(host, &worktree, &agent, owner(run), cancelled)?;
    let Started::Herdr2 { terminal } = &started else {
        return Err(Error::Output {
            operation: "Starting the agent",
        });
    };
    let herdr_session = HerdrSession {
        run_id: run.id.clone(),
        target: Some(destination.target.clone()),
        host: ssh,
        session,
        pane: Pane::Herdr2 {
            terminal: terminal.clone(),
        },
        agent_name: name.to_owned(),
        updated_at: Utc::now(),
    };
    attempt.retained_session = Some(herdr_session.clone());
    prompt(
        host,
        &started,
        request,
        run,
        store,
        &herdr_session,
        cancelled,
    )
}

fn owner(run: &Run) -> work::owned_agent::Owner<'_> {
    work::owned_agent::Owner {
        key: "sheprd.orchestrator.run",
        value: &run.id,
    }
}

/// The terminal Stop and Send address: the run's agent, recovered through
/// its ownership label. A launch whose reply was lost left its session
/// Pending; the address found is saved so later actions use it directly.
/// Nothing is started and the prompt is never replayed.
pub(super) fn control(
    host: &Host,
    store: &mut Store,
    run: &Run,
    session: &mut HerdrSession,
    cancelled: &AtomicBool,
) -> Result<String> {
    let Started::Herdr2 { terminal } = recover(host, run, session, cancelled)? else {
        return Err(Error::NothingToOpen);
    };
    if session.pane == Pane::Pending {
        session.pane = Pane::Herdr2 {
            terminal: terminal.clone(),
        };
        session.updated_at = Utc::now();
        store.update_run(run, Some(session))?;
    }
    Ok(terminal)
}

fn recover(
    host: &Host,
    run: &Run,
    session: &HerdrSession,
    cancelled: &AtomicBool,
) -> Result<Started> {
    match inspect(host, run, session, false, cancelled) {
        Err(Error::OwnedAgent(work::owned_agent::Error::Uncertain)) => {
            recover_unlabelled(host, run, session, cancelled)
        }
        found => found,
    }
}

/// Runs saved before launches were labelled keep their unlabelled shell;
/// Stop and Send still reach it once its saved address, checkout, and
/// foreground agent check out.
fn recover_unlabelled(
    host: &Host,
    run: &Run,
    session: &HerdrSession,
    cancelled: &AtomicBool,
) -> Result<Started> {
    let Pane::Herdr2 { terminal } = &session.pane else {
        return Err(work::owned_agent::Error::Uncertain.into());
    };
    let path = run
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.path.as_ref())
        .ok_or(Error::NothingToOpen)?
        .to_string_lossy();
    let kind = sheprd_remote::teleport::AgentKind::parse(&run.agent).ok_or(Error::NothingToOpen)?;
    let tree = Worktree {
        path: path.into_owned(),
        opened: work::Opened::Herdr2,
    };
    Ok(work::owned_agent::recover_unlabelled(
        host,
        &tree,
        owner(run),
        kind,
        terminal,
        cancelled,
    )?)
}

pub(super) fn inspect(
    host: &Host,
    run: &Run,
    session: &HerdrSession,
    include_finished: bool,
    cancelled: &AtomicBool,
) -> Result<Started> {
    let workspace = run.workspace.as_ref().ok_or(Error::NothingToOpen)?;
    let path = workspace
        .path
        .as_ref()
        .ok_or(Error::NothingToOpen)?
        .to_string_lossy();
    let tree = Worktree {
        path: path.into_owned(),
        opened: work::Opened::Herdr2,
    };
    let agent = work::Agent {
        kind: sheprd_remote::teleport::AgentKind::parse(&run.agent).ok_or(Error::NothingToOpen)?,
        name: &session.agent_name,
        model: run.model.as_deref(),
        prompt: None,
    };
    let expected = match &session.pane {
        Pane::Herdr2 { terminal } => Some(terminal.as_str()),
        Pane::Pending => None,
        Pane::Herdr1 { .. } => return Err(Error::NothingToOpen),
    };
    Ok(work::owned_agent::inspect(
        host,
        &tree,
        owner(run),
        &agent,
        expected,
        include_finished,
        cancelled,
    )?)
}

fn prompt(
    host: &Host,
    started: &Started,
    request: &DispatchRequest,
    run: &mut Run,
    store: &mut Store,
    herdr_session: &HerdrSession,
    cancelled: &AtomicBool,
) -> Result<()> {
    store.update_run(run, Some(herdr_session))?;
    work::prompt(host, started, &request.prompt, cancelled)?;
    run.state = RunState::Running;
    run.updated_at = Utc::now();
    store.update_run(run, Some(herdr_session))
}

/// Interrupts the agent in `terminal` as Herdr 1's Stop does: Escape, then
/// Ctrl-C.
pub(super) fn stop(host: &Host, terminal: &str, cancelled: &AtomicBool) -> Result<()> {
    let line = format!(
        "core_cli keys --json -- {} esc ctrl+c >/dev/null\n",
        shell_quote(terminal)
    );
    host.capture(&line, cancelled)
        .map(drop)
        .map_err(script("Stopping the agent"))
}

/// Types `text` to the agent in `terminal` as a prompt, as Herdr 1's
/// `agent prompt` does, without waiting for it to finish what it is doing.
pub(super) fn send(host: &Host, terminal: &str, text: &str, cancelled: &AtomicBool) -> Result<()> {
    let line = format!(
        "core_cli send --json -- {} {} >/dev/null\n",
        shell_quote(terminal),
        shell_quote(text)
    );
    host.capture(&line, cancelled)
        .map(drop)
        .map_err(script("Sending the message"))
}

/// Ends the shell in `terminal` and the agent in it, as removing Herdr 1's
/// workspace closes its panes. One already gone is no failure; an
/// unreachable host or a kill that left it running keeps the run and its
/// checkout, so Remove can try again.
pub(super) fn end(host: &Host, terminal: &str, cancelled: &AtomicBool) -> Result<()> {
    Ok(work::owned_agent::end_verified(host, terminal, cancelled)?)
}

/// Proves no live herdr-core program works in `checkout`, as a failed
/// start may have left a shell whose address never came back.
pub(super) fn idle(host: &Host, checkout: &std::path::Path, cancelled: &AtomicBool) -> Result<()> {
    let tree = Worktree {
        path: checkout.to_string_lossy().into_owned(),
        opened: work::Opened::Herdr2,
    };
    Ok(work::owned_agent::checkout_idle(host, &tree, cancelled)?)
}
