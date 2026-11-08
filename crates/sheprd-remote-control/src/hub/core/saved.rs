//! Which of Sheprd's saved tabs a Herdr 2 pane is in. Only Sheprd changes
//! such a tab's layout (`control::layout`): a phone names the tab and the
//! pane's terminal, and Sheprd splits or zooms it as the desktop would.

use super::{Error, Result, Source, lock, pane_id};
use crate::hub::Hub;

/// How a saved tab's id reads in the layout.
pub(super) const SAVED_TAB: &str = "core:tab:";

impl Source {
    /// The id of the saved tab `tab` names, and the address of `pane`'s
    /// terminal; `None` when either is not this core's.
    pub(super) fn saved_target(&self, tab: &str, pane: &str) -> Option<(u64, String)> {
        let id = tab.strip_prefix(SAVED_TAB)?.parse().ok()?;
        let state = lock(&self.state);
        let boot = &state.core.as_ref()?.hello().core.id;
        let terminal = state
            .terminals
            .iter()
            .find(|terminal| pane_id(boot, &terminal.address) == pane)?;
        Some((id, terminal.address.clone()))
    }
}

impl Hub {
    /// Sheprd's saved tab and the terminal of `pane` in `tab`, for a change
    /// to its layout. `None` for a pane that is not Herdr 2's; a Herdr 2
    /// pane outside Sheprd's tabs (a desktop space's, or one no tab holds)
    /// has no layout a phone may change.
    pub(crate) fn core_saved_pane(&self, tab: &str, pane: &str) -> Result<Option<(u64, String)>> {
        if !super::owns(&serde_json::json!({ "pane_id": pane })) {
            return Ok(None);
        }
        let core = self.core.as_ref().ok_or(Error::Disconnected)?;
        let target = core.saved_target(tab, pane).ok_or(Error::Unsupported)?;
        Ok(Some(target))
    }
}
