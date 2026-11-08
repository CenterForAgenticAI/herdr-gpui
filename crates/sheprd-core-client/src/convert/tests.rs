use super::*;
use sheprd_backend::RunState;
use std::collections::BTreeMap;

fn process(pid: u32, argv: &[&str]) -> Process {
    Process {
        argv: argv.iter().map(|arg| (*arg).to_owned()).collect(),
        name: argv[0].rsplit('/').next().unwrap().to_owned(),
        pid,
    }
}

fn info(status: AgentStatus, foreground: Option<Process>) -> TerminalInfo {
    TerminalInfo {
        agent: status,
        cwd: Some("/work/app".into()),
        foreground,
        id: TerminalId::new("t1").unwrap(),
        labels: BTreeMap::new(),
        name: None,
        reporter: None,
        state: RunState::Running,
        title: None,
    }
}

fn claude(status: AgentStatus) -> TerminalInfo {
    info(
        status,
        Some(process(
            42,
            &["/usr/local/bin/claude", "--model", "opus", "fix it"],
        )),
    )
}

fn record(pid: u32, session: &str) -> String {
    format!(r#"{{"pid":{pid},"sessionId":"{session}","cwd":"/work/app","status":"idle"}}"#)
}

#[test]
fn only_an_idle_known_agent_moves() {
    assert_eq!(check(&claude(AgentStatus::Idle)).unwrap(), Agent::Claude);
    assert_eq!(check(&claude(AgentStatus::Done)).unwrap(), Agent::Claude);
    assert!(matches!(
        check(&claude(AgentStatus::Working)),
        Err(convert::Error::Busy(Agent::Claude))
    ));
    assert!(matches!(
        check(&info(AgentStatus::Idle, Some(process(9, &["zsh"])))),
        Err(convert::Error::NoAgent)
    ));
}

#[test]
fn a_chat_does_not_move_again() {
    let mut chat = claude(AgentStatus::Idle);
    chat.labels
        .insert(TOKEN.to_owned(), "0123456789abcdef0123456789abcdef".into());
    assert!(matches!(check(&chat), Err(convert::Error::AlreadyChat)));
}

#[test]
fn claudes_session_comes_from_its_own_record() {
    let home = Path::new("/home/me");
    let resume = resume(&claude(AgentStatus::Idle), Agent::Claude, home, |path| {
        assert_eq!(path, Path::new("/home/me/.claude/sessions/42.json"));
        Ok(record(42, "e6355726-1f61-42c3-bf05-5c1d1cba3262"))
    })
    .unwrap();
    assert_eq!(resume.session, "e6355726-1f61-42c3-bf05-5c1d1cba3262");
    assert_eq!(resume.cwd, "/work/app");
    assert_eq!(
        resume.flags,
        ["--model", "opus"],
        "the prompt is not replayed"
    );
}

#[test]
fn a_record_of_another_process_or_an_odd_session_is_none() {
    let home = Path::new("/h");
    let found = claude(AgentStatus::Idle);
    for text in [
        record(43, "abc"),
        record(42, "-rf"),
        record(42, "a b"),
        "not json".to_owned(),
    ] {
        assert!(matches!(
            resume(&found, Agent::Claude, home, |_| Ok(text.clone())),
            Err(convert::Error::NoCoreSession(Agent::Claude))
        ));
    }
    assert!(matches!(
        resume(&found, Agent::Claude, home, |_| Err(
            io::ErrorKind::NotFound.into()
        )),
        Err(convert::Error::NoCoreSession(Agent::Claude))
    ));
}

#[test]
fn other_agents_have_no_session_to_read_yet() {
    let codex = info(AgentStatus::Idle, Some(process(7, &["codex"])));
    assert!(matches!(
        resume(&codex, Agent::Codex, Path::new("/h"), |_| unreachable!()),
        Err(convert::Error::NoCoreSession(Agent::Codex))
    ));
}

#[test]
fn the_shell_is_back_once_the_agent_left_the_front() {
    assert!(!shell_in_front(&claude(AgentStatus::Idle), Agent::Claude));
    let shell = info(AgentStatus::Idle, Some(process(9, &["-zsh"])));
    assert!(shell_in_front(&shell, Agent::Claude));
    // Nothing in front tells nothing.
    assert!(!shell_in_front(
        &info(AgentStatus::Idle, None),
        Agent::Claude
    ));
}
