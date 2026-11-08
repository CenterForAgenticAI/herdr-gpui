//! What a herdr-core terminal asks of the window beyond its cells: bells to
//! ring and text to put on the clipboard (OSC 52). Herdr 1 forwards these
//! from its daemon; herdr-core streams raw output, so the app's emulator
//! finds them. Both go through the same paths as Herdr 1's: `[bell]`, its
//! burst limit, quiet hours and this computer's mute, and the clipboard
//! toast. A mirror only watches, so what
//! it sees is left to the tab that drives the terminal.

use super::CoreTerminalView;
use crate::SheprdWindow;
use gpui::{Context, Entity, Window};

/// Bells and the latest clipboard write a view collected since the window
/// last took them.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Effects {
    pub(crate) bells: u32,
    pub(crate) clipboard: Option<String>,
}

impl Effects {
    /// Adds what a newer take found; its clipboard write replaces an older
    /// one, as each write replaces the whole clipboard.
    pub(crate) fn merge(&mut self, newer: Self) {
        self.bells = self.bells.saturating_add(newer.bells);
        if newer.clipboard.is_some() {
            self.clipboard = newer.clipboard;
        }
    }
}

impl SheprdWindow {
    /// Takes new screens from `views` and what they ask of the window. A
    /// view whose tab is in another workspace than the one shown keeps its
    /// screen current but rings nothing, as its tab is out of sight; its
    /// clipboard writes still land, as Herdr's do from any pane.
    pub(super) fn poll_core_views(
        &mut self,
        views: impl IntoIterator<Item = (Entity<CoreTerminalView>, bool)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut effects = Effects::default();
        for (view, shown) in views {
            let mut taken = view.update(cx, |view, cx| {
                view.poll(cx);
                view.take_effects()
            });
            if !shown {
                taken.bells = 0;
            }
            effects.merge(taken);
        }
        self.apply_core_effects(effects, window, cx);
    }

    /// Rings the bells as `[bell]` asks and writes the clipboard.
    fn apply_core_effects(
        &mut self,
        effects: Effects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // herdr-core runs on this computer, so muting it as a device holds
        // its bells back, as it does a Herdr daemon's.
        let muted = crate::notification_mutes::Mutes::any(cx)
            .is_some_and(|mutes| mutes.host(crate::endpoint::LOCAL));
        if effects.bells > 0 && !muted && !self.config.notifications.quiet() {
            self.bell
                .queue(u16::try_from(effects.bells).unwrap_or(u16::MAX));
            self.ring_bell(window);
        }
        if self.apply_clipboard_writes(effects.clipboard, cx) {
            cx.notify();
        }
    }
}
