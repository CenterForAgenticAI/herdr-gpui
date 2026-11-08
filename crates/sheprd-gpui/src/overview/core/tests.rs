//! What a Herdr 2 terminal shows as on the Overview.

use super::agent_name;
use sheprd_backend::{AgentStatus, Process, RunState, TerminalId, TerminalInfo};

fn info(reporter: Option<&str>, program: &str, agent: AgentStatus) -> TerminalInfo {
    TerminalInfo {
        agent,
        cwd: None,
        foreground: Some(Process {
            argv: vec![program.to_owned()],
            name: program.to_owned(),
            pid: 9,
        }),
        id: TerminalId::new("t1").unwrap(),
        labels: Default::default(),
        name: None,
        reporter: reporter.map(str::to_owned),
        state: RunState::Running,
        title: None,
    }
}

#[test]
fn a_known_agent_is_named_by_its_identity() {
    let claude = info(Some("claude-code"), "node", AgentStatus::Working);
    assert_eq!(agent_name(&claude).as_deref(), Some("claude"));
    let codex = info(None, "codex", AgentStatus::Unknown);
    assert_eq!(agent_name(&codex).as_deref(), Some("codex"));
}

#[test]
fn an_unknown_program_that_reports_a_status_is_an_agent_too() {
    let other = info(Some("my\u{202e}bot"), "python3", AgentStatus::Idle);
    assert_eq!(agent_name(&other).as_deref(), Some("mybot"));
}

#[test]
fn shells_and_silent_programs_are_not_listed() {
    assert_eq!(agent_name(&info(None, "zsh", AgentStatus::Unknown)), None);
    assert_eq!(
        agent_name(&info(Some("tool"), "tool", AgentStatus::Unknown)),
        None
    );
}
