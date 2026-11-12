//! How a paragraph's words and inline formulas sit in their lines.

use gpui::{AnyElement, IntoElement as _, ParentElement as _, Styled as _, div};

/// What an inline item is, for where a line may break around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Piece {
    /// A word, and whether it ends in whitespace.
    Word {
        spaced: bool,
    },
    Math,
    Break,
}

/// Whether `next` must stay on the line of `previous`. A browser breaks a
/// line only after whitespace, so no break falls between a formula and the
/// text it touches: `$x$, then` or `($x$)`. Words keep their own breaks.
pub(super) fn joined(previous: Piece, next: Piece) -> bool {
    match (previous, next) {
        (Piece::Math, Piece::Word { .. } | Piece::Math) => true,
        (Piece::Word { spaced }, Piece::Math) => !spaced,
        _ => false,
    }
}

/// A paragraph's items in runs no line breaks inside. A run of several
/// items is one flat non-wrapping row, however long it grows.
#[derive(Default)]
pub(super) struct Runs {
    done: Vec<AnyElement>,
    run: Vec<AnyElement>,
    last: Option<Piece>,
}

impl Runs {
    pub(super) fn push(&mut self, piece: Piece, item: AnyElement) {
        if !self.last.is_some_and(|last| joined(last, piece)) {
            self.flush();
        }
        self.run.push(item);
        self.last = Some(piece);
    }

    pub(super) fn finish(mut self) -> Vec<AnyElement> {
        self.flush();
        self.done
    }

    fn flush(&mut self) {
        if self.run.len() > 1 {
            let run = div()
                .flex()
                .flex_none()
                .items_baseline()
                .children(self.run.drain(..));
            self.done.push(run.into_any_element());
        } else {
            self.done.append(&mut self.run);
        }
    }
}
