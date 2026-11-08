//! A host's herdr-core seen from the window's other features: installing
//! one clears what the window remembered of a host without it, and the
//! agents working there count when dispatch ranks hosts.

use super::*;
use crate::{
    browser::Scope,
    core_terminals::{Error, Host, Hub, Start},
};
use sheprd_backend::{RunState, TerminalId, TerminalInfo};
use sheprd_client::protocol::AgentStatus;

fn ssh() -> ConnectTarget {
    ConnectTarget::Ssh {
        target: "fixture".into(),
        session: "default".into(),
    }
}

fn scope() -> Scope {
    Scope::endpoint("ssh:fixture")
}

fn missing(_: Start) -> std::result::Result<sheprd_core_client::CoreBackend, Error> {
    Err(Error::Bridge(
        sheprd_client::core_bridge::Error::Missing.into(),
    ))
}

fn agent(id: &str, agent: AgentStatus) -> TerminalInfo {
    TerminalInfo {
        agent,
        cwd: None,
        id: TerminalId::new(id).unwrap(),
        labels: Default::default(),
        name: None,
        state: RunState::Running,
        title: None,
    }
}

#[gpui::test]
fn installing_herdr_core_lets_the_host_take_herdr_2_tabs_again(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let (mut remote, _server) = connected_endpoint("ssh:fixture");
    remote.connection.target = ssh();
    view.update_in(cx, |view, window, cx| {
        view.endpoints.push(remote);
        view.selected_endpoint = 1;
        view.reset_selected(cx);
        view.config.herdr_2_terminals = true;
        let host = Host::Remote(scope());
        view.core_terminals
            .set_remote_hub_for_test(scope(), ssh(), Hub::with_join(missing));
        view.core_terminals
            .hub_of_mut_for_test(&host)
            .unwrap()
            .connect(Start::IfMissing);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !view.core_terminals.lacks_core(&host) {
            assert!(Instant::now() < deadline, "the attempt never finished");
            view.poll_core_terminals(window, cx);
            std::thread::yield_now();
        }
        assert!(!view.new_tabs_use_core());
        view.core_install_ended(
            host.clone(),
            "fixture",
            Ok(sheprd_remote::core_install::Outcome::Installed),
            cx,
        );
        assert!(view.core_terminals.hub_of(&host).is_none());
        assert!(view.new_tabs_use_core());
    });
}

#[gpui::test]
fn a_hosts_working_herdr_core_agents_count_when_ranking_it(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let (mut remote, _server) = connected_endpoint("ssh:fixture");
    remote.connection.target = ssh();
    view.update(cx, |view, cx| {
        view.endpoints[0].connection.target = ConnectTarget::Local;
        view.endpoints.push(remote);
        let host = Host::Remote(scope());
        view.core_terminals
            .set_remote_hub_for_test(scope(), ssh(), Hub::with_join(missing));
        for (id, status) in [
            ("t1", AgentStatus::Working),
            ("t2", AgentStatus::Working),
            ("t3", AgentStatus::Idle),
        ] {
            view.core_terminals
                .record_for_test(&host, agent(id, status));
        }
        assert_eq!(view.core_terminals.working(&host), 2);
        if !cfg!(any(target_os = "linux", target_os = "macos")) {
            return;
        }
        let candidates = view.dispatch_candidates("repo", cx);
        let candidate = candidates
            .iter()
            .find(|candidate| candidate.endpoint_id == "ssh:fixture")
            .unwrap();
        assert_eq!(candidate.working, 2);
    });
}
