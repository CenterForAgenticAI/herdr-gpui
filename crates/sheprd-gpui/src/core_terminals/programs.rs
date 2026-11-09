//! Programs the app runs in tabs of the workspaces it keeps, such as a
//! worktree script, and how they end. herdr-core reports a terminal's labels
//! as soon as it starts, and its tab's view how its program ended.

use super::{Purpose, workspaces::scope_of};
use crate::SheprdWindow;
use gpui::{App, Context};
use sheprd_backend::{Exit, Spawn, TerminalId};

impl SheprdWindow {
    /// Starts `spawn` in a new tab of the kept workspace `id` on the shown
    /// host, which the tab is brought to once it opens.
    pub(crate) fn start_in_kept_workspace(
        &mut self,
        id: &str,
        spawn: Spawn,
        cx: &mut Context<Self>,
    ) {
        self.focus_kept_workspace(id, cx);
        let purpose = Purpose::Tab {
            scope: scope_of(self),
            workspace: id.to_owned(),
        };
        self.start_core_program(purpose, spawn, cx);
    }

    /// The terminal of this computer's core that `key` labels `value`.
    pub(crate) fn local_terminal_labelled(&self, key: &str, value: &str) -> Option<TerminalId> {
        self.core_terminals
            .infos
            .all()
            .find(|info| info.labels.get(key).is_some_and(|known| known == value))
            .map(|info| info.id.clone())
    }

    /// How `terminal` ended, once a tab showing it saw it end.
    pub(crate) fn core_terminal_exit(&self, terminal: &TerminalId, cx: &App) -> Option<Exit> {
        self.core_terminals
            .views
            .iter()
            .filter(|((_, shown), _)| shown == terminal)
            .find_map(|(_, view)| view.read(cx).exit())
    }
}
