//! A phone's size, held on a Herdr 2 terminal for as long as its view is
//! open. The pane's shared screen takes the driver's role at that size;
//! the core sizes the terminal back to the driver before once the screen
//! lets go, as Herdr 1 does when a direct attach drops.

use super::{Error, Result, Source, lock};
use sheprd_terminal::Size;
use std::sync::{Arc, Weak, atomic::Ordering};

/// Held until dropped. Dropping it talks to the core, so drop it off the
/// async threads.
pub(crate) struct Hold {
    address: String,
    id: u64,
    source: Weak<Source>,
}

impl Source {
    /// Holds `pane`'s terminal at `cols` by `rows`.
    pub(in crate::hub) fn hold(self: &Arc<Self>, pane: &str, cols: u16, rows: u16) -> Result<Hold> {
        let size = Size::new(cols, rows).map_err(|_| crate::hub::Error::Response)?;
        let (core, terminal) = self.terminal(pane)?;
        let screen = self.screen(&core, &terminal)?;
        let id = self.holds.fetch_add(1, Ordering::Relaxed);
        lock(&screen).hold(&core, id, size)?;
        Ok(Hold {
            address: terminal.address,
            id,
            source: Arc::downgrade(self),
        })
    }
}

impl Hold {
    pub(in crate::hub) fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let size = Size::new(cols, rows).map_err(|_| crate::hub::Error::Response)?;
        let (core, screen) = self.screen().ok_or(Error::Disconnected)?;
        lock(&screen).hold(&core, self.id, size)
    }

    /// The screen and its connection, while the hold still has them. A
    /// screen dropped meanwhile took its driving attach with it.
    fn screen(&self) -> Option<(Arc<sheprd_core_client::Core>, super::SharedScreen)> {
        let source = self.source.upgrade()?;
        let state = lock(&source.state);
        Some((
            state.core.clone()?,
            state.screens.get(&self.address)?.clone(),
        ))
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        let Some((core, screen)) = self.screen() else {
            return;
        };
        if let Err(error) = lock(&screen).release(&core, self.id) {
            tracing::debug!(%error, "a phone's size was not released");
        }
    }
}
