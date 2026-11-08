//! History commands on the herdr-core pane holding the keyboard: find,
//! find again, clear, and editing the history in `$EDITOR`. A Herdr 1 pane
//! asks its daemon for these; a herdr-core view answers them from its own
//! emulator, so they work whatever Herdr 1 is installed or supports.

use super::{CoreTerminalView, Error, Purpose, fallback::Fallback, panes::Axis};
use crate::{SheprdWindow, controls::Command, i18n::fl, window::Flash};
use gpui::{prelude::*, *};
use sheprd_backend::{Spawn, TerminalId};
use std::path::{Path, PathBuf};

/// A tab's pane, by its tab and terminal.
type Pane = (crate::browser::TabId, TerminalId);

impl super::CoreTerminals {
    /// The view holding the keyboard, a tab's pane or the stand-in shell,
    /// even while its own find bar has it.
    pub(crate) fn focused_view(
        &self,
        window: &Window,
        cx: &App,
    ) -> Option<Entity<CoreTerminalView>> {
        self.shown()
            .find(|view| view.read(cx).focus_handle().contains_focused(window, cx))
            .cloned()
    }
}

impl SheprdWindow {
    /// Runs `command` on the herdr-core view holding the keyboard. False
    /// when none does or the command is not a history command, so the
    /// window runs it as usual.
    pub(crate) fn core_history_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(view) = self.core_terminals.focused_view(window, cx) else {
            return false;
        };
        match command {
            Command::Find => view.update(cx, |view, cx| view.open_find(window, cx)),
            Command::FindNext | Command::FindPrevious => view.update(cx, |view, cx| {
                view.find_again(command == Command::FindPrevious, window, cx)
            }),
            Command::ClearPane => view.update(cx, |view, cx| view.clear_history(cx)),
            Command::EditScrollback => view.update(cx, |view, _| view.request_edit()),
            _ => return false,
        }
        true
    }

    /// Opens the history text a view read in `$EDITOR`, once it arrives:
    /// saved to a private file off the UI thread, then edited in a pane
    /// beside the view that closes when the editor quits, which also
    /// removes the file.
    pub(super) fn poll_core_edits(&mut self, cx: &mut Context<Self>) {
        let mut views: Vec<(Option<Pane>, Entity<CoreTerminalView>)> = self
            .core_terminals
            .views
            .iter()
            .map(|(key, view)| (Some(key.clone()), view.clone()))
            .collect();
        if let Fallback::Shown(view) = &self.core_terminals.fallback {
            views.push((None, view.clone()));
        }
        for (pane, view) in views {
            let Some(text) = view.update(cx, |view, _| view.take_edit()) else {
                continue;
            };
            match text {
                Ok(text) => self.edit_core_history(pane, text, cx),
                Err(error) => self.core_edit_failed(&error, cx),
            }
        }
    }

    fn edit_core_history(&mut self, pane: Option<Pane>, text: String, cx: &mut Context<Self>) {
        // The stand-in shell has no tab to split: the editor gets its own.
        let purpose = match pane {
            Some((tab, at)) => Purpose::Split {
                tab,
                at,
                axis: Axis::Row,
            },
            None => match self.browser_key() {
                Some((scope, workspace)) => Purpose::Tab { scope, workspace },
                None => {
                    self.show_flash(Flash::warning(fl!("core-terminals-open-workspace")), cx);
                    return;
                }
            },
        };
        let saved = cx.background_spawn(async move { save_history(&text) });
        cx.spawn(async move |this, cx| {
            let saved = saved.await;
            let _ = this.update(cx, |this, cx| match saved {
                Ok(path) => this.start_core_program(purpose, editor_spawn(&path)),
                Err(error) => this.core_edit_failed(&error, cx),
            });
        })
        .detach();
    }

    fn core_edit_failed(&mut self, error: &Error, cx: &mut Context<Self>) {
        tracing::warn!(%error, "Could not edit a herdr-core terminal's history");
        self.show_flash(Flash::warning(fl!("core-terminals-edit-failed")), cx);
    }
}

/// Saves `text` to a file only the user can read, kept for the editor,
/// which removes it when it quits.
fn save_history(text: &str) -> Result<PathBuf, Error> {
    use std::io::Write as _;
    let mut file = tempfile::Builder::new()
        .prefix("sheprd-scrollback-")
        .suffix(".txt")
        .tempfile()
        .map_err(Error::EditFile)?;
    file.write_all(text.as_bytes()).map_err(Error::EditFile)?;
    let (_, path) = file.keep().map_err(|error| Error::EditFile(error.error))?;
    Ok(path)
}

/// The editor herdr-core runs on `path`: `$VISUAL`, else `$EDITOR`, else
/// `vi` (Notepad on Windows), through a shell so an editor with arguments
/// such as `code --wait` works, removing the file once the editor quits.
/// The app's own editor settings go along, as a core started elsewhere may
/// not have them.
pub(super) fn editor_spawn(path: &Path) -> Spawn {
    let path = path.display().to_string();
    #[cfg(not(windows))]
    let argv = vec![
        "/bin/sh".to_owned(),
        "-c".to_owned(),
        r#"eval "${VISUAL:-${EDITOR:-vi}} \"\$1\""; status=$?; rm -f "$1"; exit $status"#
            .to_owned(),
        "sheprd-scrollback".to_owned(),
        path,
    ];
    #[cfg(windows)]
    let argv = {
        let editor = ["VISUAL", "EDITOR"]
            .into_iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "notepad.exe".to_owned());
        vec![
            "cmd.exe".to_owned(),
            "/D".to_owned(),
            "/C".to_owned(),
            format!("{editor} \"{path}\" & del \"{path}\""),
        ]
    };
    let env = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| Some((name.to_owned(), std::env::var(name).ok()?)))
        .filter(|(_, value)| !value.trim().is_empty())
        .collect();
    Spawn {
        argv,
        env,
        ..Spawn::default()
    }
}
