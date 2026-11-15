//! A fresh, cancellable preflight owned by one close dialog. Unknown status
//! requires the same explicit consent as work that has not been published.
// i18n: migrated
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use gpui::App;
use sheprd_client::{
    ConnectTarget,
    protocol::{ClientShellSnapshot, ClientShellWorktree},
};

use super::workspace::WorkspaceTarget;
use crate::i18n::fl;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Checkout {
    id: String,
    worktree: Option<ClientShellWorktree>,
    branch: Option<String>,
    input: Option<crate::pull_request::Input>,
}

#[cfg(test)]
mod tests;

/// What the user types to close anyway, the same word in every language.
const CONSENT: &str = "close"; // i18n: data

/// The Git line while the check runs.
pub(super) fn checking() -> String {
    fl!("workspace-close-checking")
}

#[derive(Default)]
pub(super) struct Report {
    pub dirty: bool,
    pub unpushed: bool,
    pub unknown: bool,
}

impl Report {
    pub fn needs_consent(&self) -> bool {
        self.dirty || self.unpushed || self.unknown
    }
}

pub(super) struct CloseCheck {
    checkouts: Vec<Checkout>,
    receiver: mpsc::Receiver<Report>,
    cancelled: Arc<AtomicBool>,
    pub report: Option<Report>,
}

impl Drop for CloseCheck {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn app_input(target: &WorkspaceTarget) -> Option<crate::pull_request::Input> {
    let tree = target.worktree.as_ref()?;
    let branch = target.branch.as_ref()?.clone();
    let checkout = match &target.source {
        super::workspace::Source::Directory(path)
        | super::workspace::Source::Repository { root: path, .. }
            if !path.is_empty() =>
        {
            path.clone()
        }
        _ => return None,
    };
    Some(crate::pull_request::Input {
        checkout: Some(checkout),
        repo_key: Some(tree.key.clone()),
        branch,
    })
}

fn checkouts(snapshot: Option<&ClientShellSnapshot>, target: &WorkspaceTarget) -> Vec<Checkout> {
    if target.keeper == sheprd_session::Keeper::App {
        return vec![Checkout {
            id: target.id.clone(),
            worktree: target.worktree.clone(),
            branch: target.branch.clone(),
            input: app_input(target),
        }];
    }
    snapshot
        .into_iter()
        .flat_map(|snapshot| snapshot.workspaces.iter())
        .filter(|workspace| target.close_members.contains(&workspace.workspace_id))
        .map(|workspace| Checkout {
            id: workspace.workspace_id.clone(),
            worktree: workspace.worktree.clone(),
            branch: workspace.branch.clone(),
            input: None,
        })
        .collect()
}

impl CloseCheck {
    #[cfg(test)]
    pub(super) fn fixture(
        snapshot: &ClientShellSnapshot,
        target: &WorkspaceTarget,
        report: Option<Report>,
    ) -> Self {
        let (_, receiver) = mpsc::sync_channel(1);
        Self {
            checkouts: checkouts(Some(snapshot), target),
            receiver,
            cancelled: Arc::new(AtomicBool::new(false)),
            report,
        }
    }
}

fn remote_close_status(
    target: &ConnectTarget,
    input: &crate::pull_request::Input,
    cancelled: &AtomicBool,
) -> crate::Result<(bool, bool)> {
    let checkout = input
        .checkout
        .as_deref()
        .ok_or(crate::Error::StaleWorkspace)?;
    let host = sheprd_remote::work::host_for_kept(target)?;
    let identity = sheprd_remote::work::identity(&host, checkout, cancelled)?;
    if input.repo_key.as_deref() != Some(identity.key.as_str())
        || identity.branch.as_deref() != Some(input.branch.as_str())
    {
        return Err(crate::Error::StaleWorkspace);
    }
    let dirty = !sheprd_remote::work::git(
        &host,
        checkout,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ],
        cancelled,
    )?
    .is_empty();
    let unpushed = !sheprd_remote::work::git(
        &host,
        checkout,
        &["rev-list", "--max-count=1", "HEAD", "--not", "--remotes"],
        cancelled,
    )?
    .is_empty();
    Ok((dirty, unpushed))
}

impl CloseCheck {
    pub fn start(
        snapshot: Option<&ClientShellSnapshot>,
        target: &WorkspaceTarget,
        endpoint: &ConnectTarget,
        local: bool,
        cx: &App,
    ) -> Self {
        let checkouts = checkouts(snapshot, target);
        let app = target.keeper == sheprd_session::Keeper::App;
        let endpoint = endpoint.clone();
        let mut report = Report {
            unknown: checkouts.len() > 128,
            ..Report::default()
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancellation = cancelled.clone();
        let jobs = checkouts
            .iter()
            .take(128)
            .map(|checkout| {
                checkout.input.clone().or_else(|| {
                    crate::pull_request::repository_input(
                        checkout.worktree.as_ref(),
                        checkout.branch.as_deref(),
                    )
                    .ok()
                })
            })
            .collect::<Vec<_>>();
        cx.background_executor()
            .spawn(async move {
                let deadline = Instant::now() + Duration::from_secs(15);
                for input in jobs {
                    if cancellation.load(Ordering::Relaxed) {
                        return;
                    }
                    let status = input.ok_or(crate::Error::StaleWorkspace).and_then(|input| {
                        if app {
                            if endpoint.is_remote() {
                                remote_close_status(&endpoint, &input, &cancellation)
                            } else {
                                Ok(crate::git::close_status(&input, deadline, &|| {
                                    cancellation.load(Ordering::Relaxed)
                                })?)
                            }
                        } else if !local {
                            Err(sheprd_github::pull_request::Error::UntrustedEndpoint.into())
                        } else {
                            Ok(crate::git::close_status(&input, deadline, &|| {
                                cancellation.load(Ordering::Relaxed)
                            })?)
                        }
                    });
                    match status {
                        Ok((dirty, unpushed)) => {
                            report.dirty |= dirty;
                            report.unpushed |= unpushed;
                        }
                        Err(_) => report.unknown = true,
                    }
                }
                let _ = sender.send(report);
            })
            .detach();
        Self {
            checkouts,
            receiver,
            cancelled,
            report: None,
        }
    }

    pub fn poll(&mut self) -> bool {
        if self.report.is_some() {
            return false;
        }
        match self.receiver.try_recv() {
            Ok(report) => self.report = Some(report),
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.report = Some(Report {
                    unknown: true,
                    ..Report::default()
                })
            }
        }
        true
    }

    pub fn current(&self, snapshot: &ClientShellSnapshot, target: &WorkspaceTarget) -> bool {
        self.checkouts == checkouts(Some(snapshot), target)
    }

    /// Whether the close may go without the dialog once
    /// `confirm_close_workspace` is off: the check finished without finding
    /// uncommitted files or unpushed commits. Unverifiable status does not
    /// hold it, since a workspace outside a Herdr worktree is never
    /// verifiable and turning confirmation off asked not to be stopped.
    pub fn skippable(&self, confirm: bool) -> bool {
        !confirm
            && self
                .report
                .as_ref()
                .is_some_and(|report| !report.dirty && !report.unpushed)
    }

    /// Whether the close may be sent, with `text` typed as consent.
    pub fn allows(&self, text: &str, confirm: bool) -> bool {
        self.ready(text) || self.skippable(confirm)
    }

    /// The dialog's Git line. It asks for typed consent only while the
    /// close needs it, since the input shows only then.
    pub fn status(&self, confirm: bool) -> String {
        let Some(report) = &self.report else {
            return checking();
        };
        let mut warnings = Vec::new();
        if report.dirty {
            warnings.push(fl!("workspace-close-dirty"));
        }
        if report.unpushed {
            warnings.push(fl!("workspace-close-unpushed"));
        }
        if report.unknown {
            warnings.push(fl!("workspace-close-unknown"));
        }
        if !report.needs_consent() {
            warnings.push(fl!("workspace-close-clean"));
        } else if !self.skippable(confirm) {
            warnings.push(fl!("workspace-close-consent", word = CONSENT));
        }
        warnings.join(" ")
    }

    pub fn ready(&self, text: &str) -> bool {
        self.report
            .as_ref()
            .is_some_and(|report| !report.needs_consent() || text == CONSENT)
    }
}
