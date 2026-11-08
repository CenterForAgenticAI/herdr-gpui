//! The checkout of the herdr-core pane the window shows, for the Git chip,
//! the pull request chip, and reviews. Herdr reports a workspace's branch;
//! herdr-core reports only a pane's folder, so Git is asked for the branch
//! on a thread, one question at a time, again when the folder changes and
//! every few seconds while it stays, as a checkout or a branch switch would.

use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

/// How long a branch read stays current before Git is asked again.
const FRESH: Duration = Duration::from_secs(5);
/// How long Git may take to answer; a hung one is abandoned.
const TIMEOUT: Duration = Duration::from_secs(5);
/// The longest branch name a lookup accepts.
const MAX_BRANCH: usize = 1024;

/// The shown pane's folder and the branch checked out there.
#[derive(Default)]
pub(crate) struct Checkout {
    /// The folder last asked about, the branch Git named, and when.
    known: Option<(PathBuf, Option<String>, Instant)>,
    asking: Option<(PathBuf, mpsc::Receiver<Option<String>>)>,
}

impl Checkout {
    /// The branch checked out in `folder`, when Git has said and still would.
    pub(crate) fn branch(&self, folder: &Path) -> Option<&str> {
        let (known, branch, _) = self.known.as_ref()?;
        (known == folder).then_some(branch.as_deref()).flatten()
    }

    /// Whether Git has answered about `folder`, with a branch or without.
    #[cfg(test)]
    pub(super) fn answered(&self, folder: &Path) -> bool {
        self.known.as_ref().is_some_and(|(known, ..)| known == folder)
    }

    /// Takes Git's answer and asks again about `folder` when it is new or
    /// the answer is old; whether what [`Self::branch`] says changed.
    pub(crate) fn poll(&mut self, folder: Option<&Path>, now: Instant) -> bool {
        let mut changed = false;
        if let Some((asked, answer)) = &self.asking {
            match answer.try_recv() {
                Ok(branch) => {
                    let asked = asked.clone();
                    changed = self
                        .known
                        .as_ref()
                        .is_none_or(|(known, old, _)| *known != asked || *old != branch);
                    self.known = Some((asked, branch, now));
                    self.asking = None;
                }
                Err(mpsc::TryRecvError::Empty) => return false,
                Err(mpsc::TryRecvError::Disconnected) => self.asking = None,
            }
        }
        let Some(folder) = folder else {
            changed |= self.known.take().is_some();
            return changed;
        };
        let current = self.known.as_ref().is_some_and(|(known, _, at)| {
            known == folder && now.saturating_duration_since(*at) < FRESH
        });
        if !current {
            self.ask(folder.to_path_buf());
        }
        changed
    }

    fn ask(&mut self, folder: PathBuf) {
        let (send, answer) = mpsc::sync_channel(1);
        let asked = folder.clone();
        let spawned = std::thread::Builder::new()
            .name("herdr-core-branch".into())
            .spawn(move || {
                let _ = send.send(branch(&asked));
            });
        match spawned {
            Ok(_) => self.asking = Some((folder, answer)),
            Err(error) => tracing::warn!(%error, "Could not start a branch lookup"),
        }
    }
}

/// Blocking: the branch checked out in `folder`, or none on a detached head,
/// outside a checkout, or when Git does not answer in time.
fn branch(folder: &Path) -> Option<String> {
    let mut command = sheprd_client::new_command("git");
    command
        .args(["-c", "core.fsmonitor=false", "-C"])
        .arg(folder)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut output = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut output).ok()?;
    valid(output.trim_end_matches(['\n', '\r']))
}

/// A branch name a lookup may carry: not empty, bounded, and plain text.
pub(super) fn valid(branch: &str) -> Option<String> {
    (!branch.is_empty() && branch.len() <= MAX_BRANCH && !branch.chars().any(char::is_control))
        .then(|| branch.to_owned())
}
