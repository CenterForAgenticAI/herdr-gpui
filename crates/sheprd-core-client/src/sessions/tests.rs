use super::*;
use serde_json::json;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod conversion;

#[test]
fn session_events_are_explicit_and_not_prompts() {
    for agent in [AgentKind::Claude, AgentKind::Codex, AgentKind::Copilot] {
        let event = json!({"hook_event_name":"SessionStart", "session_id":"exact-123", "transcript_path":"/log"});
        assert_eq!(
            selection(agent, &event).unwrap().unwrap().value,
            "exact-123"
        );
        assert!(selection(agent, &json!({"session_id":"guessed"})).is_err());
        assert!(
            selection(
                agent,
                &json!({"hook_event_name":"UserPromptSubmit", "session_id":"other"})
            )
            .is_err()
        );
    }
    assert!(
        selection(
            AgentKind::Codex,
            &json!({"hook_event_name":"SessionStart", "session_id":"x"})
        )
        .is_err()
    );
}

#[test]
fn home_clears_an_opencode_selection() {
    assert!(
        selection(AgentKind::Opencode, &json!({"session_id":null}))
            .unwrap()
            .is_none()
    );
    assert!(selection(AgentKind::Opencode, &json!({})).is_err());
    assert!(
        selection(
            AgentKind::Claude,
            &json!({"hook_event_name":"SessionStart", "session_id":"child", "agent_id":"nested"})
        )
        .is_err()
    );
}
