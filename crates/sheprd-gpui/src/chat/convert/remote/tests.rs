use super::*;

#[test]
fn conversion_clears_inherited_core_selection_and_requires_the_captured_core() {
    let terminal = TerminalId::new("t1").unwrap();
    let script = script("core-one", &terminal, Agent::Codex);
    assert!(script.starts_with("unset HC_SOCKET HC_SESSION HC_TERMINAL_ID HC_TERMINAL_SOCKET;"));
    assert!(script.contains(&format!(
        "export SHEPRD_EXPECT_CORE={}",
        shell_quote("core-one")
    )));
    assert!(script.contains(&format!("session-convert {} codex", shell_quote("t1"))));
    assert!(!script.contains("/Users/"));
}

#[test]
fn cancelling_before_conversion_never_contacts_the_host() {
    let owner = Owner {
        core: "core-one".into(),
        host: Host::Remote(crate::browser::Scope::endpoint("ssh:box")),
        target: ConnectTarget::Ssh {
            target: "invalid.example".into(),
            session: "default".into(),
        },
    };
    let result = run(
        &owner,
        &TerminalId::new("t1").unwrap(),
        Agent::Codex,
        &AtomicBool::new(true),
    );
    assert!(
        matches!(result, Err(Error::Connect(error)) if error.kind() == std::io::ErrorKind::Interrupted)
    );
}
