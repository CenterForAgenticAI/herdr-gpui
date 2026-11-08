//! The selected text a note quotes, and where the note sits: a selection
//! the screen shows is quoted at once, one reaching into the history is
//! read from it first.

use super::CoreTerminalView;
use crate::core_terminals::Error;
use gpui::{Bounds, Context, Pixels, point, px, size};
use sheprd_backend::History;

/// What [`CoreTerminalView::quote_selection`] found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Quote {
    /// Nothing is selected.
    Nothing,
    Ready(String),
    /// The text is being read from the history; [`CoreTerminalView::take_quote`]
    /// has it once it arrives.
    Reading,
}

impl CoreTerminalView {
    /// The selected text, for a note about it.
    pub(crate) fn quote_selection(&mut self) -> Quote {
        let Some(selection) = self.selection.filter(|selection| !selection.is_empty()) else {
            return Quote::Nothing;
        };
        let shown = self
            .screen
            .frame
            .as_deref()
            .and_then(|frame| selection.text(frame, self.top()));
        match (shown, selection.range()) {
            (Some(text), _) => Quote::Ready(text),
            (None, Some(range)) => {
                self.quoting = Some(self.ask(History::Read {
                    anchor: range.start,
                    cursor: range.end,
                }));
                Quote::Reading
            }
            (None, None) => Quote::Nothing,
        }
    }

    /// The text [`Self::quote_selection`] read from the history, once.
    pub(crate) fn take_quote(&mut self) -> Option<Result<String, Error>> {
        self.quote_ready.take()
    }

    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.take().is_some() {
            cx.notify();
        }
    }

    /// The last selected row the screen shows, in the window, for a note to
    /// sit beside.
    pub(crate) fn selection_anchor(&self) -> Option<Bounds<Pixels>> {
        let bounds = self.bounds?;
        let frame = self.screen.frame.as_ref()?;
        let (row, columns) = self
            .selection?
            .visible_rows(frame.width, self.top(), frame.height)
            .pop()?;
        let (width, height) = (self.look.cell_width, self.look.cell_height);
        Some(Bounds::new(
            bounds.origin
                + point(
                    px(f32::from(columns.start) * width),
                    px(f32::from(row) * height),
                ),
            size(
                px(f32::from(columns.end.saturating_sub(columns.start)) * width),
                px(height),
            ),
        ))
    }
}
