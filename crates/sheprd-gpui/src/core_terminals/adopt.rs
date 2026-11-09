//! Herdr 2 terminals started outside the app for one of its workspaces, as
//! a phone's new tab or chat is: herdr-core labels each with the workspace
//! and, for a named Herdr 1 session, the session
//! (`sheprd_session::labelled_workspace`), and the app puts it in a tab of
//! its own there, once, as if it had started it. A terminal some tab on
//! this computer already holds is left where it is.

use crate::{
    SheprdWindow,
    browser::{Location, Store, scope},
};
use gpui::Context;
use sheprd_backend::TerminalId;
use sheprd_client::ConnectTarget;
use sheprd_session::panes::Panes;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

/// How often terminals waiting for a tab are looked at again: a workspace
/// may become known, or a tab may close to make room, while the terminal
/// itself stays the same.
const EVERY: Duration = Duration::from_secs(1);
/// How many adopted terminals are remembered; past it the list starts over,
/// and a terminal no tab holds by then may be adopted again.
const MAX_ADOPTED: usize = 1024;

/// Where a labelled terminal goes.
enum Workspace {
    /// A workspace that exists, by its id.
    Known(String),
    /// A new one in this folder, as a phone's new workspace asks.
    New(String),
}

impl SheprdWindow {
    /// Puts each terminal labelled for one of this computer's workspaces,
    /// and in no tab here yet, in a new tab of that workspace. At most once
    /// a second, unless `changed` says the core's list just changed.
    pub(super) fn adopt_labelled_terminals(&mut self, changed: bool, cx: &mut Context<Self>) {
        let now = Instant::now();
        let due = self
            .core_terminals
            .adopt_checked
            .is_none_or(|checked| now.saturating_duration_since(checked) >= EVERY);
        if !changed && !due {
            return;
        }
        self.core_terminals.adopt_checked = Some(now);
        let Some(store) = cx.try_global::<Store>() else {
            return;
        };
        let held = held_terminals(store);
        let local = self.core_host().map(|index| scope(&self.endpoints[index]));
        let wanted: Vec<_> = self
            .core_terminals
            .infos
            .all()
            .filter(|info| {
                !held.contains(&info.id) && !self.core_terminals.adopted.contains(&info.id)
            })
            .filter_map(|info| {
                // A phone's new workspace: kept in its folder, once a tab
                // can open there.
                if let Some(cwd) = sheprd_session::new_workspace(&info.labels) {
                    let scope = local.clone()?;
                    return Some((info.id.clone(), scope, Workspace::New(cwd.to_owned())));
                }
                let labelled = sheprd_session::labelled_workspace(&info.labels)?;
                let index = self.endpoint_of_session(labelled.session)?;
                let scope = scope(&self.endpoints[index]);
                let known = store.kept_workspace(&scope, labelled.id).is_some()
                    || self
                        .live_at(index)
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| {
                            snapshot
                                .workspaces
                                .iter()
                                .any(|row| row.workspace_id == labelled.id)
                        });
                known.then(|| {
                    let workspace = Workspace::Known(labelled.id.to_owned());
                    (info.id.clone(), scope, workspace)
                })
            })
            .collect();
        let mut opened = false;
        for (terminal, scope, workspace) in wanted {
            let Some(workspace) = self.workspace_for(&scope, workspace, cx) else {
                // Tried again once a workspace can be kept.
                continue;
            };
            let location = Location::CoreTerminal {
                panes: Panes::single(terminal.clone()),
                observe: false,
            };
            let tab = Store::update(cx, |store| {
                store.open(scope, &workspace, Some(location), None)
            });
            if tab.is_none() {
                // Tried again once a tab closes to make room.
                tracing::warn!(%workspace, "No room for a tab a phone started");
                continue;
            }
            opened = true;
            // Once: a tab the user closes later ends it, and one moved
            // elsewhere stays there.
            if self.core_terminals.adopted.len() >= MAX_ADOPTED {
                self.core_terminals.adopted.clear();
            }
            self.core_terminals.adopted.insert(terminal);
        }
        if opened {
            self.sync_core_views(cx);
        }
    }

    /// The id of the workspace a terminal goes in: the one named, or the one
    /// kept in its folder, kept now when there is none; `None` when the app
    /// keeps the most it does.
    fn workspace_for(
        &mut self,
        scope: &crate::browser::Scope,
        workspace: Workspace,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let cwd = match workspace {
            Workspace::Known(id) => return Some(id),
            Workspace::New(cwd) => cwd,
        };
        // In `scope`, not the shown host's: the desktop may show another.
        let kept = cx
            .global::<Store>()
            .kept_workspaces(scope)
            .find(|kept| {
                !kept.cwd.is_empty()
                    && std::path::Path::new(&kept.cwd) == std::path::Path::new(&cwd)
            })
            .map(|kept| kept.id.clone());
        if kept.is_some() {
            return kept;
        }
        let label = std::path::Path::new(&cwd)
            .file_name()
            .map_or_else(|| cwd.clone(), |name| name.to_string_lossy().into_owned());
        let id = Store::update(cx, |store| store.keep_workspace(scope, &label, &cwd));
        if id.is_some() {
            self.refresh_core_session(cx);
        }
        id
    }

    /// This computer's endpoint for the Herdr 1 session `name`, or for the
    /// window's own session when `None`.
    fn endpoint_of_session(&self, name: Option<&str>) -> Option<usize> {
        let Some(name) = name else {
            return self.core_host();
        };
        self.endpoints.iter().position(|endpoint| {
            matches!(&endpoint.connection.target,
                ConnectTarget::Session { name: session, .. } if session == name)
        })
    }
}

/// Every terminal some tab on this computer holds, mirrors included. A tab
/// of another host holds that host's terminals, whose IDs mean others here.
fn held_terminals(store: &Store) -> HashSet<TerminalId> {
    store
        .tabs()
        .filter(|tab| tab.scope.is_local())
        .filter_map(|tab| match tab.location.as_ref()? {
            Location::CoreTerminal { panes, .. } => Some(panes.terminals()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}
