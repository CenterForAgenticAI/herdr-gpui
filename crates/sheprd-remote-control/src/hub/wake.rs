//! A phone on a machine without Herdr 1, such as a cloud machine set up
//! with only the pinned core, starts that core rather than finding nothing
//! running: when it opens its home view, and when it asks for new work.
//! Where Herdr 1 is installed it stays the user's to start: moving to
//! Herdr 2 is their choice.

use super::{Hub, Result, core, locate, on_core};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Herdr 1's program, whose presence leaves the core unstarted.
const HERDR: &str = if cfg!(windows) { "herdr.exe" } else { "herdr" };
/// How long after a failed start a phone's view tries again; new work
/// always tries, and says why it failed.
const RETRY: Duration = Duration::from_secs(60);

impl Hub {
    /// The core new work goes to: the one answering, else this machine's,
    /// started now when Herdr 1 neither answers nor is installed here.
    /// `Ok(None)` when the work is Herdr 1's; an error when a start failed.
    pub(super) async fn core_for_work(&self) -> Result<Option<Arc<core::Source>>> {
        if let Some(core) = self.active_core() {
            return Ok(Some(core));
        }
        let Some((core, binary)) = self.startable() else {
            return Ok(None);
        };
        let mut failed = self.waking.lock().await;
        // Another phone's request may have started it meanwhile.
        if core.connected() {
            return Ok(Some(core));
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
        let recent = self
            .waking
            .try_lock()
            .map_or(true, |failed| failed.is_some_and(|at| at.elapsed() < RETRY));
        if recent {
            return;
        }
        let hub = self.clone();
        tokio::spawn(async move {
            let _ = hub.core_for_work().await;
        });
    }

    /// This machine's core source and the program to start it with, when
    /// no Herdr 1 session answers or is installed here.
    fn startable(&self) -> Option<(Arc<core::Source>, std::path::PathBuf)> {
        // Tests' hubs look for no programs, and start nothing.
        let core = self.core.clone().filter(|_| self.search_program)?;
        if self.primary.borrow().connected
            || self.sessions.first_connected().is_some()
            || locate(HERDR).is_some()
        {
            return None;
        }
        Some((core, locate(sheprd_core_client::PROGRAM)?))
    }
}
