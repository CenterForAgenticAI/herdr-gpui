//! Which of a pane's foreground processes run a given agent.

use super::snapshot::{Process, ProcessInfo};
use sheprd_chat_protocol::Agent;

/// The most processes matching an agent that are worth telling apart: past
/// these the pane runs something no single session explains.
const MAX_MATCHING_PROCESSES: usize = 16;

/// The foreground processes running `agent`, unless there are more than
/// [`MAX_MATCHING_PROCESSES`] to verify.
pub fn agent_processes(agent: Agent, info: &ProcessInfo) -> Option<Vec<&Process>> {
    let matching: Vec<_> = info
        .foreground_processes
        .iter()
        .filter(|process| is_agent(agent, &process.argv))
        .take(MAX_MATCHING_PROCESSES + 1)
        .collect();
    (matching.len() <= MAX_MATCHING_PROCESSES).then_some(matching)
}

/// The program an argument vector runs, without its directory.
pub fn program(argv: &[String]) -> Option<&str> {
    argv.first()?.rsplit('/').next()
}

/// Whether `argv` runs `agent` itself, directly or as a Node or Bun script,
/// rather than naming it as an argument to some other program.
pub fn is_agent(agent: Agent, argv: &[String]) -> bool {
    let Some(program) = program(argv) else {
        return false;
    };
    if program == agent.name() {
        return true;
    }
    if !matches!(program, "node" | "nodejs" | "bun") {
        return false;
    }
    argv.get(1).is_some_and(|script| match agent {
        Agent::Claude => {
            script.ends_with("/@anthropic-ai/claude-code/cli.js") || script.ends_with("/claude")
        }
        Agent::Codex => {
            script.ends_with("/@openai/codex/bin/codex.js") || script.ends_with("/codex")
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests;
