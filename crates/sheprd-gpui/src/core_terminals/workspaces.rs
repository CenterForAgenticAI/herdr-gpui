//! The workspaces the app keeps for Herdr 2 terminals: which one is in
//! front, and creating, renaming, closing, and moving them. herdr-core has
//! no workspaces, so these are the app's own, saved with its tabs
//! (`browser::Store`), and shown beside Herdr 1's on this computer.

use super::{Purpose, WORKSPACE};
use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, scope},
    i18n::fl,
    reorder::Beside,
    window::Flash,
};
use gpui::{prelude::*, *};
use sheprd_backend::Spawn;
use std::path::{Path, PathBuf};

impl SheprdWindow {
    /// The kept workspace in front of the selected host, if one is: the one
    /// the user last chose, or with no Herdr 1 installed, the first.
    pub(crate) fn kept_in_front(&self) -> Option<&str> {
        let (scope, session) = self.core_terminals.session.as_ref()?;
        if *scope != scope_of(self) {
            return None;
        }
        let chosen = self
            .core_terminals
            .in_front
            .as_ref()
            .filter(|(chosen, _)| chosen == scope)
            .and_then(|(_, id)| session.workspace(id));
        let chosen = match chosen {
            Some(workspace) => Some(workspace),
            None if self.live.missing_installation => session.workspaces.first(),
            None => None,
        };
        chosen.map(|workspace| workspace.id.as_str())
    }

    /// Whether `id` is a workspace the app keeps on the selected host.
    pub(crate) fn is_kept_workspace(&self, id: &str) -> bool {
        self.core_terminals
            .session
            .as_ref()
            .is_some_and(|(scope, session)| {
                *scope == scope_of(self) && session.workspace(id).is_some()
            })
    }

    /// Brings a kept workspace to the front; whether it is one.
    pub(crate) fn focus_kept_workspace(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if !self.is_kept_workspace(id) {
            return false;
        }
        self.core_terminals.in_front = Some((scope_of(self), id.to_owned()));
        self.core_terminals.reveal = true;
        cx.notify();
        true
    }

    /// Hands the front back to Herdr 1's focused workspace, as when one of
    /// its workspaces is chosen.
    pub(crate) fn leave_kept_workspace(&mut self) {
        self.core_terminals.in_front = None;
    }

    /// Keeps a new workspace on this computer, brings it to the front, and
    /// starts a shell in a tab of it, in `cwd` when given.
    pub(crate) fn new_kept_workspace(
        &mut self,
        label: Option<&str>,
        cwd: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.keep_new_workspace(label, cwd.as_deref(), cx) else {
            return;
        };
        let spawn = Spawn {
            cwd,
            ..Spawn::default()
        };
        self.start_core_program(
            Purpose::Tab {
                scope: scope_of(self),
                workspace: id,
            },
            spawn,
        );
    }

    /// Keeps a new workspace for a checkout at `cwd` whose first tab shows
    /// `location`, such as a herdr-core terminal a worker already started,
    /// and brings it to the front. Answers its ID, or `None` when it could
    /// not be kept, which a flash says.
    pub(crate) fn open_kept_workspace(
        &mut self,
        label: Option<&str>,
        cwd: &Path,
        location: Location,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let id = self.keep_new_workspace(label, Some(cwd), cx)?;
        let scope = scope_of(self);
        let tab = Store::update(cx, |store| store.open(scope, &id, Some(location), None));
        match tab {
            Some(tab) => {
                self.sync_core_views(cx);
                self.show_browser_tab(tab, window, cx);
            }
            None => self.show_flash(Flash::warning(fl!("core-terminals-too-many-tabs")), cx),
        }
        Some(id)
    }

    /// The kept workspace on the selected host whose folder is `cwd`.
    pub(crate) fn kept_workspace_at(&self, cwd: &Path) -> Option<String> {
        let (_, session) = self
            .core_terminals
            .session
            .as_ref()
            .filter(|(scope, _)| *scope == scope_of(self))?;
        session
            .workspaces
            .iter()
            .find(|workspace| !workspace.cwd.is_empty() && Path::new(&workspace.cwd) == cwd)
            .map(|workspace| workspace.id.clone())
    }

    /// Keeps a new workspace and brings it to the front, named for `label`,
    /// else its folder.
    fn keep_new_workspace(
        &mut self,
        label: Option<&str>,
        cwd: Option<&Path>,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if self.selected_is_remote() {
            self.show_flash(Flash::warning(fl!("core-terminals-local-only")), cx);
            return None;
        }
        let scope = scope_of(self);
        let folder = cwd.map(|cwd| cwd.display().to_string());
        let label = label
            .map(str::to_owned)
            .or_else(|| {
                cwd.and_then(|cwd| cwd.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| fl!("core-workspaces-default"));
        let kept = Store::update(cx, |store| {
            store.keep_workspace(&scope, &label, folder.as_deref().unwrap_or(""))
        });
        let Some(id) = kept else {
            self.show_flash(Flash::warning(fl!("core-workspaces-too-many")), cx);
            return None;
        };
        self.refresh_core_session(cx);
        self.focus_kept_workspace(&id, cx);
        Some(id)
    }

    /// The folder of the workspace in front, which a new workspace starts in.
    pub(crate) fn front_workspace_cwd(&self) -> Option<PathBuf> {
        let (_, front) = self.browser_key()?;
        let sessions = self.sessions(self.selected_endpoint);
        let cwd = &sessions.workspace(&front)?.cwd;
        (!cwd.is_empty()).then(|| PathBuf::from(cwd))
    }

    /// Renames a kept workspace on the selected host.
    pub(crate) fn rename_kept_workspace(&mut self, id: &str, label: &str, cx: &mut Context<Self>) {
        let scope = scope_of(self);
        Store::update(cx, |store| store.rename_kept(&scope, id, label));
        self.refresh_core_session(cx);
        cx.notify();
    }

    /// Closes a kept workspace: its tabs close and the terminals they drive
    /// end, as closing a Herdr 1 workspace ends its panes.
    pub(crate) fn close_kept_workspace(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scope = scope_of(self);
        let tabs = Store::update(cx, |store| store.forget_kept(&scope, id));
        for tab in &tabs {
            if let Some(Location::CoreTerminal {
                panes,
                observe: false,
            }) = &tab.location
            {
                for terminal in panes.terminals() {
                    self.end_core_terminal(terminal.clone());
                }
            }
        }
        self.forget_browser_tabs(|gone| tabs.iter().any(|tab| tab.id == gone));
        if self
            .core_terminals
            .in_front
            .as_ref()
            .is_some_and(|(_, front)| front == id)
        {
            self.core_terminals.in_front = None;
            self.core_terminals.reveal = true;
        }
        self.refresh_core_session(cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Moves a kept workspace beside another of the selected host's.
    pub(crate) fn move_kept_workspace(
        &mut self,
        id: &str,
        beside: Beside<&str>,
        cx: &mut Context<Self>,
    ) -> bool {
        let scope = scope_of(self);
        let moved = Store::update(cx, |store| store.move_kept(&scope, id, beside));
        if moved {
            self.refresh_core_session(cx);
            cx.notify();
        }
        moved
    }

    /// Shows a tab of the kept workspace just brought to the front, so its
    /// groups do not keep drawing Herdr 1's terminal.
    pub(super) fn reveal_kept_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.core_terminals.reveal) {
            return;
        }
        let Some((scope, workspace)) = self.browser_key() else {
            return;
        };
        let shown = self
            .active_group()
            .and_then(|group| self.group_pick(group))
            .is_some_and(|pick| matches!(pick, crate::browser::Pick::Page(_)));
        if shown || !self.is_kept_workspace(&workspace) {
            return;
        }
        let first = cx
            .try_global::<Store>()
            .and_then(|store| store.in_workspace(&scope, &workspace).next())
            .map(|tab| tab.id);
        if let Some(tab) = first {
            self.show_browser_tab(tab, window, cx);
        }
    }

    /// Keeps the workspace a window with no Herdr 1 installed gathers its
    /// tabs in, so it lists, renames, and closes like any other.
    pub(super) fn keep_default_workspace(
        &self,
        missing_installation: bool,
        scope: &Scope,
        cx: &mut App,
    ) {
        if !missing_installation
            || cx
                .try_global::<Store>()
                .is_some_and(|store| store.kept_workspaces(scope).next().is_some())
        {
            return;
        }
        let label = fl!("core-workspaces-default");
        Store::update(cx, |store| store.keep_default(scope, WORKSPACE, &label));
    }
}

fn scope_of(window: &SheprdWindow) -> Scope {
    scope(&window.endpoints[window.selected_endpoint])
}
