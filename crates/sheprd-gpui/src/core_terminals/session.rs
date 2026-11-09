//! The session over herdr-core: the app's herdr-core tabs, gathered in the
//! workspaces the app keeps for them, with the agent each terminal reports.

use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, scope},
    session_model::Sessions,
};
use gpui::App;
use sheprd_backend::TerminalId;
use sheprd_session::{CoreTab, CoreWorkspace, Session, Worktree, panes::Panes};
use std::sync::Arc;

impl SheprdWindow {
    /// The host whose herdr-core session the window keeps: the selected one
    /// when it is this computer, else this computer's first, so its Herdr 2
    /// agents still count while another host is shown.
    pub(crate) fn core_host(&self) -> Option<usize> {
        if !self.selected_is_remote() {
            return Some(self.selected_endpoint);
        }
        self.endpoints
            .iter()
            .position(|endpoint| !endpoint.connection.target.is_remote())
    }

    /// Rebuilds the herdr-core session of this computer's host from the
    /// store's workspaces and tabs, the shell shown in place of Herdr 1, and
    /// what the core last reported. Whether it changed.
    pub(crate) fn refresh_core_session(&mut self, cx: &mut App) -> bool {
        let mut checkouts_changed = false;
        let built = self.core_host().map(|index| {
            let scope = scope(&self.endpoints[index]);
            self.keep_default_workspace(self.live_at(index).missing_installation, &scope, cx);
            let mut folders: Vec<String> =
                cx.try_global::<Store>().map_or_else(Vec::new, |store| {
                    store
                        .kept_workspaces(&scope)
                        .filter(|workspace| !workspace.cwd.is_empty())
                        .map(|workspace| workspace.cwd.clone())
                        .collect()
                });
            // The shown pane's checkout, for the Git and pull request chips.
            if let Some(folder) = self.shown_core_folder()
                && !folders.contains(&folder)
            {
                folders.push(folder);
            }
            checkouts_changed = self
                .core_terminals
                .checkouts
                .poll(folders, std::time::Instant::now());
            let session = cx
                .try_global::<Store>()
                .map(|store| self.core_session_of(index, store, &scope, cx))
                .unwrap_or_default();
            (scope, Arc::new(session))
        });
        let unchanged = match (&self.core_terminals.session, &built) {
            (Some((old_scope, old)), Some((scope, new))) => old_scope == scope && old == new,
            (None, None) => true,
            _ => false,
        };
        if !unchanged {
            let agents = built.iter().flat_map(|(_, session)| &session.agents);
            self.core_terminals
                .recency
                .observe(agents.map(|agent| (agent.pane.as_str(), agent.status)));
            self.core_terminals.session = built;
        }
        !unchanged || checkouts_changed
    }

    pub(super) fn live_at(&self, index: usize) -> &crate::state::LiveState {
        if index == self.selected_endpoint {
            &self.live
        } else {
            &self.endpoints[index].live
        }
    }

    fn core_session_of(&self, index: usize, store: &Store, scope: &Scope, cx: &App) -> Session {
        let checkouts = &self.core_terminals.checkouts;
        let workspaces = store.kept_workspaces(scope).map(|saved| {
            let mut workspace = CoreWorkspace::from(saved);
            if let Some(checkout) = checkouts.get(&saved.cwd) {
                workspace.branch = checkout.branch.as_deref();
                workspace.worktree = Some(Worktree {
                    key: checkout.key.clone(),
                    label: checkout.label.clone(),
                    linked: checkout.linked,
                });
            }
            workspace
        });
        // The shell shown in place of Herdr 1 is no tab, but runs agents too.
        let shell = self
            .live_at(index)
            .missing_installation
            .then(|| self.core_fallback_view())
            .flatten()
            .map(|view| Panes::single(view.read(cx).terminal().clone()));
        let tabs = store
            .in_scope(scope)
            .filter_map(|tab| match &tab.location {
                Some(Location::CoreTerminal {
                    panes,
                    observe: false,
                    ..
                }) => Some(CoreTab {
                    id: tab.id.number(),
                    panes,
                    title: &tab.title,
                    workspace_id: &tab.workspace_id,
                }),
                _ => None,
            })
            .chain(shell.as_ref().map(|panes| CoreTab {
                id: super::SHELL_TAB,
                panes,
                title: "",
                workspace_id: super::WORKSPACE,
            }));
        let focused = self
            .core_terminals
            .in_front
            .as_ref()
            .filter(|(front, _)| front == scope)
            .map(|(_, id)| id.as_str());
        let mut session = Session::core(workspaces, tabs, focused, |terminal| {
            self.core_terminals.reported(terminal)
        });
        // Which agent each is, and what to call it, by the policy the
        // Overview and the attention service name agents with.
        for agent in &mut session.agents {
            let Some(info) = TerminalId::new(agent.pane.as_str())
                .ok()
                .and_then(|terminal| self.core_terminals.reported(&terminal))
            else {
                continue;
            };
            agent.identity =
                sheprd_core_client::agent_kind(info).map(|kind| kind.name().to_owned());
            if let Some(name) = sheprd_core_client::agent_name(info) {
                agent.name = name;
            }
        }
        session
    }

    /// The pull request and Git status lookup of each kept workspace on the
    /// selected host that is a Git checkout on a branch, by workspace.
    pub(crate) fn kept_checkout_inputs(&self) -> Vec<(String, crate::pull_request::Input)> {
        let Some(core) = self.sessions(self.selected_endpoint).core else {
            return Vec::new();
        };
        core.workspaces
            .iter()
            .filter_map(|workspace| {
                let tree = workspace.worktree.as_ref()?;
                let input =
                    crate::pull_request::key_input(&tree.key, workspace.branch.as_deref()).ok()?;
                Some((workspace.id.clone(), input))
            })
            .collect()
    }

    /// Puts `session` in place of the herdr-core session of this computer's
    /// host, as a core reporting its agents would.
    #[cfg(test)]
    pub(crate) fn set_core_session_for_test(&mut self, session: Session) {
        let Some(index) = self.core_host() else {
            return;
        };
        let scope = scope(&self.endpoints[index]);
        self.core_terminals.session = Some((scope, Arc::new(session)));
    }

    /// This computer's Herdr 2 agents as the sidebar lists them, while the
    /// host of its herdr-core session is the one at `index`.
    pub(crate) fn listed_core_agents(
        &self,
        index: usize,
    ) -> Option<crate::sidebar::CoreAgents<'_>> {
        let (core_scope, session) = self.core_terminals.session.as_ref()?;
        if *core_scope != scope(self.endpoints.get(index)?) {
            return None;
        }
        let shown = self
            .core_terminals
            .shown
            .as_ref()
            .map(|(_, terminal)| terminal.as_str());
        let snapshot = self.live_at(index).snapshot.as_deref();
        let agents = session
            .agents
            .iter()
            .map(|agent| crate::sidebar::CoreAgent {
                agent,
                session,
                snapshot,
                info: TerminalId::new(agent.pane.as_str())
                    .ok()
                    .and_then(|terminal| self.core_terminals.reported(&terminal)),
                focused: shown == Some(agent.pane.as_str()),
            })
            .collect();
        Some(crate::sidebar::CoreAgents {
            index,
            answers: self.core_terminals.hub.backend().is_some(),
            agents,
            recency: &self.core_terminals.recency,
        })
    }

    /// The sessions of the host at `index`: Herdr 1's, and herdr-core's
    /// when the host is this computer.
    pub(crate) fn sessions(&self, index: usize) -> Sessions {
        let live = if index == self.selected_endpoint {
            &self.live
        } else {
            &self.endpoints[index].live
        };
        let core = self
            .core_terminals
            .session
            .as_ref()
            .filter(|(core_scope, _)| *core_scope == scope(&self.endpoints[index]))
            .map(|(_, session)| session.clone());
        Sessions {
            core,
            core_answers: self.core_terminals.hub.backend().is_some(),
            herdr_1: live.session(),
        }
    }
}
