//! The session over herdr-core: the app's herdr-core tabs, gathered in the
//! workspaces the app keeps for them, with the agent each terminal reports.

use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, scope},
    i18n::fl,
    session_model::Sessions,
};
use gpui::App;
use sheprd_session::{CoreTab, CoreWorkspace, Session};
use std::sync::Arc;

impl SheprdWindow {
    /// Rebuilds the herdr-core session of the selected host when it is this
    /// computer, from the store's tabs and what the core last reported.
    pub(super) fn refresh_core_session(&mut self, cx: &App) {
        // Built while the core answers: a core out of reach has no agents
        // to count, and the overview shows the host as it shows Herdr 1.
        let local = !self.selected_is_remote() && self.core_terminals.hub.backend().is_some();
        let built = local
            .then(|| cx.try_global::<Store>())
            .flatten()
            .map(|store| {
                let scope = scope(&self.endpoints[self.selected_endpoint]);
                let session = self.core_session_of(store, &scope);
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
    }

    fn core_session_of(&self, store: &Store, scope: &Scope) -> Session {
        let label = fl!("core-terminals-sidebar");
        // With no Herdr 1 installed, the app keeps the window's one
        // workspace; otherwise core tabs open in Herdr 1's workspaces.
        let workspaces = self.live.missing_installation.then_some(CoreWorkspace {
            cwd: "",
            id: super::WORKSPACE,
            label: &label,
        });
        let tabs = store.in_scope(scope).filter_map(|tab| match &tab.location {
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
        });
        let focused = self.browser_key().map(|(_, workspace)| workspace);
        Session::core(workspaces, tabs, focused.as_deref(), |terminal| {
            self.core_terminals.infos.get(terminal)
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
            herdr_1: live.session(),
        }
    }
}
