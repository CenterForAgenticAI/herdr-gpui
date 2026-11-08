//! Herdr 2 (herdr-core) terminals in a teleported workspace. herdr-core keeps
//! no tabs, the app does, so the app hands over each tab's pane tree; what
//! runs in each terminal is read through the host's `herdr-core` CLI, as the
//! `herdr` CLI reads a Herdr 1 pane.

use super::{
    error::{Result, Step, decode},
    host::Host,
    launch::{AgentKind, Work},
    layout::Node,
    snapshot::SplitDirection,
};
use serde::Deserialize;
use sheprd_client::shell_quote;
use sheprd_session::panes::{Axis, Node as PaneNode, Panes};
use std::{collections::HashMap, sync::atomic::AtomicBool};

/// The exit status `herdr-core` gives a terminal it does not know, which a
/// stop treats as already gone.
const NOT_FOUND: u8 = 3;

/// Shells a terminal starts with when it was opened with no program; one in
/// the foreground is an idle prompt.
const SHELLS: &[&str] = &[
    // keep-sorted start
    "bash", "csh", "dash", "elvish", "fish", "ksh", "nu", "pwsh", "sh", "tcsh", "xonsh",
    "zsh",
    // keep-sorted end
];

/// One terminal as `herdr-core get --json` reports it, reduced to what a
/// move reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct Terminal {
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub foreground: Option<Foreground>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub status: Option<Status>,
}

/// The terminal's foreground process: its shell at an idle prompt.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct Foreground {
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
}

/// What herdr-core recognized running, such as `claude-code`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct Status {
    #[serde(default)]
    pub app: Option<String>,
}

impl Terminal {
    /// Where its program works: the foreground process's folder, else the
    /// terminal's.
    pub(super) fn cwd(&self) -> Option<String> {
        self.foreground
            .as_ref()
            .and_then(|foreground| foreground.cwd.clone())
            .or_else(|| self.cwd.clone())
    }

    /// What the terminal runs, as a Herdr 1 pane's process info would say.
    /// herdr-core reports no agent session, so an agent is started afresh
    /// with a handoff note rather than resumed.
    pub(super) fn work(&self) -> Work {
        let Some(foreground) = self.foreground.as_ref().filter(|f| !f.argv.is_empty()) else {
            return Work::Shell;
        };
        let argv = &foreground.argv;
        let program = basename(&argv[0]);
        let leader = foreground.pid.is_some() && foreground.pid == self.pid;
        if leader && SHELLS.contains(&program.trim_start_matches('-')) {
            return Work::Shell;
        }
        let agent = AgentKind::parse(program).or_else(|| {
            let app = self.status.as_ref()?.app.as_deref()?;
            AgentKind::parse(app).or_else(|| AgentKind::parse(app.strip_suffix("-code")?))
        });
        match agent {
            Some(agent) => Work::Agent {
                name: agent.name().to_owned(),
                argv: argv.clone(),
            },
            None => Work::Command(argv.clone()),
        }
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Each of `terminals` as its host's core reports it. One the core no
/// longer knows is left out, and its pane becomes an idle shell; any other
/// failure fails the review, so a running program is never taken for one.
pub(super) fn terminals(
    host: &Host,
    terminals: &[String],
    cancelled: &AtomicBool,
) -> Result<HashMap<String, Terminal>> {
    if terminals.is_empty() {
        return Ok(HashMap::new());
    }
    let body = terminals
        .iter()
        .map(|id| {
            format!(
                // herdr-core prints its errors as JSON on stdout too, so
                // only a record is passed on.
                "printf '\\036%s\\n' {id}\nif record=$(core_cli get --json {id}); then printf '%s\\n' \"$record\"; else status=$?; [ \"$status\" -eq {NOT_FOUND} ] || {{ printf '%s\\n' \"$record\" >&2; exit \"$status\"; }}; fi\n",
                id = shell_quote(id)
            )
        })
        .collect::<String>();
    let output = host.query(Step::Review, &body, &[], cancelled)?;
    parse_terminals(&String::from_utf8_lossy(&output))
}

/// One section per terminal: its ID, then its record, or nothing when the
/// core does not know it.
fn parse_terminals(output: &str) -> Result<HashMap<String, Terminal>> {
    let mut found = HashMap::new();
    for section in output.split('\u{1e}').filter(|section| !section.is_empty()) {
        let (id, json) = section.split_once('\n').unwrap_or((section, ""));
        let json = json.trim();
        if json.is_empty() {
            continue;
        }
        let terminal = serde_json::from_str::<Terminal>(json).map_err(decode(Step::Review))?;
        found.insert(id.to_owned(), terminal);
    }
    Ok(found)
}

/// Ask the agent in `terminal` for a handoff note and wait for its turn to
/// end, as `herdr agent prompt --wait` does for a Herdr 1 pane.
pub(super) fn prompt(
    host: &Host,
    terminal: &str,
    prompt: &str,
    timeout: &str,
    cancelled: &AtomicBool,
) -> Result<()> {
    let line = format!(
        "core_cli send --json --wait --timeout {} {} {} >/dev/null\n",
        shell_quote(timeout),
        shell_quote(terminal),
        shell_quote(prompt),
    );
    host.query(Step::Handoff, &line, &[], cancelled).map(drop)
}

/// End the programs of `terminals`. Their tabs belong to the app, which
/// closes them; a terminal already gone counts as ended.
pub(super) fn stop(host: &Host, terminals: &[String], cancelled: &AtomicBool) -> Result<()> {
    if terminals.is_empty() {
        return Ok(());
    }
    let body = terminals
        .iter()
        .map(|id| {
            format!(
                "core_cli kill --json {id} >/dev/null || {{ status=$?; [ \"$status\" -eq {NOT_FOUND} ] || exit \"$status\"; }}\n",
                id = shell_quote(id)
            )
        })
        .collect::<String>();
    host.query(Step::Retire, &body, &[], cancelled).map(drop)
}

/// A tab's panes as the split tree Teleport rebuilds, keyed by terminal ID.
pub(super) fn tree(panes: &Panes) -> Node {
    node(panes.root())
}

fn node(pane: &PaneNode) -> Node {
    match pane {
        PaneNode::Leaf(id) => Node::Pane(id.as_str().to_owned()),
        PaneNode::Split {
            axis,
            share,
            first,
            second,
        } => Node::Split {
            direction: match axis {
                Axis::Row => SplitDirection::Right,
                Axis::Column => SplitDirection::Down,
            },
            ratio: share.fraction(),
            first: Box::new(node(first)),
            second: Box::new(node(second)),
        },
    }
}

/// Every terminal in `tabs`, in tab order.
pub(super) fn ids(tabs: &[Panes]) -> Vec<String> {
    tabs.iter()
        .flat_map(Panes::terminals)
        .map(|id| id.as_str().to_owned())
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
