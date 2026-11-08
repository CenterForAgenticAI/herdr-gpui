//! Which core a herdr-core tab's terminals run in: this computer's, or the
//! one on the saved host its workspace belongs to. Terminal IDs are only
//! unique within one core, so each host keeps its own hub and its own list
//! of what that core reports.

use super::{CoreTerminals, Error, Hub, Purpose, State, infos::Infos};
use crate::{
    SheprdWindow,
    browser::{Scope, TabId},
    controls::Command,
    i18n::fl,
    window::Flash,
};
use gpui::{Context, Window};
use sheprd_backend::{TerminalId, TerminalInfo};
use sheprd_core_client::CoreBackend;
use std::time::Instant;

/// The core a tab's terminals run in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Host {
    /// This computer's core.
    Local,
    /// The core on the saved host this scope names.
    Remote(Scope),
}

impl Host {
    /// The core a workspace in `scope` starts its terminals in.
    pub(crate) fn of(scope: &Scope) -> Self {
        if scope.is_local() {
            Self::Local
        } else {
            Self::Remote(scope.clone())
        }
    }
}

/// Another host's core and what it reports of its terminals.
pub(super) struct Remote {
    pub(super) hub: Hub,
    pub(super) infos: Infos,
}

impl CoreTerminals {
    /// The hub of `host`'s core, once the window has asked for it.
    pub(crate) fn hub_of(&self, host: &Host) -> Option<&Hub> {
        match host {
            Host::Local => Some(&self.hub),
            Host::Remote(scope) => self.remote.get(scope).map(|remote| &remote.hub),
        }
    }

    pub(super) fn hub_of_mut(&mut self, host: &Host) -> Option<&mut Hub> {
        match host {
            Host::Local => Some(&mut self.hub),
            Host::Remote(scope) => self.remote.get_mut(scope).map(|remote| &mut remote.hub),
        }
    }

    pub(super) fn backend_of(&self, host: &Host) -> Option<&CoreBackend> {
        self.hub_of(host)?.backend()
    }

    pub(super) fn infos_of(&self, host: &Host) -> Option<&Infos> {
        match host {
            Host::Local => Some(&self.infos),
            Host::Remote(scope) => self.remote.get(scope).map(|remote| &remote.infos),
        }
    }

    pub(super) fn infos_of_mut(&mut self, host: &Host) -> Option<&mut Infos> {
        match host {
            Host::Local => Some(&mut self.infos),
            Host::Remote(scope) => self.remote.get_mut(scope).map(|remote| &mut remote.infos),
        }
    }

    /// The core `tab`'s terminals run in; this computer's for a tab the
    /// window does not show.
    pub(crate) fn host_of(&self, tab: TabId) -> Host {
        self.tabs
            .get(&tab)
            .map_or(Host::Local, |shown| shown.host.clone())
    }

    /// What `tab`'s core reports of `terminal`.
    pub(crate) fn info(&self, tab: TabId, terminal: &TerminalId) -> Option<&TerminalInfo> {
        self.infos_of(&self.tabs.get(&tab)?.host)?.get(terminal)
    }

    /// Every host whose core the window knows of, this computer first.
    pub(super) fn hosts(&self) -> Vec<Host> {
        std::iter::once(Host::Local)
            .chain(self.remote.keys().cloned().map(Host::Remote))
            .collect()
    }

    /// Uses `hub` for the core of the host `scope` names, as the first ask
    /// for it would, so a test never dials a real host.
    #[cfg(test)]
    pub(crate) fn set_remote_hub_for_test(&mut self, scope: Scope, hub: Hub) {
        let infos = Infos::default();
        self.remote.insert(scope, Remote { hub, infos });
    }

    /// Reads what each core reports while any of its tabs is known, for
    /// split chrome and the sidebar's rows. True when what is known changed.
    pub(super) fn poll_infos(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for host in self.hosts() {
            let listed = self.tabs.values().any(|shown| shown.host == host);
            let backend = listed.then(|| self.backend_of(&host).cloned()).flatten();
            if let Some(infos) = self.infos_of_mut(&host) {
                changed |= infos.poll(backend.as_ref(), now);
            }
        }
        changed
    }

    /// Whether `host` was found to have no herdr-core installed, so new
    /// tabs there stay with its Herdr.
    pub(crate) fn lacks_core(&self, host: &Host) -> bool {
        matches!(
            self.hub_of(host).map(Hub::state),
            Some(State::Unavailable(error)) if error.core_missing()
        )
    }
}

impl SheprdWindow {
    /// Takes each hub's outcome. A lost connection ended the workers of its
    /// host's panes: their views go, so the next sync joins that core again
    /// and attaches each terminal anew; nothing is started or replayed. A
    /// pane that held the keyboard has its tab take it back once the new
    /// view is there.
    pub(super) fn poll_core_hubs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        for host in self.core_terminals.hosts() {
            let Some(hub) = self.core_terminals.hub_of_mut(&host) else {
                continue;
            };
            let connected = hub.backend().is_some();
            if !hub.poll() {
                continue;
            }
            changed = true;
            if connected && self.core_terminals.backend_of(&host).is_none() {
                if let Some((tab, _)) = self.core_terminals.focused_pane(window, cx)
                    && self.core_terminals.host_of(tab) == host
                {
                    self.core_terminals.focus_wanted = Some(tab);
                }
                let terminals = &mut self.core_terminals;
                let tabs = &terminals.tabs;
                terminals
                    .views
                    .retain(|(tab, _), _| tabs.get(tab).is_none_or(|shown| shown.host != host));
            }
            let failed = match self.core_terminals.hub_of(&host).map(Hub::state) {
                Some(State::Unavailable(error)) => Some(std::sync::Arc::clone(error)),
                _ => None,
            };
            let wanted_here = self
                .core_terminals
                .wanted
                .as_ref()
                .is_some_and(|(purpose, _)| self.purpose_host(purpose) == host);
            if let Some(error) = failed
                && wanted_here
                && let Some((purpose, _)) = self.core_terminals.wanted.take()
            {
                self.core_unavailable(purpose, &error, window, cx);
            }
        }
        if changed {
            self.start_core_shell();
            cx.notify();
        }
    }

    /// Tells the user why a terminal wanted for `purpose` did not start. A
    /// new tab on a host without herdr-core is made by the host's Herdr
    /// instead, as it was before Herdr 2.
    fn core_unavailable(
        &mut self,
        purpose: Purpose,
        error: &Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let still_there = |view: &Self, scope: &Scope| {
            view.browser_key()
                .is_some_and(|(focused, _)| focused == *scope)
        };
        if error.core_missing()
            && let Purpose::Tab { scope, .. } = &purpose
            && still_there(self, scope)
        {
            self.command(Command::Tab, window, cx);
            self.show_flash(Flash::hint(fl!("core-terminals-host-without-core")), cx);
            return;
        }
        let reason = error.to_string();
        self.show_flash(
            Flash::warning(fl!("core-terminals-unavailable", reason = reason)),
            cx,
        );
    }

    /// The hub of `host`'s core, made the first time a remote host is asked
    /// for; `None` for a host that is no longer a saved device.
    pub(super) fn core_hub(&mut self, host: &Host) -> Option<&mut Hub> {
        if let Host::Remote(scope) = host
            && !self.core_terminals.remote.contains_key(scope)
        {
            let target = self
                .endpoints
                .iter()
                .find(|endpoint| crate::browser::scope(endpoint) == *scope)?
                .connection
                .target
                .clone();
            self.core_terminals.remote.insert(
                scope.clone(),
                Remote {
                    hub: Hub::remote(target),
                    infos: Infos::default(),
                },
            );
        }
        self.core_terminals.hub_of_mut(host)
    }
}
