//! The app's [`Session`] of each host: Herdr 1's, read from the daemon's
//! snapshot, and herdr-core's, which the window keeps from its tabs and
//! what the core reports. Readers that move onto it work on both backends
//! (`docs/session-model.md`).

use sheprd_client::protocol::{ClientShellSnapshot, ClientShellWorkspace};
use sheprd_session::{Agent, Keeper, Session, Tab, Workspace, Worktree};
use std::{cell::RefCell, sync::Arc};

/// The session a Herdr 1 snapshot describes.
pub(crate) fn of_snapshot(snapshot: &ClientShellSnapshot) -> Session {
    Session {
        agents: snapshot
            .agents
            .iter()
            .map(|agent| Agent {
                focused: agent.focused,
                identity: agent.agent.clone(),
                name: crate::sidebar::agent_name(agent).to_owned(),
                pane: agent.pane_id.clone(),
                status: agent.agent_status,
                tab: agent.tab_id.clone(),
                workspace: agent.workspace_id.clone(),
            })
            .collect(),
        focused: snapshot.focused_workspace_id.clone(),
        workspaces: snapshot
            .workspaces
            .iter()
            .map(|workspace| of_workspace(snapshot, workspace))
            .collect(),
    }
}

fn of_workspace(snapshot: &ClientShellSnapshot, workspace: &ClientShellWorkspace) -> Workspace {
    Workspace {
        branch: workspace.branch.clone(),
        cwd: workspace.new_workspace_cwd.clone(),
        custom_label: workspace.custom_label,
        id: workspace.workspace_id.clone(),
        keeper: Keeper::Herdr1,
        label: workspace.label.clone(),
        status: workspace.agent_status,
        tabs: snapshot
            .tabs
            .iter()
            .filter(|tab| tab.workspace_id == workspace.workspace_id)
            .map(|tab| Tab {
                focused: tab.focused,
                id: tab.tab_id.clone(),
                label: tab.label.clone(),
                status: tab.agent_status,
            })
            .collect(),
        worktree: workspace.worktree.as_ref().map(|worktree| Worktree {
            key: worktree.key.clone(),
            label: worktree.label.clone(),
            linked: worktree.is_linked_worktree,
        }),
    }
}

/// A snapshot's session, built once for each snapshot that lands rather
/// than on every read. Keyed by the snapshot itself, so a snapshot swapped
/// in by any path, a test's included, is never read through a stale one.
#[derive(Clone, Debug, Default)]
pub(crate) struct SnapshotSession(RefCell<Option<(Arc<ClientShellSnapshot>, Arc<Session>)>>);

impl SnapshotSession {
    pub(crate) fn get(&self, snapshot: &Arc<ClientShellSnapshot>) -> Arc<Session> {
        let mut cached = self.0.borrow_mut();
        if let Some((built, session)) = cached.as_ref()
            && Arc::ptr_eq(built, snapshot)
        {
            return session.clone();
        }
        let session = Arc::new(of_snapshot(snapshot));
        *cached = Some((snapshot.clone(), session.clone()));
        session
    }
}

/// One host's sessions together: Herdr 1's, and on this computer the one
/// over herdr-core. Readers walk both without merging them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sessions {
    pub(crate) core: Option<Arc<Session>>,
    /// Whether the core behind `core` answers, which a host with no Herdr 1
    /// installed is online by.
    pub(crate) core_answers: bool,
    pub(crate) herdr_1: Option<Arc<Session>>,
}

impl Sessions {
    fn each(&self) -> impl Iterator<Item = &Session> {
        self.herdr_1.iter().chain(&self.core).map(Arc::as_ref)
    }

    /// Each session with who keeps its workspaces.
    pub(crate) fn keyed(&self) -> impl Iterator<Item = (Keeper, &Session)> {
        let herdr_1 = self
            .herdr_1
            .as_deref()
            .map(|session| (Keeper::Herdr1, session));
        let core = self.core.as_deref().map(|session| (Keeper::App, session));
        herdr_1.into_iter().chain(core)
    }

    pub(crate) fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.each().flat_map(|session| &session.agents)
    }

    pub(crate) fn workspaces(&self) -> impl Iterator<Item = &Workspace> {
        self.each().flat_map(|session| &session.workspaces)
    }

    pub(crate) fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces().find(|workspace| workspace.id == id)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.herdr_1.is_none() && self.core.is_none()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
