//! Which agent runs in a herdr-core terminal. herdr-core has no notion of an
//! agent: it passes on what the program reports (OSC 7501) and which process
//! is in front, and the policy that names the agent is `sheprd-client`'s.

use sheprd_backend::TerminalInfo;
use sheprd_client::herdr::AgentKind;

/// The agent `info`'s terminal runs, by its report, else its foreground
/// process; none for a shell or any other program.
pub fn agent_kind(info: &TerminalInfo) -> Option<AgentKind> {
    let (name, argv) = info.foreground.as_ref().map_or(("", &[][..]), |process| {
        (process.name.as_str(), process.argv.as_slice())
    });
    AgentKind::identify(info.reporter.as_deref(), name, argv)
}
