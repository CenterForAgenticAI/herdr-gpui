//! A phone's new work on a machine without Herdr 1, such as a cloud
//! machine set up with only the pinned core, starts that core rather than
//! failing because nothing runs. Where Herdr 1 is installed it stays the
//! user's to start: moving to Herdr 2 is their choice.

use super::{Hub, core, locate, on_core};
use std::sync::Arc;

/// Herdr 1's program, whose presence leaves the core unstarted.
const HERDR: &str = if cfg!(windows) { "herdr.exe" } else { "herdr" };

impl Hub {
    /// The core new work goes to: the one answering, else this machine's,
    /// started now when Herdr 1 neither answers nor is installed here.
    pub(super) async fn core_for_work(&self) -> Option<Arc<core::Source>> {
        if let Some(core) = self.active_core() {
            return Some(core);
        }
        // Tests' hubs look for no programs, and start nothing.
        let core = self.core.clone().filter(|_| self.search_program)?;
        if self.primary.borrow().connected || locate(HERDR).is_some() {
            return None;
        }
        let binary = locate(sheprd_core_client::PROGRAM)?;
        let _waking = self.waking.lock().await;
        // Another phone's request may have started it meanwhile.
        if core.connected() {
            return Some(core);
        }
        tracing::info!(binary = %binary.display(), "Starting herdr-core for a phone");
        let started = core.clone();
        match on_core(move || started.wake(&binary)).await {
            Ok(()) => Some(core),
            Err(error) => {
                tracing::warn!(%error, "Could not start herdr-core for a phone");
                None
            }
        }
    }
}
