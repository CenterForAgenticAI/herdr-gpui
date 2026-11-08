//! Herdr 2 terminals started outside the app for one of its workspaces, as
//! a phone's new tab or chat is: herdr-core labels each with the workspace
//! (`sheprd_session::WORKSPACE_LABEL`), and the app puts it in a tab of its
//! own there, once, as if it had started it. A terminal some tab already
//! holds is left where it is.

use crate::{
    SheprdWindow,
    browser::{Location, Store, scope},
};
use gpui::Context;
use sheprd_backend::TerminalId;
use sheprd_session::panes::Panes;
use std::collections::HashSet;

impl SheprdWindow {
    /// Puts each terminal labelled for one of this computer's workspaces,
    /// and in no tab yet, in a new tab of that workspace.
    pub(super) fn adopt_labelled_terminals(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.core_host() else {
            return;
        };
        let scope = scope(&self.endpoints[index]);
        let wanted = {
            let Some(store) = cx.try_global::<Store>() else {
                return;
            };
            let held = held_terminals(store);
            let herdr = self.live_at(index).snapshot.clone();
            let known = |workspace: &str| {
                store.kept_workspace(&scope, workspace).is_some()
                    || herdr.as_ref().is_some_and(|snapshot| {
                        snapshot
                            .workspaces
                            .iter()
                            .any(|row| row.workspace_id == workspace)
                    })
            };
            self.core_terminals
                .infos
                .all()
                .filter(|info| !held.contains(&info.id))
                .filter(|info| !self.core_terminals.adopted.contains(&info.id))
                .filter_map(|info| {
                    let workspace = sheprd_session::labelled_workspace(&info.labels)?;
                    known(workspace).then(|| (info.id.clone(), workspace.to_owned()))
                })
                .collect::<Vec<_>>()
        };
        for (terminal, workspace) in wanted {
            // Once: a tab the user closes later ends it, and one moved
            // elsewhere stays there.
            self.core_terminals.adopted.insert(terminal.clone());
            let location = Location::CoreTerminal {
                panes: Panes::single(terminal),
                observe: false,
            };
            let opened = Store::update(cx, |store| {
                store.open(scope.clone(), &workspace, Some(location), None)
            });
            if opened.is_none() {
                tracing::warn!(%workspace, "No room for a tab a phone started");
            }
        }
        if self.core_terminals.adopted.len() > MAX_ADOPTED {
            self.core_terminals.adopted.clear();
        }
        self.sync_core_views(cx);
    }
}

/// How many adopted terminals are remembered; past it the list starts over,
/// and a terminal no tab holds by then may be adopted again.
const MAX_ADOPTED: usize = 1024;

/// Every terminal some tab of the app holds, mirrors included.
fn held_terminals(store: &Store) -> HashSet<TerminalId> {
    store
        .tabs()
        .filter_map(|tab| match tab.location.as_ref()? {
            Location::CoreTerminal { panes, .. } => Some(panes.terminals()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}
