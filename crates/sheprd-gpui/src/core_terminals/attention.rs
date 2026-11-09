//! Herdr 2 agents asking for the user. herdr-core sends no notifications, only
//! each program's status, so the app hears the changes itself: an agent that
//! becomes blocked plays the request sound, one whose turn ends plays the done
//! sound, and either counts on the Dock or taskbar badge. Herdr 1 decides this
//! in its daemon; the sound settings, their delay, and the rule that a finished
//! turn in the tab in front stays quiet are the same.

use crate::SheprdWindow;
use gpui::{prelude::*, *};
use sheprd_backend::{AgentStatus, TerminalId};
use sheprd_client::protocol::SemanticNotificationSound;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

/// A change worth a sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum News {
    /// The agent waits on the user: a permission, a question, an error.
    NeedsYou,
    /// The agent's turn ended.
    Finished,
}

impl News {
    /// What a change from `before` to `now` tells the user. A terminal heard
    /// of for the first time tells nothing, so opening the app is quiet.
    pub(super) fn of(before: Option<AgentStatus>, now: AgentStatus) -> Option<Self> {
        match (before?, now) {
            (AgentStatus::Blocked, AgentStatus::Blocked) => None,
            (_, AgentStatus::Blocked) => Some(Self::NeedsYou),
            (AgentStatus::Working, AgentStatus::Done | AgentStatus::Idle) => Some(Self::Finished),
            _ => None,
        }
    }

    /// Whether `status` still says so once the delay is over.
    pub(super) fn holds(self, status: AgentStatus) -> bool {
        match self {
            Self::NeedsYou => status == AgentStatus::Blocked,
            Self::Finished => matches!(status, AgentStatus::Done | AgentStatus::Idle),
        }
    }

    fn sound(self) -> SemanticNotificationSound {
        match self {
            Self::NeedsYou => SemanticNotificationSound::Request,
            Self::Finished => SemanticNotificationSound::Done,
        }
    }
}

/// The most changes waiting out the delay; older ones give way.
const MAX_DUE: usize = 32;
/// How long past its delay a change may still play: one no window could play
/// in time, as every window lost the core, is old news.
const STALE: Duration = Duration::from_secs(5);

/// What each herdr-core agent last reported, and the changes waiting out the
/// delay, app-wide. Every window lists this computer's core on its own, so a
/// change is news once, in whichever window hears it first, and any window
/// still connected plays it.
#[derive(Default)]
pub(crate) struct Heard {
    status: HashMap<TerminalId, AgentStatus>,
    /// When the newest list taken was asked for. Windows read the core on
    /// their own, so an older list can arrive after a newer one.
    newest: Option<Instant>,
    due: VecDeque<(Instant, TerminalId, News)>,
    /// The terminals each window shows while it is the active one.
    front: HashMap<WindowId, HashSet<TerminalId>>,
}

impl Global for Heard {}

impl Heard {
    /// Takes the status of each `listed` terminal, from a list asked for at
    /// `read`, and returns what changed into news. Terminals the core no
    /// longer has, by `known`, are forgotten. A list older than one already
    /// taken tells nothing.
    pub(super) fn hear<'a>(
        &mut self,
        read: Instant,
        listed: impl IntoIterator<Item = (&'a TerminalId, AgentStatus)>,
        known: impl Fn(&TerminalId) -> bool,
    ) -> Vec<(TerminalId, News)> {
        if self.newest.is_some_and(|newest| read < newest) {
            return Vec::new();
        }
        self.newest = Some(read);
        self.status.retain(|terminal, _| known(terminal));
        listed
            .into_iter()
            .filter_map(|(terminal, status)| {
                let before = self.status.insert(terminal.clone(), status);
                News::of(before, status).map(|news| (terminal.clone(), news))
            })
            .collect()
    }

    /// Queues `news` heard at `heard`, replacing what waited for the same
    /// terminal.
    pub(super) fn push(&mut self, heard: Instant, terminal: TerminalId, news: News) {
        self.due.retain(|(_, waiting, _)| *waiting != terminal);
        if self.due.len() == MAX_DUE {
            self.due.pop_front();
        }
        self.due.push_back((heard, terminal, news));
    }

    /// Records the terminals `window` shows in front: none unless it is
    /// active. Windows no longer `open` are forgotten.
    pub(super) fn show(
        &mut self,
        window: WindowId,
        shown: HashSet<TerminalId>,
        open: impl Fn(WindowId) -> bool,
    ) {
        self.front.retain(|id, _| open(*id));
        if shown.is_empty() {
            self.front.remove(&window);
        } else {
            self.front.insert(window, shown);
        }
    }

    /// Takes the news to play at `now`: past `delay`, still true of the
    /// latest status, and, for a finished turn, not in front of the user.
    /// What is still waiting stays; what went stale or untrue is dropped.
    pub(super) fn take_due(&mut self, now: Instant, delay: Duration) -> Vec<(TerminalId, News)> {
        let mut play = Vec::new();
        for (heard, terminal, news) in std::mem::take(&mut self.due) {
            if now < heard + delay {
                self.due.push_back((heard, terminal, news));
                continue;
            }
            let holds = self
                .status
                .get(&terminal)
                .is_some_and(|status| news.holds(*status));
            let seen = news == News::Finished
                && self.front.values().any(|shown| shown.contains(&terminal));
            if holds && !seen && now <= heard + delay + STALE {
                play.push((terminal, news));
            }
        }
        play
    }
}

impl SheprdWindow {
    /// Hears what changed in the latest list of the window's terminals.
    /// Call when the list changed.
    pub(super) fn hear_core_agents(&mut self, cx: &mut Context<Self>) {
        let terminals = &self.core_terminals;
        let Some(read) = terminals.infos.read_at() else {
            return;
        };
        let listed: Vec<(&TerminalId, AgentStatus)> = terminals
            .chat_terminals()
            .map(|info| (&info.id, info.agent))
            .collect();
        let heard = cx.default_global::<Heard>();
        let news = heard.hear(read, listed, |terminal| {
            terminals.infos.get(terminal).is_some()
        });
        let now = Instant::now();
        for (terminal, news) in news {
            heard.push(now, terminal, news);
        }
    }

    /// Plays the news whose delay is over and still holds, and counts the
    /// agents that need the user on the badge. Runs every tick, after
    /// `core_terminals.shown` is current. A window that lost the core
    /// counts, shows, and plays nothing, as what it last heard is stale.
    pub(super) fn tell_core_agents(&mut self, window: &Window, cx: &mut Context<Self>) {
        let id = window.window_handle().window_id();
        let connected = self.core_terminals.hub.backend().is_some();
        #[cfg(any(target_os = "macos", windows))]
        {
            // Counted as a Herdr 1 agent is: blocked, or done.
            let attention: HashSet<TerminalId> = self
                .core_terminals
                .chat_terminals()
                .filter(|_| connected)
                .filter(|info| matches!(info.agent, AgentStatus::Blocked | AgentStatus::Done))
                .map(|info| info.id.clone())
                .collect();
            crate::app_badge::sync_core(id, attention, cx);
        }
        // The tab the active window shows, a chat's included, whatever
        // holds the keyboard.
        let shown: HashSet<TerminalId> = self
            .core_terminals
            .shown
            .as_ref()
            .filter(|_| connected && window.is_window_active())
            .and_then(|(tab, _)| self.core_terminals.tabs.get(tab))
            .filter(|shown| shown.holds_chats())
            .map(|shown| shown.panes.terminals().into_iter().cloned().collect())
            .unwrap_or_default();
        let open: HashSet<WindowId> = cx
            .windows()
            .iter()
            .map(|handle| handle.window_id())
            .collect();
        let heard = cx.default_global::<Heard>();
        heard.show(id, shown, |window| open.contains(&window));
        if !connected || heard.due.is_empty() {
            return;
        }
        // Until the settings load, the default delay of a second.
        let delay = Duration::from_secs(self.sound.delay().unwrap_or(1));
        for (terminal, news) in heard.take_due(Instant::now(), delay) {
            let agent = self
                .core_terminals
                .infos
                .get(&terminal)
                .and_then(sheprd_core_client::agent_kind);
            self.sound.play_agent(
                news.sound(),
                agent.map(|agent| agent.name()),
                terminal.as_str(),
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
