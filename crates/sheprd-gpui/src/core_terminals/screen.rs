//! What one attached terminal shows, built from its backend's updates.

use sheprd_backend::{Exit, FrameData, KeyReporting, ScrollMetrics, SurfaceUpdate};
use std::sync::Arc;

/// The latest screen of one terminal. The worker applies updates to it and
/// the window takes a copy when it changed; `Arc` keeps that copy cheap and
/// lets the worker patch rows without disturbing a frame being painted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Screen {
    pub(crate) bells: u32,
    pub(crate) ended: Option<Exit>,
    /// Why the terminal could not be shown, for its tab to say.
    pub(crate) failure: Option<String>,
    pub(crate) frame: Option<Arc<FrameData>>,
    /// Where the view is in its history.
    pub(crate) history: ScrollMetrics,
    /// The key events the program asked for beyond typed text.
    pub(crate) keys: KeyReporting,
    /// Whether the program reads the mouse, so presses go to it.
    pub(crate) mouse_reporting: bool,
    /// How often the text changed; history points hold for one revision.
    pub(crate) revision: u64,
    pub(crate) title: Option<String>,
}

impl Screen {
    /// The frame's columns and rows, once one is shown.
    pub(crate) fn grid(&self) -> Option<(u16, u16)> {
        self.frame.as_ref().map(|frame| (frame.width, frame.height))
    }

    pub(crate) fn apply(&mut self, update: SurfaceUpdate) {
        match update {
            SurfaceUpdate::Bell => self.bells = self.bells.saturating_add(1),
            SurfaceUpdate::Ended(exit) => self.ended = Some(exit),
            SurfaceUpdate::Frame(frame) => self.frame = Some(Arc::new(frame)),
            SurfaceUpdate::KeyReporting(keys) => self.keys = keys,
            SurfaceUpdate::MouseReporting(reporting) => self.mouse_reporting = reporting,
            SurfaceUpdate::Rows { cursor, rows } => {
                // Rows only patch a frame already shown.
                let Some(frame) = self.frame.as_mut() else {
                    return;
                };
                let frame = Arc::make_mut(frame);
                let width = usize::from(frame.width);
                for row in rows {
                    if row.y >= frame.height {
                        continue;
                    }
                    let start = usize::from(row.y) * width + usize::from(row.x);
                    let room = width.saturating_sub(usize::from(row.x));
                    for (slot, cell) in frame.cells[start..]
                        .iter_mut()
                        .zip(row.cells.into_iter().take(room))
                    {
                        *slot = cell;
                    }
                }
                frame.cursor = cursor;
            }
            SurfaceUpdate::Scroll { metrics, revision } => {
                self.history = metrics;
                self.revision = revision;
            }
            SurfaceUpdate::Title(title) => {
                self.title = Some(title).filter(|title| !title.is_empty())
            }
        }
    }
}
