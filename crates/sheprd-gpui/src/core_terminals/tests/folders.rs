use crate::{
    browser::{Scope, TabId},
    core_terminals::{CoreTerminals, Host, TabPanes, panes::Panes},
};
use sheprd_backend::{RunState, TerminalId, TerminalInfo};
use sheprd_client::protocol::AgentStatus;
use std::path::PathBuf;

fn id(name: &str) -> TerminalId {
    TerminalId::new(name).unwrap()
}

fn info(name: &str, cwd: &str) -> TerminalInfo {
    TerminalInfo {
        agent: AgentStatus::Unknown,
        cwd: Some(PathBuf::from(cwd)),
        foreground: None,
        id: id(name),
        labels: Default::default(),
        name: None,
        reporter: None,
        state: RunState::Running,
        title: None,
    }
}

fn tab(terminals: &mut CoreTerminals, number: u64, terminal: &str, workspace: &str, observe: bool) {
    let mut shown = TabPanes::new(Host::Local, Panes::single(id(terminal)), observe);
    shown.workspace = Some((Scope::endpoint("local"), workspace.to_owned()));
    terminals.tabs.insert(TabId::test(number), shown);
}

fn keep_roots(terminals: &mut CoreTerminals) {
    let infos = &terminals.infos;
    for shown in terminals.tabs.values_mut() {
        shown.keep_root(infos);
    }
}

#[test]
fn a_core_workspace_started_where_its_oldest_driving_tab_did() {
    let mut terminals = CoreTerminals::default();
    tab(&mut terminals, 3, "t3", "w", false);
    tab(&mut terminals, 2, "t2", "w", false);
    tab(&mut terminals, 1, "t1", "w", true);
    tab(&mut terminals, 0, "t0", "other", false);
    for (name, cwd) in [("t0", "/o"), ("t1", "/m"), ("t2", "/two"), ("t3", "/three")] {
        terminals.infos.insert(info(name, cwd));
    }
    keep_roots(&mut terminals);
    // The mirror and the other workspace's tab are older, and passed over.
    assert_eq!(
        terminals.launch_root(&Scope::endpoint("local"), "w"),
        Some(PathBuf::from("/two"))
    );
    assert_eq!(
        terminals.launch_root(&Scope::endpoint("local"), "none"),
        None
    );
}

#[test]
fn a_core_workspace_stays_where_it_started_when_its_pane_moves() {
    let mut terminals = CoreTerminals::default();
    tab(&mut terminals, 0, "t0", "w", false);
    terminals.infos.insert(info("t0", "/start"));
    keep_roots(&mut terminals);
    terminals.infos.insert(info("t0", "/start/elsewhere"));
    keep_roots(&mut terminals);
    assert_eq!(
        terminals.launch_root(&Scope::endpoint("local"), "w"),
        Some(PathBuf::from("/start"))
    );
}
