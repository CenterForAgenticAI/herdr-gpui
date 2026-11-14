//! `[pane_keys]` on a herdr-core terminal: the line-editing chords and any
//! configured remap reach it as they reach a Herdr pane.

use super::{
    accessibility::{look, open_on},
    worker::{Scripted, until},
};
use crate::{
    SheprdWindow,
    core_terminals::{CoreTerminalView, Look, worker::Command},
    keymap::{DaemonKeys, Keymap, PaneKey, PaneKeys, parse_keystroke},
};
use gpui::{Entity, TestAppContext, VisualTestContext};
#[cfg(target_os = "macos")]
use sheprd_backend::ClientKeyCode;
use sheprd_backend::ClientPaneInputEvent;
use std::collections::BTreeMap;

fn keymap(entries: &[(&str, &str)]) -> Keymap {
    let table: PaneKeys = entries
        .iter()
        .map(|(typed, sent)| ((*typed).to_owned(), (*sent).to_owned()))
        .collect();
    Keymap::with_overrides(&BTreeMap::new(), &table, &DaemonKeys::default()).unwrap()
}

fn focused(
    cx: &mut TestAppContext,
    look: Look,
) -> (Entity<CoreTerminalView>, &mut VisualTestContext, Scripted) {
    let (view, cx, backend) = open_on(cx, Scripted::default(), false);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.set_look(look, cx));
        let focus = view.read(cx).focus_handle().clone();
        window.focus(&focus, cx);
    });
    (view, cx, backend)
}

/// The keyboard input the terminal received, once at least one arrived.
fn inputs(backend: &Scripted) -> Vec<ClientPaneInputEvent> {
    until(|| {
        let inputs: Vec<_> = backend
            .received
            .lock()
            .unwrap()
            .iter()
            .filter_map(|command| match command {
                Command::Input(input @ ClientPaneInputEvent::Key { .. })
                | Command::Input(input @ ClientPaneInputEvent::TextCommit(_)) => {
                    Some(input.clone())
                }
                _ => None,
            })
            .collect();
        (!inputs.is_empty()).then_some(inputs)
    })
}

#[cfg(target_os = "macos")]
#[gpui::test]
fn mac_line_editing_chords_reach_the_terminal(cx: &mut TestAppContext) {
    for (typed, code, modifiers) in [
        ("cmd-backspace", 'u', 2),
        ("cmd-left", 'a', 2),
        ("cmd-right", 'e', 2),
        ("alt-left", 'b', 4),
        ("alt-right", 'f', 4),
    ] {
        let (_view, cx, backend) = focused(cx, look());
        cx.simulate_keystrokes(typed);
        let sent = inputs(&backend);
        assert!(
            matches!(
                sent.as_slice(),
                [ClientPaneInputEvent::Key {
                    code: ClientKeyCode::Char(actual_code),
                    modifiers: actual_modifiers,
                    ..
                }] if *actual_code == code && *actual_modifiers == modifiers
            ),
            "{typed}: {sent:?}"
        );
    }
}

#[gpui::test]
fn a_configured_pane_key_sends_its_text(cx: &mut TestAppContext) {
    let pane_keys = keymap(&[("ctrl-alt-k", "text:hi")]).pane_keys().clone();
    let (_view, cx, backend) = focused(
        cx,
        Look {
            pane_keys,
            ..look()
        },
    );
    cx.simulate_keystrokes("ctrl-alt-k");
    assert_eq!(
        inputs(&backend),
        [ClientPaneInputEvent::TextCommit("hi".into())]
    );
}

#[gpui::test]
fn the_window_hands_its_pane_keys_to_herdr_core_views(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::palette::fixture_window);
    let pane_keys = cx.update(|_, cx| {
        view.update(cx, |view: &mut SheprdWindow, _| {
            view.config.keybindings = keymap(&[("ctrl-alt-k", "text:hi")]);
            view.core_look().pane_keys
        })
    });
    assert_eq!(
        pane_keys.get(&parse_keystroke("ctrl-alt-k").unwrap()),
        Some(&PaneKey::Text("hi".into()))
    );
}
