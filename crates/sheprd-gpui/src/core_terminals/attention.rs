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
    collections::{HashMap, VecDeque},
    time::Instant,
};

/// The most changes waiting out the delay; older ones give way.
const MAX_DUE: usize = 32;

/// What each herdr-core agent last reported, app-wide: every window lists the
/// same core, so a change is news once, in whichever window hears it first.
#[derive(Default)]
pub(crate) struct Heard(HashMap<TerminalId, AgentStatus>);

impl Global for Heard {}

impl Heard {
    /// Takes the status of each `listed` terminal and returns what changed
    /// into news. Terminals the core no longer has, by `known`, are
    /// forgotten.
    pub(super) fn hear<'a>(
        &mut self,
        listed: impl IntoIterator<Item = (&'a TerminalId, AgentStatus)>,
        known: impl Fn(&TerminalId) -> bool,
    ) -> Vec<(TerminalId, News)> {
        self.0.retain(|terminal, _| known(terminal));
        listed
            .into_iter()
            .filter_map(|(terminal, status)| {
                let before = self.0.insert(terminal.clone(), status);
                News::of(before, status).map(|news| (terminal.clone(), news))
            })
            .collect()
    }
}

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

/// The window's news waiting out the delay.
#[derive(Default)]
pub(super) struct Attention {
    due: VecDeque<(Instant, TerminalId, News)>,
}

impl Attention {
    fn push(&mut self, heard: Instant, terminal: TerminalId, news: News) {
        self.due.retain(|(_, waiting, _)| *waiting != terminal);
        if self.due.len() == MAX_DUE {
            self.due.pop_front();
        }
        self.due.push_back((heard, terminal, news));
    }
}

impl SheprdWindow {
    /// Hears what changed in the latest list of the window's terminals.
    /// Call when the list changed.
    pub(super) fn hear_core_agents(&mut self, cx: &mut Context<Self>) {
        let terminals = &self.core_terminals;
        let listed: Vec<(&TerminalId, AgentStatus)> = terminals
            .chat_terminals()
            .map(|info| (&info.id, info.agent))
            .collect();
        let news = cx
            .default_global::<Heard>()
            .hear(listed, |terminal| terminals.infos.get(terminal).is_some());
        let now = Instant::now();
        for (terminal, news) in news {
            self.core_terminals.attention.push(now, terminal, news);
        }
    }

    /// Plays the news whose delay is over and still holds, and counts the
    /// agents that need the user on the badge. Runs every tick.
    pub(super) fn tell_core_agents(&mut self, window: &Window, cx: &mut Context<Self>) {
        #[cfg(any(target_os = "macos", windows))]
        {
            // Counted as a Herdr 1 agent is: blocked, or done.
            let attention: std::collections::HashSet<TerminalId> = self
                .core_terminals
                .chat_terminals()
                .filter(|info| matches!(info.agent, AgentStatus::Blocked | AgentStatus::Done))
                .map(|info| info.id.clone())
                .collect();
            crate::app_badge::sync_core(window.window_handle().window_id(), attention, cx);
        }
        if self.core_terminals.attention.due.is_empty() {
            return;
        }
        // Until the settings load, the default delay of a second.
        let delay = std::time::Duration::from_secs(self.sound.delay().unwrap_or(1));
        let now = Instant::now();
        let in_front = window
            .is_window_active()
            .then(|| self.core_terminals.focused_pane(window, cx))
            .flatten()
            .map(|(tab, _)| tab);
        let due = std::mem::take(&mut self.core_terminals.attention.due);
        for (heard, terminal, news) in due {
            if now < heard + delay {
                self.core_terminals
                    .attention
                    .due
                    .push_back((heard, terminal, news));
                continue;
            }
            let Some(info) = self.core_terminals.infos.get(&terminal) else {
                continue;
            };
            if !news.holds(info.agent) {
                continue;
            }
            let shown = in_front.is_some() && in_front == self.core_terminals.tab_of(&terminal);
            if news == News::Finished && shown {
                continue;
            }
            let agent = sheprd_core_client::agent_kind(info);
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
