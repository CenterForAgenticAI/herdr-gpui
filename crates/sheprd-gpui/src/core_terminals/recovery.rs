//! Replay core-owned recovery aliases into saved layouts before adoption.
//! Core labels survive an app crash before its asynchronous tabs-file write.

use super::{Host, TabPanes};
use crate::{SheprdWindow, browser::Store};
use sheprd_backend::TerminalId;
use std::collections::BTreeMap;

impl TabPanes {
    fn remap(&mut self, replacements: &BTreeMap<TerminalId, TerminalId>) -> bool {
        let mut changed = false;
        for (old, new) in replacements {
            if !self.panes.replace(old, new.clone()) {
                continue;
            }
            changed = true;
            if self.active == *old {
                self.active = new.clone();
            }
            if self.previous.as_ref() == Some(old) {
                self.previous = Some(new.clone());
            }
            if self.zoomed.as_ref() == Some(old) {
                self.zoomed = Some(new.clone());
            }
            if let Some(name) = self.names.remove(old) {
                self.names.insert(new.clone(), name);
            }
        }
        changed
    }
}

impl SheprdWindow {
    pub(super) fn remap_recovered_chats(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let focused = self
            .focused_core_chat(window, cx)
            .or_else(|| self.core_terminals.focused_pane(window, cx));
        let mut focus_rebound = None;
        let mappings: Vec<_> = self
            .core_terminals
            .hosts()
            .into_iter()
            .filter_map(|host| {
                let infos = self.core_terminals.infos_of(&host)?;
                let mapping = sheprd_core_client::chat_recovery::mapping(
                    infos.all().map(|info| (info.id.as_str(), &info.labels)),
                );
                (!mapping.is_empty()).then_some((host, mapping))
            })
            .collect();
        let Some(store) = cx.try_global::<Store>() else {
            return;
        };
        let tabs: Vec<_> = store
            .tabs()
            .filter_map(|tab| Some((tab.id, Host::of(&tab.scope), tab.location.clone()?)))
            .collect();
        let mut changed = false;
        for (tab, host, location) in tabs {
            let Some((_, mapping)) = mappings.iter().find(|(mapped, _)| *mapped == host) else {
                continue;
            };
            let location = if let Some(shown) = self.core_terminals.tabs.get_mut(&tab) {
                if !shown.remap(mapping) {
                    continue;
                }
                let location = shown.location();
                shown.saved = Some(location.clone());
                shown.unsaved = false;
                location
            } else {
                let Some(mut shown) = TabPanes::restore(host, &location) else {
                    continue;
                };
                if !shown.remap(mapping) {
                    continue;
                }
                shown.location()
            };
            Store::update(cx, |store| store.visited(tab, Some(location), None));
            if focused.as_ref().is_some_and(|(focused, terminal)| {
                *focused == tab && mapping.contains_key(terminal)
            }) {
                focus_rebound = Some(tab);
            }
            changed = true;
        }
        if changed {
            self.sync_core_views(cx);
        }
        if let Some(tab) = focus_rebound {
            self.focus_core_terminal(tab, window, cx);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
