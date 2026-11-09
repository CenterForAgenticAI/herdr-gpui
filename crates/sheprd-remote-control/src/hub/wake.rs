//! A phone on a machine without Herdr 1, such as a cloud machine set up
//! with only the pinned core, starts that core rather than finding nothing
//! running: when it opens its home view, and when it asks for new work.
//! Where Herdr 1 is installed it stays the user's to start: moving to
//! Herdr 2 is their choice.

use super::{Hub, Result, core, locate, locate_on, on_core};
use std::{
    ffi::OsStr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

/// Herdr 1's program, whose presence leaves the core unstarted.
const HERDR: &str = if cfg!(windows) { "herdr.exe" } else { "herdr" };
/// How long after a failed start a phone's view tries again; new work
/// always tries, and says why it failed.
const RETRY: Duration = Duration::from_secs(60);

/// What asks for this machine's core to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wake {
    /// A phone opening its home view.
    View,
    /// A phone's new tab, chat, or workspace.
    Work,
}

impl Hub {
    /// The core new work goes to: the one answering, else this machine's,
    /// started now when Herdr 1 neither answers nor is installed here.
    /// `Ok(None)` when the work is Herdr 1's; an error when a start failed.
    pub(super) async fn core_for_work(&self) -> Result<Option<Arc<core::Source>>> {
        self.start_core(Wake::Work).await
    }

    /// [`Self::core_for_work`] for `wake`: a view does not start a core again
    /// within a minute of a failed start; new work always may.
    async fn start_core(&self, wake: Wake) -> Result<Option<Arc<core::Source>>> {
        if let Some(core) = self.active_core() {
            return Ok(Some(core));
        }
        let Some(core) = self.startable() else {
            return Ok(None);
        };
        // The login shell's PATH can take a moment to read the first time.
        let Some(binary) = on_core(|| Ok(core_program())).await? else {
            return Ok(None);
        };
        let mut failed = self.waking.lock().await;
        // Another phone's request may have started it meanwhile.
        if core.connected() {
            return Ok(Some(core));
        }
        // Checked under the lock, so views queued behind a failed start do
        // not each try again.
        if wake == Wake::View && failed.is_some_and(|at| at.elapsed() < RETRY) {
            return Ok(None);
        }
        tracing::info!(binary = %binary.display(), "Starting herdr-core for a phone");
        let started = core.clone();
        match on_core(move || started.wake(&binary)).await {
            Ok(()) => {
                *failed = None;
                Ok(Some(core))
            }
            Err(error) => {
                tracing::warn!(%error, "Could not start herdr-core for a phone");
                *failed = Some(Instant::now());
                Err(error)
            }
        }
    }

    /// Starts this machine's core in the background for a phone opening its
    /// home view with nothing answering, so the phone finds a workspace to
    /// start work in; at most once a minute after a failure.
    pub(crate) fn wake_for_phone(self: &Arc<Self>) {
        if self.layout().connected || self.startable().is_none() {
            return;
        }
        // A start underway, or a recent failure: nothing to queue.
        let idle = self
            .waking
            .try_lock()
            .is_ok_and(|failed| failed.is_none_or(|at| at.elapsed() >= RETRY));
        if !idle {
            return;
        }
        let hub = self.clone();
        tokio::spawn(async move {
            let _ = hub.start_core(Wake::View).await;
        });
    }

    /// This machine's core source, when no Herdr 1 session answers; whether
    /// Herdr 1 is installed is [`core_program`]'s to say.
    fn startable(&self) -> Option<Arc<core::Source>> {
        // Tests' hubs look for no programs, and start nothing.
        let core = self.core.clone().filter(|_| self.search_program)?;
        if self.primary.borrow().connected || self.sessions.first_connected().is_some() {
            return None;
        }
        Some(core)
    }
}

/// The program to start this machine's core with, when Herdr 1 is not
/// installed here. Blocking: it may read the login shell's environment.
fn core_program() -> Option<PathBuf> {
    let login = sheprd_client::login_env::path();
    if find(HERDR, login.as_deref()).is_some() {
        return None;
    }
    find(sheprd_core_client::PROGRAM, login.as_deref())
}

/// `program` where the daemon looks, else on the login shell's `login`
/// PATH: a service's own PATH can leave out a directory the user's shell
/// adds, such as Homebrew's, and a program there is still installed.
pub(super) fn find(program: &str, login: Option<&OsStr>) -> Option<PathBuf> {
    locate(program).or_else(|| login.and_then(|path| locate_on(program, Some(path))))
}
