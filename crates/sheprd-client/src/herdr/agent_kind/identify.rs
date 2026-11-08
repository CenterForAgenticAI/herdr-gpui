//! Which agent a terminal runs when no Herdr 1 integration says so, as on
//! herdr-core: first the name the program reports for itself (OSC 7501's
//! `app`), then the process in front. Names follow Herdr's own lookup
//! (`detect::lookup_agent`), so both generations agree on what an agent is.

use super::AgentKind;

/// Runtimes that run an agent's script rather than being one.
const RUNTIMES: &[&str] = &["bun", "deno", "node"];

impl AgentKind {
    /// The agent behind `name`: a reported app such as `claude-code`, or a
    /// program such as `/usr/local/bin/cursor-agent` or `codex.exe`.
    pub fn lookup(name: &str) -> Option<Self> {
        Some(match base(name).to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Self::Claude,
            "codex" => Self::Codex,
            "opencode" | "opencode2" | "open-code" => Self::Opencode,
            "pi" => Self::Pi,
            "omp" => Self::Omp,
            "copilot" | "github-copilot" | "ghcs" => Self::Copilot,
            "devin" | "devin-cli" => Self::Devin,
            "droid" => Self::Droid,
            "kimi" | "kimi-code" => Self::Kimi,
            "mastracode" | "mastra-code" => Self::Mastracode,
            "hermes" | "hermes-agent" => Self::Hermes,
            "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Self::Qodercli,
            "qwen" | "qwen-code" => Self::Qwen,
            "kilo" | "kilo-code" => Self::Kilo,
            "cursor" | "cursor-agent" => Self::Cursor,
            "agy" | "antigravity" | "antigravity-cli" => Self::Antigravity,
            "grok" | "grok-build" => Self::Grok,
            "letta" | "letta-code" => Self::Letta,
            _ => return None,
        })
    }

    /// The agent a terminal runs: the one its program reports being, else
    /// the one its foreground process is, looking past a runtime such as
    /// `node` to the script it runs. Shells and other programs are none.
    pub fn identify(reporter: Option<&str>, name: &str, argv: &[String]) -> Option<Self> {
        if let Some(kind) = reporter.and_then(Self::lookup) {
            return Some(kind);
        }
        let program = argv.first().map_or(name, String::as_str);
        if let Some(kind) = Self::lookup(program).or_else(|| Self::lookup(name)) {
            return Some(kind);
        }
        if !RUNTIMES.contains(&base(program).to_ascii_lowercase().as_str()) {
            return None;
        }
        // The first argument that is not a flag is the script.
        let script = argv.iter().skip(1).find(|arg| !arg.starts_with('-'))?;
        Self::lookup(script)
    }
}

/// A program's name without its folder, a login shell's leading dash, or a
/// Windows or script extension.
fn base(name: &str) -> &str {
    let name = name.trim();
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let name = name.trim_start_matches('-');
    [".exe", ".cmd", ".js", ".mjs", ".cjs"]
        .iter()
        .find_map(|extension| {
            let cut = name.len().checked_sub(extension.len())?;
            name.get(cut..)
                .filter(|tail| tail.eq_ignore_ascii_case(extension))
                .and_then(|_| name.get(..cut))
        })
        .unwrap_or(name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
