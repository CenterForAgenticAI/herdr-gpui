//! The session over herdr-core: the app's herdr-core tabs, gathered in the
//! workspaces the app keeps for them, with the agent each terminal reports.

use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, scope},
    session_model::Sessions,
};
use gpui::App;
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
            self.core_terminals.session = built;
        }
        !unchanged || checkouts_changed
    }

    fn live_at(&self, index: usize) -> &crate::state::LiveState {
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
        Session::core(workspaces, tabs, focused, |terminal| {
            self.core_terminals.reported(terminal)
        })
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
