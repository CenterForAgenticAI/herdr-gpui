//! The pointer over a herdr-core pane. A program that reads the mouse gets
//! its presses, drags, and wheel; otherwise a press selects text, a double
//! click a word and a triple click a line, and the wheel scrolls history.
//! Shift selects even over a program that reads the mouse, as terminals do.

use super::CoreTerminalView;
use crate::{
    core_terminals::selection::{Cell, Selection},
    terminal::wheel_rows,
    terminal_painter::{Highlight, Tint},
};
use gpui::{prelude::*, *};
use sheprd_backend::{
    ClientMouseButton, ClientMouseKind, ClientMousePosition, ClientPaneInputEvent,
};

/// Where a press is going: to the program, or into a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Press {
    Reported(ClientMouseButton),
    Selecting,
}

fn button(button: MouseButton) -> Option<ClientMouseButton> {
    match button {
        MouseButton::Left => Some(ClientMouseButton::Left),
        MouseButton::Right => Some(ClientMouseButton::Right),
        MouseButton::Middle => Some(ClientMouseButton::Middle),
        _ => None,
    }
}

fn modifier_bits(modifiers: Modifiers) -> u8 {
    u8::from(modifiers.shift)
        | (u8::from(modifiers.control) << 1)
        | (u8::from(modifiers.alt) << 2)
        | (u8::from(modifiers.platform) << 3)
}

impl CoreTerminalView {
    /// The cell under `position`, held inside the screen.
    pub(super) fn cell_at(&self, position: Point<Pixels>) -> Option<Cell> {
        let bounds = self.bounds?;
        let frame = self.screen.frame.as_ref()?;
        if frame.width == 0 || frame.height == 0 {
            return None;
        }
        let x = f32::from(position.x - bounds.origin.x) / self.look.cell_width.max(1.);
        let y = f32::from(position.y - bounds.origin.y) / self.look.cell_height.max(1.);
        // Clamped to the screen first, so the casts cannot truncate.
        let column = x.clamp(0., f32::from(frame.width - 1)) as u16;
        let row = y.clamp(0., f32::from(frame.height - 1)) as u16;
        Some(Cell { row, column })
    }

    fn report(&self, kind: ClientMouseKind, cell: Cell, modifiers: Modifiers, lines: u16) {
        self.send(ClientPaneInputEvent::Mouse {
            kind,
            position: ClientMousePosition::Cell {
                column: cell.column,
                row: cell.row,
            },
            geometry: None,
            modifiers: modifier_bits(modifiers),
            lines,
        });
    }

    pub(super) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if self.press_link(event) {
            return;
        }
        let Some(cell) = self.cell_at(event.position) else {
            return;
        };
        if self.screen.mouse_reporting && !event.modifiers.shift {
            if let Some(pressed) = button(event.button) {
                self.press = Some(Press::Reported(pressed));
                let modifiers = self.reported_modifiers(event.button, event.modifiers);
                self.report(ClientMouseKind::Down(pressed), cell, modifiers, 1);
            }
            return;
        }
        if event.button != MouseButton::Left {
            return;
        }
        let Some(frame) = self.screen.frame.as_deref() else {
            return;
        };
        self.selection = Some(match event.click_count {
            2 => Selection::word(frame, cell),
            count if count >= 3 => Selection::line(frame, cell),
            _ => Selection::at(cell),
        });
        self.press = Some(Press::Selecting);
        cx.notify();
    }

    pub(super) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hover_link(event.position, event.modifiers, cx);
        let Some(cell) = self.cell_at(event.position) else {
            return;
        };
        match self.press {
            Some(Press::Reported(pressed)) if event.pressed_button.is_some() => {
                self.report(ClientMouseKind::Drag(pressed), cell, event.modifiers, 1);
            }
            Some(Press::Selecting) if event.pressed_button == Some(MouseButton::Left) => {
                if let Some(selection) = &mut self.selection {
                    selection.extend(cell);
                    cx.notify();
                }
            }
            // Movement with no button held, for a program that asks for
            // every motion (mode 1003); the encoder drops it otherwise.
            None if event.pressed_button.is_none()
                && self.screen.mouse_reporting
                && !event.modifiers.shift
                && self.hovered != Some(cell) =>
            {
                self.hovered = Some(cell);
                self.report(ClientMouseKind::Moved, cell, event.modifiers, 1);
            }
            _ => {}
        }
    }

    pub(super) fn mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.release_link(event, window, cx) {
            return;
        }
        match self.press.take() {
            Some(Press::Reported(pressed)) => {
                if let Some(cell) = self.cell_at(event.position) {
                    let modifiers = self.reported_modifiers(event.button, event.modifiers);
                    self.report(ClientMouseKind::Up(pressed), cell, modifiers, 1);
                }
            }
            // Releasing a selection copies it while `copy_on_select` is on;
            // otherwise it stays highlighted for Cmd-C or Ctrl-C.
            Some(Press::Selecting)
                if self.look.copy_on_select
                    && event.button == MouseButton::Left
                    && self.copy_selection(cx) =>
            {
                self.finish_copy(window, cx);
            }
            Some(Press::Selecting) | None => {}
        }
    }

    /// Whole rows of wheel motion, the remainder kept for the next event,
    /// as one event carrying the count: the terminal scrolls history by it,
    /// or repeats it as reports for a program reading the wheel.
    pub(super) fn scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        let Some(cell) = self.cell_at(event.position) else {
            return;
        };
        self.wheel_rest += wheel_rows(event, self.look.cell_height, self.look.scroll_lines);
        let steps = self.wheel_rest.trunc();
        self.wheel_rest -= steps;
        if steps == 0. {
            return;
        }
        let kind = if steps > 0. {
            ClientMouseKind::ScrollUp
        } else {
            ClientMouseKind::ScrollDown
        };
        // Clamped to a u16 first, so the cast cannot truncate.
        let lines = steps.abs().min(f32::from(u16::MAX)) as u16;
        self.report(kind, cell, event.modifiers, lines);
    }

    /// The modifiers a press reports. A right-click held with exactly
    /// Herdr's `right_click_passthrough_modifier` reaches the program
    /// without it, as Herdr strips it. A herdr-core pane has no pane menu
    /// yet, so its other right-clicks reach the program as they are.
    fn reported_modifiers(&self, button: MouseButton, held: Modifiers) -> Modifiers {
        if button == MouseButton::Right && self.look.right_click_passthrough.matches(held) {
            Modifiers::default()
        } else {
            held
        }
    }

    /// Ends a copy: the copied flash, and the highlight dropped unless
    /// `keep_selection_after_copy` keeps it.
    pub(super) fn finish_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.look.keep_selection {
            self.selection = None;
        }
        cx.notify();
        if let Some(to_window) = &self.to_window {
            to_window(super::ToWindow::Copied, window, cx);
        }
    }

    /// Copies the selection; false when nothing is selected.
    pub(super) fn copy_selection(&self, cx: &mut App) -> bool {
        let Some(frame) = self.screen.frame.as_deref() else {
            return false;
        };
        let Some(text) = self
            .selection
            .filter(|selection| !selection.is_empty())
            .map(|selection| selection.text(frame))
        else {
            return false;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    /// The selection as the painter tints it.
    pub(crate) fn highlights(&self) -> Vec<Highlight> {
        let width = self.screen.frame.as_ref().map_or(0, |frame| frame.width);
        self.selection
            .map(|selection| {
                selection
                    .rows(width)
                    .into_iter()
                    .map(|(row, columns)| Highlight {
                        row,
                        columns,
                        tint: Tint::Selection,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
