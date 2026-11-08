//! The state mods read, built from what the window already holds: each
//! listed host's sessions, Herdr 1's and on this computer Herdr 2's. Usage and the pull request are not offered yet,
//! so those topics read as empty.

use crate::{SheprdWindow, state::ConnectionStatus};
use sheprd_client::protocol::AgentStatus as WireStatus;
use sheprd_mods::{Agent, AgentStatus, Focus, Host, State, Tab, Workspace};

impl SheprdWindow {
    pub(crate) fn mods_state(&self) -> State {
        let mut state = State::default();
        for (index, endpoint) in self.endpoints.iter().enumerate() {
            if !endpoint.enabled {
                continue;
            }
            let live = if index == self.selected_endpoint {
                &self.live
            } else {
                &endpoint.live
            };
            state.hosts.push(Host {
                id: endpoint.id.clone(),
                name: endpoint.label.clone(),
                connected: matches!(live.status, ConnectionStatus::Connected),
            });
            // Herdr 1's workspaces, then those the app keeps for Herdr 2.
            let sessions = self.sessions(index);
            let selected = index == self.selected_endpoint;
            let front = selected
                .then(|| self.browser_key())
                .flatten()
                .map(|(_, workspace)| workspace);
            for workspace in sessions.workspaces() {
                state.workspaces.push(Workspace {
                    id: workspace.id.clone(),
                    host: endpoint.id.clone(),
                    name: workspace.label.clone(),
                    focused: front.as_deref() == Some(workspace.id.as_str()),
                    tabs: workspace
                        .tabs
                        .iter()
                        .map(|tab| Tab {
                            id: tab.id.clone(),
                            name: tab.label.clone(),
                            focused: tab.focused,
                        })
                        .collect(),
                });
            }
            state.agents.extend(sessions.agents().map(|agent| Agent {
                id: agent.pane.clone(),
                host: endpoint.id.clone(),
                workspace: agent.workspace.clone(),
                name: agent.name.clone(),
                status: status(agent.status),
            }));
            if selected {
                let workspace = front.as_deref().and_then(|id| sessions.workspace(id));
                // A kept workspace's tabs are the app's; Herdr 1 names its own.
                let tab = live
                    .snapshot
                    .as_ref()
                    .filter(|_| self.kept_in_front().is_none())
                    .and_then(|snapshot| snapshot.focused_tab_id.clone());
                state.focus = Some(Focus {
                    host: endpoint.id.clone(),
                    workspace: front.clone(),
                    tab,
                    name: workspace.map(|workspace| workspace.label.clone()),
                    path: workspace
                        .map(|workspace| workspace.cwd.clone())
                        .filter(|path| !path.is_empty()),
                });
            }
        }
        state
    }
}

fn status(status: WireStatus) -> AgentStatus {
    match status {
        WireStatus::Working => AgentStatus::Working,
        WireStatus::Blocked => AgentStatus::Blocked,
        WireStatus::Done => AgentStatus::Done,
        WireStatus::Idle | WireStatus::Unknown => AgentStatus::Idle,
    }
}
