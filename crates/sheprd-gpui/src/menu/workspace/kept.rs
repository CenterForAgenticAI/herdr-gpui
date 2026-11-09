//! The menu of a workspace the app keeps for Herdr 2. The app carries out
//! what it asks, with no daemon to answer, so it offers what the app can do
//! itself: renaming and closing the workspace.

use super::{Label, Page, Source, WorkspaceAction, WorkspaceMenuAction, WorkspaceTarget};
use crate::SheprdWindow;
use gpui::{prelude::*, *};
use sheprd_client::protocol::ClientShellWorktree;
use sheprd_session::Keeper;

/// The rows a kept workspace's menu shows: a note and its checkpoints too
/// once Git says which checkout and branch its folder is.
pub(super) fn items(checkout: bool, checkpoints: bool) -> Vec<(WorkspaceMenuAction, Label)> {
    let mut items = vec![
        (
            WorkspaceMenuAction::Dialog(WorkspaceAction::Rename),
            Label::RENAME,
        ),
        (
            WorkspaceMenuAction::Dialog(WorkspaceAction::Close),
            Label::CLOSE,
        ),
    ];
    if checkpoints {
        items.push((WorkspaceMenuAction::Checkpoints, Label::CHECKPOINTS));
    }
    if checkout {
        items.push((
            WorkspaceMenuAction::Dialog(WorkspaceAction::Note),
            Label::NOTE,
        ));
    }
    items
}

impl WorkspaceTarget {
    /// A target for a workspace the app keeps.
    fn kept(workspace: &sheprd_session::Workspace) -> Self {
        Self {
            boot_id: String::new(),
            id: workspace.id.clone(),
            label: workspace.label.clone(),
            worktree: workspace.worktree.as_ref().map(|tree| ClientShellWorktree {
                key: tree.key.clone(),
                label: tree.label.clone(),
                is_linked_worktree: tree.linked,
            }),
            close_members: vec![workspace.id.clone()],
            heads_group: false,
            branch: workspace.branch.clone(),
            base: None,
            source: Source::Workspace,
            keeper: Keeper::App,
            muted: false,
        }
    }
}

impl SheprdWindow {
    pub(super) fn open_kept_workspace_menu(
        &mut self,
        id: &str,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sessions = self.sessions(self.selected_endpoint);
        let Some(workspace) = sessions.core.as_ref().and_then(|core| core.workspace(id)) else {
            return;
        };
        self.hover = None;
        self.hover_menu = None;
        self.menu.reset();
        self.menu.endpoint_target = (
            self.selection_epoch,
            self.endpoints[self.selected_endpoint].generation,
        );
        self.menu.target = Some(WorkspaceTarget::kept(workspace));
        self.menu.anchor = anchor;
        self.menu.page = Some(Page::Workspace);
        self.discard_composition(cx);
        window.focus(&self.menu.focus, cx);
        cx.notify();
    }

    /// Carries out a kept workspace's dialog at once: the app is its keeper.
    pub(super) fn submit_kept_workspace_dialog(
        &mut self,
        action: WorkspaceAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.menu.target.as_ref().map(|target| target.id.clone()) else {
            return;
        };
        let text = self
            .menu
            .input
            .as_ref()
            .map_or(String::new(), |input| input.text.clone());
        match action {
            WorkspaceAction::Rename => {
                if text.trim().is_empty() {
                    self.menu.error = Some(crate::Error::EmptyWorkspaceLabel.to_string());
                    cx.notify();
                    return;
                }
                self.rename_kept_workspace(&id, &text, cx);
            }
            WorkspaceAction::Close => self.close_kept_workspace(&id, window, cx),
            _ => return,
        }
        self.dismiss_menu(window, cx);
    }
}
