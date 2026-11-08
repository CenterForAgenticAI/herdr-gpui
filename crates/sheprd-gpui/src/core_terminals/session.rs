//! The session over herdr-core: the app's herdr-core tabs, gathered in the
//! workspaces the app keeps for them, with the agent each terminal reports.

use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, scope},
    i18n::fl,
    session_model::Sessions,
};
use gpui::App;
use sheprd_session::{CoreTab, CoreWorkspace, Session, panes::Panes};
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
    /// store's tabs, the shell shown in place of Herdr 1, and what the core
    /// last reported.
    pub(super) fn refresh_core_session(&mut self, cx: &App) {
        let built = self.core_host().and_then(|index| {
            let store = cx.try_global::<Store>()?;
            let scope = scope(&self.endpoints[index]);
            let session = self.core_session_of(index, store, &scope, cx);
            Some((scope, Arc::new(session)))
        });
        let unchanged = match (&self.core_terminals.session, &built) {
            (Some((old_scope, old)), Some((scope, new))) => old_scope == scope && old == new,
            (None, None) => true,
            _ => false,
        };
        if !unchanged {
            self.core_terminals.session = built;
        }
    }

    fn core_session_of(&self, index: usize, store: &Store, scope: &Scope, cx: &App) -> Session {
        let live = if index == self.selected_endpoint {
            &self.live
        } else {
            &self.endpoints[index].live
        };
        let label = fl!("core-terminals-sidebar");
        // With no Herdr 1 installed, the app keeps the window's one
        // workspace; otherwise core tabs open in Herdr 1's workspaces.
        let workspaces = live.missing_installation.then_some(CoreWorkspace {
            cwd: "",
            id: super::WORKSPACE,
            label: &label,
        });
        // The shell shown in place of Herdr 1 is no tab, but runs agents too.
        let shell = live
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
        let focused = (index == self.selected_endpoint)
            .then(|| self.browser_key())
            .flatten()
            .map(|(_, workspace)| workspace);
        Session::core(workspaces, tabs, focused.as_deref(), |terminal| {
            self.core_terminals.reported(terminal)
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
