//! Herdr 2 panes as Go To reaches them: every pane of the focused
//! workspace's herdr-core tabs that types into its terminal, and showing an
//! agent's pane. The agents themselves are listed with Herdr 1's
//! (`sidebar::Listed`). Mirrors are left out: the terminal they watch is
//! listed where it types.

use crate::{
    SheprdWindow,
    browser::{Location, Store, TabId},
};
use gpui::{prelude::*, *};
use sheprd_backend::TerminalId;

impl SheprdWindow {
    /// Every pane of the focused workspace's herdr-core tabs that type into
    /// their terminals, tab by tab, as Go To lists them.
    pub(crate) fn listed_core_panes(&self, cx: &App) -> Vec<(TabId, TerminalId)> {
        let (Some(store), Some((scope, workspace))) =
            (cx.try_global::<Store>(), self.browser_key())
        else {
            return Vec::new();
        };
        store
            .in_workspace(&scope, &workspace)
            .filter_map(|tab| match &tab.location {
                Some(Location::CoreTerminal {
                    panes,
                    observe: false,
                    ..
                }) => Some((tab.id, panes.terminals())),
                _ => None,
            })
            .flat_map(|(tab, terminals)| {
                terminals
                    .into_iter()
                    .map(move |terminal| (tab, terminal.clone()))
            })
            .collect()
    }

    /// Shows a Herdr 2 agent the overview lists: its workspace, then its tab
    /// with its terminal in use, or the shell shown in place of Herdr 1.
    /// `tab` and `terminal` are as a `Session` names them.
    pub(crate) fn show_core_agent(
        &mut self,
        endpoint: &str,
        workspace: &str,
        tab: &str,
        terminal: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let in_front = self.endpoints[self.selected_endpoint].id == endpoint
            && self
                .browser_key()
                .is_some_and(|(_, shown)| shown == workspace);
        if !in_front {
            self.navigate_endpoint(endpoint, crate::NavigationTarget::Workspace(workspace), cx);
        }
        let Ok(tab) = tab.parse::<u64>() else {
            return;
        };
        if tab == super::SHELL_TAB {
            if let Some(view) = self.core_fallback_view() {
                window.focus(&view.read(cx).focus_handle().clone(), cx);
            }
            return;
        }
        let Ok(terminal) = TerminalId::new(terminal) else {
            return;
        };
        self.show_core_pane(TabId::from(tab), &terminal, window, cx);
    }

    /// Shows `tab` with `terminal` as its pane in use, unzoomed if another
    /// pane filled it, and gives that pane the keyboard.
    pub(crate) fn show_core_pane(
        &mut self,
        tab: TabId,
        terminal: &TerminalId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(shown) = self.core_terminals.tabs.get_mut(&tab) {
            if shown.zoom.is_open() && shown.zoomed.as_ref() != Some(terminal) {
                shown.set_zoom(None);
            }
            shown.activate(terminal.clone());
        }
        self.show_browser_tab(tab, window, cx);
        cx.notify();
    }
}
