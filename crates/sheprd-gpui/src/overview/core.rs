//! Herdr 2 agents on the Overview: the agents in the window's herdr-core
//! tabs, listed with this computer's Herdr 1 agents. herdr-core has no
//! workspaces or snapshot, so a row names its agent by identity
//! (`sheprd_core_client::agent_kind`), its checkout by folder, and opens the
//! tab and pane that hold it.

use super::{Key, Open, Row, State, model::Since};
use crate::SheprdWindow;
use sheprd_backend::{AgentStatus, TerminalInfo};
use std::time::SystemTime;

/// What herdr-core agents are keyed by where a host's endpoint id would be:
/// pane ids of one Herdr and terminal ids of the other never meet.
pub(crate) const ENDPOINT: &str = "herdr-core";

/// Untrusted titles are cut here, as a Herdr 1 row's are.
const TITLE_LIMIT: usize = 200;

/// The agent's name: the one it is identified as, else the one it reports
/// for itself. A program that reports no status and is no known agent is
/// not one.
fn agent_name(info: &TerminalInfo) -> Option<String> {
    if let Some(kind) = sheprd_core_client::agent_kind(info) {
        return Some(kind.name().to_owned());
    }
    let reported = info.reporter.as_deref()?;
    let name = crate::notifications::safe_text(reported, 32);
    let name = name.trim();
    (!name.is_empty() && info.agent != AgentStatus::Unknown).then(|| name.to_owned())
}

impl SheprdWindow {
    /// The endpoint and label of this computer, which the core runs on.
    fn core_overview_host(&self) -> (String, String) {
        self.endpoints
            .iter()
            .find(|endpoint| !endpoint.connection.target.is_remote())
            .map(|endpoint| (endpoint.id.clone(), endpoint.label.clone()))
            .unwrap_or_else(|| (ENDPOINT.to_owned(), crate::i18n::fl!("overview-core-host")))
    }

    /// Whether `id` is the endpoint the core's rows are listed with.
    pub(super) fn is_core_overview_host(&self, id: &str) -> bool {
        self.core_overview_host().0 == id
    }

    /// The rows of the agents in the window's herdr-core tabs.
    pub(super) fn core_overview_rows(&self, since: &Since) -> Vec<Row> {
        let (endpoint, host) = self.core_overview_host();
        self.core_terminals
            .chat_terminals()
            .filter_map(|info| {
                let name = agent_name(info)?;
                let tab = self.core_terminals.tab_of(&info.id)?;
                let folder = info.cwd.as_deref();
                let workspace = folder
                    .and_then(|cwd| cwd.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let checkout =
                    folder.map_or_else(|| info.id.to_string(), |cwd| cwd.display().to_string());
                let title = info
                    .title
                    .as_deref()
                    .map(|title| crate::notifications::safe_text(title, TITLE_LIMIT))
                    .map(|title| title.trim().to_owned())
                    .filter(|title| !title.is_empty() && *title != name)
                    .unwrap_or_else(|| name.clone());
                Some(Row {
                    key: Key::Agent {
                        endpoint: ENDPOINT.to_owned(),
                        checkout,
                        agent: name.clone(),
                    },
                    state: State::of(info.agent),
                    title,
                    detail: name.clone(),
                    identity: Some(name),
                    workspace,
                    branch: None,
                    host: host.clone(),
                    endpoint: endpoint.clone(),
                    since: since.get(ENDPOINT, info.id.as_str()),
                    pr: None,
                    open: Open::Core {
                        tab,
                        terminal: info.id.clone(),
                    },
                    muted: false,
                })
            })
            .collect()
    }

    /// Notes when each core agent entered its state, and the terminals
    /// listed, so agents gone from the core are forgotten.
    pub(super) fn observe_core_overview(&mut self, now: SystemTime) -> Vec<String> {
        let mut listed = Vec::new();
        for info in self.core_terminals.chat_terminals() {
            self.overview
                .since
                .observe_one(ENDPOINT, info.id.as_str(), info.agent, now);
            listed.push(info.id.to_string());
        }
        listed
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
