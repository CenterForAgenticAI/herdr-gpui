//! Where a focus request is aimed, and what a click on an agents-list row
//! means: a closed set of targets and outcomes, so navigation is never a
//! boolean-plus-ID pair or a string tag.

use crate::{SheprdWindow, browser::TabId};
use gpui::Context;
use sheprd_backend::TerminalId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NavigationTarget<T> {
    Workspace(T),
    Tab(T),
    Pane(T),
}

pub(crate) type OwnedNavigationTarget = NavigationTarget<String>;

impl<T: AsRef<str>> NavigationTarget<T> {
    /// Borrow the payload, as `Option::as_deref` does. Not `AsRef`, which
    /// returns a reference to the same value rather than a new target.
    pub(crate) fn as_deref(&self) -> NavigationTarget<&str> {
        match self {
            Self::Workspace(id) => NavigationTarget::Workspace(id.as_ref()),
            Self::Tab(id) => NavigationTarget::Tab(id.as_ref()),
            Self::Pane(id) => NavigationTarget::Pane(id.as_ref()),
        }
    }
}

/// Take ownership of a borrowed target. `ToOwned` is not usable here: its
/// blanket `Clone` impl already covers this type and would hand back a
/// borrowing target instead of an owning one.
impl<T: AsRef<str>> From<&NavigationTarget<T>> for OwnedNavigationTarget {
    fn from(target: &NavigationTarget<T>) -> Self {
        match target.as_deref() {
            NavigationTarget::Workspace(id) => Self::Workspace(id.to_owned()),
            NavigationTarget::Tab(id) => Self::Tab(id.to_owned()),
            NavigationTarget::Pane(id) => Self::Pane(id.to_owned()),
        }
    }
}

/// What a click on an agents-list row means now. `Navigate` is today's path
/// unchanged; `Rescroll` names the pane already on screen, which instead
/// goes to its live bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentRowAction {
    Navigate,
    Rescroll(RescrollTarget),
}

/// Where the pane a row names lives, which decides who scrolls it to the
/// bottom. Each variant carries what its sender needs, so a target can
/// always be performed and never needs a parallel parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RescrollTarget {
    /// A Herdr 1 pane: the daemon scrolls it (`pane.scroll`, offset 0).
    HerdrPane { pane: String },
    /// The chat the app draws over a Herdr 1 pane: the transcript view
    /// follows its end, client-locally.
    Chat { pane: String },
    /// A herdr-core terminal: its view scrolls its own history (offset 0).
    /// No sidebar agent row names one today; the devices overview's agent
    /// rows do, and take this arm when they adopt the resolver.
    // Callers come with the devices overview's agent rows adopting the
    // resolver.
    #[cfg_attr(not(test), allow(dead_code))]
    CoreTerminal { tab: TabId, terminal: TerminalId },
}

impl SheprdWindow {
    /// What clicking the agents-list row of `pane` on `endpoint` means now:
    /// open it, or, when it is already the pane on screen, scroll it to the
    /// live bottom. Pure over prepared state — reads the snapshot, the
    /// chats, and readiness; writes nothing — so tests assert the decision
    /// without a daemon.
    pub(crate) fn agent_row_action(&self, endpoint: &str, pane: &str) -> AgentRowAction {
        if !self.herdr_pane_shown(endpoint, pane) {
            return AgentRowAction::Navigate;
        }
        if self.chats.shown(pane) {
            AgentRowAction::Rescroll(RescrollTarget::Chat {
                pane: pane.to_owned(),
            })
        } else {
            AgentRowAction::Rescroll(RescrollTarget::HerdrPane {
                pane: pane.to_owned(),
            })
        }
    }

    /// Resolves a row click and performs it. The one call every
    /// agents-list row site makes; returns as `navigate_endpoint` does,
    /// false only when an open page refuses the click.
    pub(crate) fn open_agent_row(
        &mut self,
        endpoint: &str,
        pane: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.page.is_some() {
            return false;
        }
        match self.agent_row_action(endpoint, pane) {
            AgentRowAction::Navigate => {
                self.navigate_endpoint(endpoint, NavigationTarget::Pane(pane), cx)
            }
            AgentRowAction::Rescroll(target) => {
                // The row click supersedes whatever an earlier click queued,
                // as `navigate_endpoint` clears before it navigates. The
                // toast slot cannot be held here: the classifier only
                // rescrolls an input-ready window.
                self.pending_navigation = None;
                self.scroll_to_bottom(target, cx);
                true
            }
        }
    }

    /// Scrolls the pane `target` names to its live bottom, by the sender of
    /// the Herdr that runs it. Total over the enum; every arm no-ops safely
    /// when its pane, chat, or view is gone. Idempotent: the target state is
    /// "at the bottom", so a second dispatch is harmless.
    pub(crate) fn scroll_to_bottom(&mut self, target: RescrollTarget, cx: &mut Context<Self>) {
        match target {
            RescrollTarget::HerdrPane { pane } => {
                let Some(boot) = self
                    .live
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.boot_id.clone())
                else {
                    return;
                };
                self.scroll_pane_to_bottom(&boot, &pane, cx);
            }
            RescrollTarget::Chat { pane } => self.jump_chat_to_end(&pane, cx),
            RescrollTarget::CoreTerminal { tab, .. } => {
                let Some(view) = self.core_pane_view(tab).cloned() else {
                    return;
                };
                view.update(cx, |view, _| view.scroll_history(0));
            }
        }
    }

    /// Whether `pane` of `endpoint` is the pane on screen now: the row's own
    /// host is selected, its live snapshot focuses the pane, and nothing
    /// covers or defers it. The daemon's pane-level focus is the one truth,
    /// not the row's `agent.focused` styling flag.
    fn herdr_pane_shown(&self, endpoint: &str, pane: &str) -> bool {
        self.endpoints
            .get(self.selected_endpoint)
            .is_some_and(|selected| selected.id == endpoint)
            // A kept herdr-core workspace in front makes the click a real
            // navigation, whose dispatch leaves the workspace.
            && self.kept_in_front().is_none()
            // An unready connection keeps today's queueing.
            && self.input_ready()
            && self
                .live
                .snapshot
                .as_deref()
                .is_some_and(|snapshot| snapshot.focused_pane_id.as_deref() == Some(pane))
    }
}
