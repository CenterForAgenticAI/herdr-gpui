//! The session over herdr-core: the app's herdr-core tabs, gathered in the
//! workspaces the app keeps for them, with the agent each terminal reports.

use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, scope},
    session_model::Sessions,
};
use gpui::App;
use sheprd_session::{CoreTab, CoreWorkspace, Session};
use std::sync::Arc;

impl SheprdWindow {
    /// Rebuilds the herdr-core session of the selected host when it is this
    /// computer, from the store's workspaces and tabs and what the core last
    /// reported.
    pub(crate) fn refresh_core_session(&mut self, cx: &mut App) {
        let built = (!self.selected_is_remote()).then(|| {
            let scope = scope(&self.endpoints[self.selected_endpoint]);
            self.keep_default_workspace(&scope, cx);
            let session = cx
                .try_global::<Store>()
                .map(|store| self.core_session_of(store, &scope))
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
    }

    fn core_session_of(&self, store: &Store, scope: &Scope) -> Session {
        let workspaces = store.kept_workspaces(scope).map(CoreWorkspace::from);
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
        let focused = self
            .core_terminals
            .in_front
            .as_ref()
            .filter(|(front, _)| front == scope)
            .map(|(_, id)| id.as_str());
        Session::core(workspaces, tabs, focused, |terminal| {
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
            core_answers: self.core_terminals.hub.backend().is_some(),
            herdr_1: live.session(),
        }
    }
}
