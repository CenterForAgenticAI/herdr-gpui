//! Worktree scripts of a workspace the app keeps for Herdr 2. herdr-core has
//! no tab to type into first, so a trusted script starts as the program of
//! a new herdr-core tab of the kept workspace, with the same variables
//! Herdr 1's tab gets (see [`super::launch`]). Herdr 1's archive line removes
//! its checkout through `herdr`; here the tab's program is the archive
//! script alone, and the app removes the checkout through
//! `sheprd_remote::work` once that program ends successfully.

use super::{Launch, ScriptKind, launch};
use crate::{
    SheprdWindow,
    i18n::{Localize, fl},
};
use gpui::Context;
use sheprd_backend::{Spawn, TerminalId};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

/// Labels the herdr-core terminal an archive script runs in with its kept
/// workspace, so the app finds it as soon as herdr-core reports it.
const ARCHIVE_LABEL: &str = "sheprd.worktree_archive";
/// How long a new checkout's setup waits for its kept workspace, which an
/// agent landing opens only once its agent started.
const SETUP_WAIT: Duration = Duration::from_secs(120);
/// How long an archive script's terminal may take to be reported started.
const ARCHIVE_START_WAIT: Duration = Duration::from_secs(60);

/// What the window follows of kept workspaces' scripts.
#[derive(Debug, Default)]
pub(crate) struct Kept {
    archive: Option<Archive>,
    setup: Option<(Launch, Instant)>,
}

/// An archive script running in a kept workspace's tab; its checkout is
/// removed once the script succeeds, and stays when it fails.
#[derive(Debug)]
struct Archive {
    workspace: String,
    path: String,
    force: bool,
    started: Instant,
    terminal: Option<TerminalId>,
}

impl Launch {
    /// Whether the workspace the script runs for is one the app keeps.
    pub(crate) fn kept(&self) -> bool {
        crate::browser::is_kept_id(&self.workspace)
    }
}

impl SheprdWindow {
    /// Starts a trusted script as the program of a new tab of its kept
    /// workspace. An archive is followed until it ends.
    pub(super) fn open_kept_script_tab(
        &mut self,
        launch: &Launch,
        script: &str,
        cx: &mut Context<Self>,
    ) -> crate::Result<()> {
        let checkout = launch.checkout.as_ref().ok_or(super::Error::Checkout)?;
        let mut labels = BTreeMap::new();
        if launch.kind == ScriptKind::Archive {
            labels.insert(ARCHIVE_LABEL.to_owned(), launch.workspace.clone());
        }
        let spawn = Spawn {
            argv: launch::core_argv(launch.kind),
            cwd: Some(checkout.path.clone().into()),
            env: launch::env(checkout, script),
            labels,
            name: Some(launch.kind.name().to_owned()),
        };
        self.start_in_kept_workspace(&launch.workspace, spawn, cx);
        if launch.kind == ScriptKind::Archive {
            self.kept_scripts.archive = Some(Archive {
                workspace: launch.workspace.clone(),
                path: checkout.path.clone(),
                force: launch.force,
                started: Instant::now(),
                terminal: None,
            });
        }
        Ok(())
    }

    /// Runs `launch`, a new checkout's setup, once the kept workspace for
    /// its folder has opened.
    pub(crate) fn queue_kept_setup(&mut self, launch: Launch) {
        self.kept_scripts.setup = Some((launch, Instant::now()));
    }

    /// Starts a queued setup once its workspace is kept, and removes an
    /// archived checkout once its script ended successfully.
    pub(crate) fn poll_kept_scripts(&mut self, cx: &mut Context<Self>) {
        self.poll_kept_setup(cx);
        self.poll_kept_archive(cx);
    }

    fn poll_kept_setup(&mut self, cx: &mut Context<Self>) {
        let Some((launch, queued)) = &self.kept_scripts.setup else {
            return;
        };
        let workspace = launch
            .checkout
            .as_ref()
            .and_then(|checkout| self.kept_workspace_at(Path::new(&checkout.path)));
        let Some(workspace) = workspace else {
            // The agent it lands with did not start, or the host changed.
            if queued.elapsed() > SETUP_WAIT {
                self.kept_scripts.setup = None;
            }
            return;
        };
        // Another script holds the one slot; this one waits its turn.
        if self.script_slot_busy() {
            return;
        }
        let Some((mut launch, _)) = self.kept_scripts.setup.take() else {
            return;
        };
        launch.workspace = workspace;
        if let Err(error) = self.start_worktree_script(launch, cx) {
            self.local_error = Some(fl!(
                "workspace-dialog-setup-failed",
                error = error.localize()
            ));
            cx.notify();
        }
    }

    fn poll_kept_archive(&mut self, cx: &mut Context<Self>) {
        let Some(archive) = &mut self.kept_scripts.archive else {
            return;
        };
        if archive.terminal.is_none() {
            let workspace = archive.workspace.clone();
            let found = self.local_terminal_labelled(ARCHIVE_LABEL, &workspace);
            let Some(archive) = &mut self.kept_scripts.archive else {
                return;
            };
            archive.terminal = found;
            if archive.terminal.is_none() {
                if archive.started.elapsed() > ARCHIVE_START_WAIT {
                    self.kept_scripts.archive = None;
                }
                return;
            }
        }
        let Some(Archive {
            workspace,
            terminal: Some(terminal),
            ..
        }) = &self.kept_scripts.archive
        else {
            return;
        };
        // Closed with its workspace: nothing is left to remove for.
        if !self.is_kept_workspace(workspace) {
            self.kept_scripts.archive = None;
            return;
        }
        let Some(exit) = self.core_terminal_exit(terminal, cx) else {
            return;
        };
        let Some(archive) = self.kept_scripts.archive.take() else {
            return;
        };
        if exit.code == Some(0) {
            self.remove_kept_checkout(archive.workspace, archive.path, archive.force, cx);
        } else {
            // The tab stays on the script's output, and the checkout with it.
            self.local_error = Some(fl!("worktree-scripts-archive-failed"));
            cx.notify();
        }
    }
}

#[cfg(test)]
impl SheprdWindow {
    /// Whether an archive script of `workspace` is being followed.
    pub(crate) fn follows_kept_archive(&self, workspace: &str) -> bool {
        self.kept_scripts
            .archive
            .as_ref()
            .is_some_and(|archive| archive.workspace == workspace)
    }

    /// The setup waiting for its kept workspace, if any.
    pub(crate) fn queued_kept_setup(&self) -> Option<&Launch> {
        self.kept_scripts.setup.as_ref().map(|(launch, _)| launch)
    }
}
