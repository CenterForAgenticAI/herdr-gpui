//! What a host's own layout draws in place of these regions: the sidebar,
//! activity bar and status bar's data as plain JSON, and the actions they send
//! back. It reads the same state the regions do, and goes through the same
//! commands, so a host's row behaves as the window's own.

use super::{
    ActivityPanel,
    workspaces::{workspace_badge, workspace_entries, workspace_label},
};
use crate::{
    controls::Command, navigation::NavigationTarget, system_load::Sample, window::SheprdWindow,
};
use gpui::{App, Context, Window};
use serde_json::{Value, json};
use sheprd_client::protocol::{
    AgentStatus, ClientShellAgent, ClientShellSnapshot, ClientShellWorkspace,
};
use std::sync::Arc;

/// Backgrounds below this relative luminance, about mid gray, take light text.
const DARK_LUMINANCE: f32 = 0.2;

/// What a host's layout asks of the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Runs a command by its name in the catalog, as a shortcut does.
    Command(String),
    /// Focuses the pane with this id.
    FocusPane(String),
    /// Focuses the workspace with this id.
    FocusWorkspace(String),
    /// Shows the activity panel with this id, as its button does.
    Panel(String),
}

/// The window's state for a host's layout, in the parts that change apart, so
/// a host can run only what reads the part that changed. Plain data: it holds
/// nothing a mod could not show.
#[derive(Clone, Debug, PartialEq)]
pub struct HostedState {
    /// The machine's load, which moves every few seconds.
    pub load: Value,
    /// Workspaces, agents, panels and the connection.
    pub main: Value,
    /// The colors and font, for a host's own widgets to follow.
    pub palette: Palette,
    /// The colors, which move only with the theme.
    pub theme: Value,
}

/// Sheprd's theme and UI font, for a host's own widgets, so what it draws
/// around the window reads as part of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub accent: u32,
    pub active: u32,
    pub background: u32,
    pub cursor: u32,
    pub danger: u32,
    pub dark: bool,
    pub font_family: String,
    pub font_size: f32,
    pub foreground: u32,
    pub info: u32,
    pub muted: u32,
    pub sidebar: u32,
    /// The sidebar's own font, which the rows use rather than the UI's.
    pub sidebar_font_family: String,
    pub sidebar_font_size: f32,
    pub success: u32,
    pub surface: u32,
    pub warning: u32,
}

/// What a host's state is built from that the window itself changes. Cheap to
/// read on every notification, which terminal output sends once a frame; equal
/// stamps say the state need not be built. What changes without the window
/// noticing, such as the machine's load, is not in it.
#[derive(Clone)]
pub struct Stamp {
    connected: bool,
    panel: ActivityPanel,
    section: bool,
    sidebar_visible: bool,
    snapshot: Option<Arc<ClientShellSnapshot>>,
    status_bar_visible: bool,
    theme: u32,
}

impl PartialEq for Stamp {
    fn eq(&self, other: &Self) -> bool {
        let same_snapshot = match (&self.snapshot, &other.snapshot) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        };
        same_snapshot
            && self.connected == other.connected
            && self.panel == other.panel
            && self.section == other.section
            && self.sidebar_visible == other.sidebar_visible
            && self.status_bar_visible == other.status_bar_visible
            && self.theme == other.theme
    }
}

fn status_name(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Idle => "idle",
        AgentStatus::Unknown => "unknown",
        AgentStatus::Working => "working",
    }
}

fn panel_id(panel: ActivityPanel) -> &'static str {
    match panel {
        ActivityPanel::Automations => "automations",
        ActivityPanel::Issues => "issues",
        ActivityPanel::Overview => "overview",
        ActivityPanel::PullRequests => "pull_requests",
        ActivityPanel::Security => "security",
        ActivityPanel::Usage => "usage",
        ActivityPanel::Workspaces => "workspaces",
    }
}

fn agent_state(agent: &ClientShellAgent) -> Value {
    json!({
        "name": agent.name.as_deref().or(agent.display_agent.as_deref()).or(agent.agent.as_deref()),
        "pane": agent.pane_id,
        "status": status_name(agent.agent_status),
        "title": agent.title,
        "workspace": agent.workspace_id,
    })
}

/// `color` as `#rrggbb`, how a mod's style names one.
fn hex(color: u32) -> String {
    format!("#{:06x}", color & 0xff_ffff)
}

/// Whether `color` is a dark background.
fn is_dark(color: u32) -> bool {
    crate::contrast::luminance(color) < DARK_LUMINANCE
}

fn load_state(sample: Option<&Sample>) -> Value {
    json!({
        "cpu": sample.and_then(|sample| sample.cpu),
        "memory": sample.and_then(|sample| sample.memory).map(|memory| memory.percent()),
    })
}

impl SheprdWindow {
    fn hosted_workspace(&self, workspace: &ClientShellWorkspace, child: bool) -> Value {
        let endpoint = self.endpoints.get(self.selected_endpoint);
        let pr_view = self.endpoint_pr_view(self.selected_endpoint);
        let badge = endpoint.and_then(|endpoint| {
            workspace_badge(
                workspace,
                pr_view.as_ref().map(|view| (&self.menu.pr_cache, view)),
                (Some(&self.git), self.config.sidebar_style.change_counts),
                (&self.teleport_marks, &endpoint.id),
                (None, false),
                &self.theme,
            )
        });
        let (ahead, behind) = workspace.git_ahead_behind.unwrap_or((0, 0));
        let tree = workspace.worktree.as_ref();
        let counts = badge.as_ref().and_then(|badge| badge.counts.as_ref());
        json!({
            "added": counts.map(|counts| counts.additions.clone()),
            "agent_status": status_name(workspace.agent_status),
            "ahead": ahead,
            "behind": behind,
            "branch": workspace.branch,
            "child": child,
            "dirty": badge.as_ref().is_some_and(|badge| badge.dirty),
            "focused": workspace.focused,
            "id": workspace.workspace_id,
            "label": workspace_label(workspace, child),
            "linked": tree.is_some_and(|tree| tree.is_linked_worktree),
            "number": workspace.number,
            "pr": badge.as_ref().and_then(|badge| badge.pr.as_ref()).map(|pr| pr.number.clone()),
            "removed": counts.map(|counts| counts.deletions.clone()),
            "repo": tree.map(|tree| tree.label.clone()),
            "repo_key": tree.map(|tree| tree.key.clone()),
        })
    }

    /// Whether a host's state may have changed; see [`Stamp`].
    pub(crate) fn hosted_stamp(&self) -> Stamp {
        Stamp {
            connected: self.live.status.is_connected(),
            panel: self.activity_panel(),
            section: self.activity_section().is_some(),
            sidebar_visible: self.sidebar_visible,
            snapshot: self.live.snapshot.clone(),
            status_bar_visible: self.status_bar_visible,
            theme: self.theme.background,
        }
    }

    /// The window's state for a host's layout.
    pub(crate) fn hosted_state(&self, cx: &App) -> HostedState {
        let counts = self.activity_counts(cx);
        let snapshot = self.live.snapshot.as_deref();
        let panels: Vec<Value> = ActivityPanel::ALL
            .iter()
            .copied()
            .filter(|panel| self.activity_panel_listed(*panel))
            .map(|panel| {
                json!({
                    "count": counts.of(panel).filter(|count| *count > 0),
                    "id": panel_id(panel),
                    "label": super::activity::label(panel).to_string(),
                })
            })
            .collect();
        let reading = self.selected_host().and_then(|host| {
            self.system_load
                .get(&host)
                .and_then(|load| load.latest().copied())
        });
        let theme = &self.theme;
        HostedState {
            load: load_state(reading.as_ref()),
            palette: Palette {
                accent: theme.primary(),
                active: theme.active,
                background: theme.background,
                cursor: theme.cursor,
                danger: theme.palette[1],
                dark: is_dark(theme.background),
                font_family: self.config.ui.family.clone(),
                font_size: self.config.ui.size,
                foreground: theme.foreground,
                info: theme.palette[6],
                muted: theme.muted,
                sidebar: theme.sidebar_background(),
                sidebar_font_family: self.config.sidebar.family.clone(),
                sidebar_font_size: self.config.sidebar.size,
                success: theme.palette[2],
                surface: theme.surface,
                warning: theme.palette[3],
            },
            main: json!({
                "agents": snapshot.map_or(Vec::new(), |snapshot| {
                    snapshot.agents.iter().map(agent_state).collect()
                }),
                "connected": self.live.status.is_connected(),
                "panel": panel_id(self.activity_panel()),
                "panels": panels,
                "section": self.activity_section().is_some(),
                "sidebar_visible": self.sidebar_visible,
                "status": self.live.status_text(self.local_error.as_deref()),
                "status_bar_visible": self.status_bar_visible,
                "workspaces": snapshot.map_or(Vec::new(), |snapshot| {
                    workspace_entries(&snapshot.workspaces)
                        .into_iter()
                        .map(|(index, child)| self.hosted_workspace(&snapshot.workspaces[index], child))
                        .collect()
                }),
            }),
            theme: json!({
                "accent": hex(self.theme.primary()),
                "active": hex(self.theme.active),
                "background": hex(self.theme.background),
                "danger": hex(self.theme.palette[1]),
                "foreground": hex(self.theme.foreground),
                "muted": hex(self.theme.muted),
                "sidebar": hex(self.theme.sidebar_background()),
                "subtext": hex(self.theme.subtext()),
                "success": hex(self.theme.palette[2]),
                "surface": hex(self.theme.surface),
                "warning": hex(self.theme.palette[3]),
            }),
        }
    }

    /// Does what a host's layout asked. An id no longer shown is ignored: it
    /// came from a state a moment old.
    pub(crate) fn hosted_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            Action::Command(name) => {
                let found = sheprd_commands::COMMANDS
                    .iter()
                    .find(|info| info.name == name);
                if let Some(info) = found {
                    let command: Command = info.command;
                    self.command(command, window, cx);
                }
            }
            Action::FocusPane(id) => {
                self.navigate(NavigationTarget::Pane(&id), cx);
            }
            Action::FocusWorkspace(id) => {
                self.navigate(NavigationTarget::Workspace(&id), cx);
            }
            Action::Panel(id) => {
                let panel = ActivityPanel::ALL
                    .iter()
                    .copied()
                    .find(|panel| panel_id(*panel) == id);
                if let Some(panel) = panel {
                    self.select_activity_panel(panel, window, cx);
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
