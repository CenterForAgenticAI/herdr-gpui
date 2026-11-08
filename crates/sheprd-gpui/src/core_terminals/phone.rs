//! A phone's split or zoom of a Herdr 2 tab. The remote control daemon
//! cannot change the layout, which this app keeps with the tab, so it asks
//! (`sheprd_remote_control::control::layout`), and the window holding the
//! tab makes the change as its own pane commands do.

use super::{CoreTerminals, Purpose, panes::Axis};
use crate::SheprdWindow;
use gpui::Context;
use sheprd_backend::{Spawn, TerminalId};
use sheprd_remote_control::control::layout::{Answer, Ask, Failure, Op};

/// What a phone's change comes to in one window.
pub(super) enum Change {
    /// Made, or refused, already.
    Answered(Answer),
    /// A new shell to start beside the pane.
    Split(Purpose),
}

impl CoreTerminals {
    /// What `ask` comes to when this window holds its tab; `None` when it
    /// does not, so another window may. A zoom is made here.
    pub(super) fn phone_change(&mut self, ask: &Ask) -> Option<Change> {
        let tab = *self.tabs.keys().find(|tab| tab.number() == ask.tab)?;
        let shown = self.tabs.get_mut(&tab)?;
        let Some(terminal) = TerminalId::new(ask.terminal.as_str())
            .ok()
            .filter(|terminal| shown.panes.contains(terminal))
        else {
            return Some(Change::Answered(Answer::Failed(Failure::Gone)));
        };
        let axis = match ask.op {
            Op::SplitRight => Axis::Row,
            Op::SplitDown => Axis::Column,
            Op::Zoom => {
                // One pane already fills its tab, as Herdr leaves it.
                if shown.panes.count() > 1 {
                    let zoom = (!shown.zoom.is_open()).then_some(terminal);
                    shown.set_zoom(zoom);
                }
                return Some(Change::Answered(Answer::Done));
            }
        };
        // A mirror only watches: it never splits.
        if shown.observe {
            return Some(Change::Answered(Answer::Failed(Failure::Watching)));
        }
        Some(Change::Split(Purpose::Split {
            tab,
            at: terminal,
            axis,
        }))
    }
}

impl SheprdWindow {
    /// Makes `ask` when this window holds its tab; `None` when it does not.
    pub(crate) fn phone_layout(&mut self, ask: &Ask, cx: &mut Context<Self>) -> Option<Answer> {
        match self.core_terminals.phone_change(ask)? {
            Change::Answered(answer) => {
                cx.notify();
                Some(answer)
            }
            Change::Split(purpose) => {
                self.start_core_program(purpose, Spawn::default());
                Some(Answer::Done)
            }
        }
    }
}
