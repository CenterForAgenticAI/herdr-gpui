//! Which sections of a chat's rail show. A section nobody clicked follows
//! the work: what runs beside the conversation opens, and the plan and the
//! changed files fold to a line while it does, so they keep their room in
//! every chat with nothing running. A click decides for good.

use crate::motion::Toggle;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RailSection {
    Subagents,
    Shells,
    Monitors,
    Plan,
    Changed,
}

impl RailSection {
    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    /// Whether it shows on its own, with `busy` saying anything still runs.
    fn opens_by_itself(self, busy: bool) -> bool {
        match self {
            Self::Subagents | Self::Shells | Self::Monitors => true,
            Self::Plan | Self::Changed => !busy,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RailFolds {
    /// Whether a section lists what ended; it starts folded away.
    ended: [Toggle; 5],
    /// Each section the user opened or closed.
    sections: [Option<Toggle>; 5],
}

impl RailFolds {
    pub(crate) fn is_open(&self, section: RailSection, busy: bool) -> bool {
        self.sections[section.index()]
            .map_or_else(|| section.opens_by_itself(busy), |fold| fold.is_open())
    }

    /// How much of `section` shows at `now`, from 0 to 1.
    pub(crate) fn shown(&self, section: RailSection, busy: bool, now: Instant) -> f32 {
        self.sections[section.index()].map_or_else(
            || f32::from(u8::from(section.opens_by_itself(busy))),
            |fold| fold.shown(now),
        )
    }

    pub(crate) fn toggle(&mut self, section: RailSection, busy: bool, now: Instant) {
        let open = self.is_open(section, busy);
        let mut fold = self.sections[section.index()].unwrap_or_else(|| {
            let mut fold = Toggle::default();
            fold.set(open, now, true);
            fold
        });
        fold.set(!open, now, false);
        self.sections[section.index()] = Some(fold);
    }

    pub(crate) fn ended_is_open(&self, section: RailSection) -> bool {
        self.ended[section.index()].is_open()
    }

    pub(crate) fn ended_shown(&self, section: RailSection, now: Instant) -> f32 {
        self.ended[section.index()].shown(now)
    }

    pub(crate) fn toggle_ended(&mut self, section: RailSection, now: Instant) {
        let fold = &mut self.ended[section.index()];
        let open = !fold.is_open();
        fold.set(open, now, false);
    }

    /// Whether anything is still opening or closing.
    pub(crate) fn moving(&self, now: Instant) -> bool {
        self.ended.iter().any(|fold| fold.moving(now))
            || self.sections.iter().flatten().any(|fold| fold.moving(now))
    }
}

#[cfg(test)]
mod tests;
