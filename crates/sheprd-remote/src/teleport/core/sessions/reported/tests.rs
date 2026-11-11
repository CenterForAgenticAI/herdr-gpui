use super::*;
use sheprd_client::herdr::sessions::{Identity, reference};
use std::collections::BTreeMap;

#[cfg(unix)]
#[test]
fn owning_host_helper_distinguishes_absence_unavailable_and_failure() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let home = tempfile::tempdir().unwrap();
    let program = home.path().join(".local/bin/sheprd-acp");
    fs::create_dir_all(program.parent().unwrap()).unwrap();
    fs::write(&program, "#!/bin/sh\n[ \"$1 $2 $3\" = 'session-discover term-1 codex' ] || exit 2\nstatus=$(cat \"$HOME/status\")\n[ \"$status\" = 0 ] || exit \"$status\"\ncat \"$HOME/binding\"\n").unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
    let mut host = Host::new(&sheprd_client::ConnectTarget::Local).unwrap();
    host.env = vec![
        ("HOME".into(), home.path().to_string_lossy().into_owned()),
        (
            "HC_SOCKET".into(),
            home.path()
                .join("isolated.sock")
                .to_string_lossy()
                .into_owned(),
        ),
    ];
    let terminals = HashMap::from([(
        "term-1".into(),
        serde_json::from_str::<Terminal>(
            r#"{"foreground":{"argv":["codex"],"pid":42},"status":{"app":"codex"}}"#,
        )
        .unwrap(),
    )]);
    let record = binding(AgentKind::Codex);
    fs::write(
        home.path().join("binding"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    fs::write(home.path().join("status"), "0").unwrap();
    let (found, unavailable) = discover(&host, &terminals, &AtomicBool::new(false)).unwrap();
    assert_eq!(found.bindings["term-1"], record);
    assert_eq!(found.sessions.get("term-1"), record.selected());
    assert!(unavailable.is_empty());
    fs::write(home.path().join("status"), "3").unwrap();
    let (found, unavailable) = discover(&host, &terminals, &AtomicBool::new(false)).unwrap();
    assert!(found.sessions.is_empty());
    assert!(
        unavailable.is_empty(),
        "explicitly cleared evidence must not fall back"
    );
    fs::write(home.path().join("status"), "127").unwrap();
    let (found, unavailable) = discover(&host, &terminals, &AtomicBool::new(false)).unwrap();
    assert!(found.sessions.is_empty());
    assert_eq!(unavailable, ["term-1"]);
    fs::write(home.path().join("status"), "1").unwrap();
    assert!(discover(&host, &terminals, &AtomicBool::new(false)).is_err());
}

fn binding(agent: AgentKind) -> Binding {
    Binding {
        version: 1,
        sequence: 1,
        identity: Identity {
            core: "core-1".into(),
            terminal: "term-1".into(),
            pid: 42,
            started: "birth-1".into(),
        },
        agent: agent.name().into(),
        session: Some(reference(agent, SessionKind::Id, "session-1".into()).unwrap()),
        environment: BTreeMap::new(),
    }
}

#[test]
fn source_binding_preserves_provenance_and_process_scope() {
    for agent in AgentKind::ALL.into_iter().filter(|agent| agent.carries()) {
        let expected = binding(agent);
        let bytes = serde_json::to_vec(&expected).unwrap();
        assert_eq!(validate(&bytes, "term-1", 42, agent).unwrap(), expected);
        assert!(matches!(
            validate(&bytes, "other", 42, agent),
            Err(Error::SourceSessionChanged)
        ));
        assert!(matches!(
            validate(&bytes, "term-1", 43, agent),
            Err(Error::SourceSessionChanged)
        ));
    }
}

#[test]
fn custom_source_roots_are_not_silently_lost() {
    let mut record = binding(AgentKind::Codex);
    record
        .environment
        .insert("CODEX_HOME".into(), "/source/custom-codex".into());
    assert!(matches!(
        validate(
            &serde_json::to_vec(&record).unwrap(),
            "term-1",
            42,
            AgentKind::Codex
        ),
        Err(Error::SessionConfiguration { agent: "codex" })
    ));
}

#[test]
fn malformed_large_and_wrong_agent_bindings_fail_closed() {
    let bytes = serde_json::to_vec(&binding(AgentKind::Codex)).unwrap();
    assert!(matches!(
        validate(&bytes, "term-1", 42, AgentKind::Copilot),
        Err(Error::SourceSessionChanged)
    ));
    assert!(validate(b"not json", "term-1", 42, AgentKind::Codex).is_err());
    assert!(matches!(
        validate(
            &vec![b' '; sheprd_client::herdr::sessions::MAX_RECORD + 1],
            "term-1",
            42,
            AgentKind::Codex
        ),
        Err(Error::SourceSessionChanged)
    ));
}
