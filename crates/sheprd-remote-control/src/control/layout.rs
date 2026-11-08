//! A phone's change to the layout of one of Sheprd's Herdr 2 tabs: a split
//! or a zoom. herdr-core keeps no layout; Sheprd keeps it in its saved tabs
//! and changes it as the desktop does, so the phone never writes it and
//! never fights the desktop over it. The question waits in the daemon with
//! the phones' questions about workspaces (`control::changes`), and Sheprd
//! answers it with `Request::LayoutAnswer`.

use super::changes::AskId;
use serde::{Deserialize, Serialize};

/// The longest terminal address sent.
pub const MAX_TERMINAL: usize = 256;

/// One change to one of Sheprd's Herdr 2 tabs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ask {
    pub id: AskId,
    /// The saved tab, by the id Sheprd gave it.
    pub tab: u64,
    /// The pane's terminal, by its herdr-core address.
    pub terminal: String,
    pub op: Op,
}

impl Ask {
    /// Whether it stays within this module's bounds.
    pub fn valid(&self) -> bool {
        !self.terminal.is_empty() && self.terminal.len() <= MAX_TERMINAL
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// A new shell beside the pane, as Split Right does on the desktop.
    SplitRight,
    /// A new shell below it.
    SplitDown,
    /// The pane fills its tab, or the split shows again.
    Zoom,
}

/// Sheprd's answer to an [`Ask`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer", content = "failure")]
pub enum Answer {
    Done,
    Failed(Failure),
}

/// Why Sheprd did not change the tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    #[error("no Sheprd window has this tab open")]
    Gone,
    #[error("this tab only watches its terminal; split it where it types")]
    Watching,
    /// Last: what a newer Sheprd answers that this daemon does not know.
    #[error("Sheprd could not change this tab")]
    #[serde(other)]
    Refused,
}
