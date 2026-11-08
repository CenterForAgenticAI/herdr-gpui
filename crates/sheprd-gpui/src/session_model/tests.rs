use super::*;
use crate::sidebar::layout_tests::snapshot;
use sheprd_client::protocol::AgentStatus;
use sheprd_session::CoreWorkspace;

#[test]
fn a_herdr_1_snapshot_reads_as_its_session() {
    let snapshot = snapshot(4);
    let session = of_snapshot(&snapshot);
    assert_eq!(session.focused.as_deref(), Some("w0"), "marked focused");
    let ids: Vec<_> = session.workspaces.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["w0", "w1", "w2", "w3"]);
    let first = session.workspace("w0").unwrap();
    assert_eq!(first.keeper, Keeper::Herdr1);
    assert_eq!(first.label, "herdr");
    assert_eq!(first.branch.as_deref(), Some("main"));
    let tabs: Vec<_> = first
        .tabs
        .iter()
        .map(|tab| (tab.id.as_str(), tab.focused))
        .collect();
    assert_eq!(tabs, [("t0", true), ("t1", false)]);
    assert!(session.workspace("w1").unwrap().tabs.is_empty());
    let checkout = session.workspace("w3").unwrap().worktree.as_ref().unwrap();
    assert_eq!(checkout.label, "agent-launcher");
    assert!(!checkout.linked);
    let agents: Vec<_> = session
        .agents
        .iter()
        .map(|agent| {
            (
                agent.pane.as_str(),
                agent.name.as_str(),
                agent.workspace.as_str(),
            )
        })
        .collect();
    assert_eq!(agents, [("p0", "Claude Code", "w0"), ("p1", "agent", "w1")]);
    assert_eq!(session.agents[0].identity.as_deref(), Some("claude"));
}

#[test]
fn a_snapshot_session_is_built_once_per_snapshot() {
    let cache = SnapshotSession::default();
    let first = Arc::new(snapshot(1));
    let built = cache.get(&first);
    assert!(Arc::ptr_eq(&built, &cache.get(&first)));
    let second = Arc::new(snapshot(2));
    let rebuilt = cache.get(&second);
    assert!(!Arc::ptr_eq(&built, &rebuilt));
    assert_eq!(rebuilt.workspaces.len(), 2);
}

#[test]
fn a_host_lists_herdr_1_then_core_workspaces() {
    let herdr_1 = Arc::new(of_snapshot(&snapshot(2)));
    let core = Arc::new(Session::core(
        [CoreWorkspace {
            cwd: "",
            id: "herdr-2",
            label: "Herdr 2",
        }],
        [],
        None,
        |_| None,
    ));
    let sessions = Sessions {
        core: Some(core),
        core_answers: true,
        herdr_1: Some(herdr_1),
    };
    let ids: Vec<_> = sessions.workspaces().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["w0", "w1", "herdr-2"]);
    assert_eq!(sessions.workspace("herdr-2").unwrap().keeper, Keeper::App);
    assert_eq!(sessions.agents().count(), 2);
    assert!(
        sessions
            .agents()
            .all(|agent| agent.status == AgentStatus::Working)
    );
    assert!(!sessions.is_empty());
    assert!(Sessions::default().is_empty());
}
