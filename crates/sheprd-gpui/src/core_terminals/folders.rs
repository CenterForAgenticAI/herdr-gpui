//! The folders views take from herdr-core panes, as they take a Herdr
//! pane's: code search, the editor, and an editor tab's starting folder ask
//! for the folder the user works in, whichever backend runs its terminal.

use super::CoreTerminals;
use crate::{SheprdWindow, browser::Scope};
use gpui::App;
use std::path::PathBuf;

impl CoreTerminals {
    /// The folder a workspace's herdr-core tabs started in, as Herdr names
    /// a workspace's by its first tab's first pane: the first pane of the
    /// oldest tab that drives its terminals.
    pub(crate) fn launch_root(&self, scope: &Scope, workspace: &str) -> Option<PathBuf> {
        let (_, tab) = self
            .tabs
            .iter()
            .filter(|(_, shown)| !shown.observe)
            .filter(|(_, shown)| {
                shown
                    .workspace
                    .as_ref()
                    .is_some_and(|(known, id)| known == scope && id == workspace)
            })
            .min_by_key(|(id, _)| **id)?;
        self.infos.get(tab.panes.first())?.cwd.clone()
    }
}

impl SheprdWindow {
    /// The folder of the pane the user works in: the herdr-core pane the
    /// active group shows, else Herdr's focused pane, where its program is
    /// rather than where its shell started.
    pub(crate) fn focused_folder(&self, cx: &App) -> Option<String> {
        if let Some((_, terminal)) = self.shown_core_pane(cx) {
            return self
                .core_folder(&terminal)
                .and_then(|folder| folder.into_os_string().into_string().ok());
        }
        let snapshot = self.live.snapshot.as_deref()?;
        let id = snapshot.focused_pane_id.as_deref()?;
        let pane = snapshot.panes.iter().find(|pane| pane.pane_id == id)?;
        pane.foreground_cwd.clone().or_else(|| pane.cwd.clone())
    }

    /// The checkout of the herdr-core pane the active group showed at the
    /// last tick, once Git named its branch. `None` when no herdr-core tab
    /// is shown, or its pane is in no checkout or on a detached head.
    pub(crate) fn core_git_input(&self) -> Option<crate::pull_request::Input> {
        let (_, terminal) = self.core_terminals.shown.as_ref()?;
        let folder = self.core_folder(terminal)?;
        let branch = self.core_terminals.checkout.branch(&folder)?.to_owned();
        let checkout = folder.into_os_string().into_string().ok()?;
        Some(crate::pull_request::Input {
            checkout: Some(checkout),
            repo_key: None,
            branch,
        })
    }

    /// Whether the active group showed a herdr-core tab at the last tick.
    pub(crate) fn shows_core_tab(&self) -> bool {
        self.core_terminals.shown.is_some()
    }

    /// The folder the focused workspace's herdr-core tabs started in, for a
    /// workspace with no Herdr pane to name one.
    pub(crate) fn core_launch_root(&self) -> Option<String> {
        let (scope, workspace) = self.browser_key()?;
        self.core_terminals
            .launch_root(&scope, &workspace)?
            .into_os_string()
            .into_string()
            .ok()
    }
}
