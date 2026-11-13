use super::*;

#[test]
fn agent_listeners_do_not_create_a_workspace_port_row() {
    let ports = parse(
        "L 10 127.0.0.1:63644 codex\n\
         L 10 [::1]:63644 codex\n\
         L 11 127.0.0.1:61031 2.1.322\n\
         L 12 127.0.0.1:62257 claude\n\
         E 10 w1\nE 11 w1\nE 12 w1\n",
    )
    .unwrap();
    assert!(ports.ports.is_empty() && ports.pids.is_empty());
}

#[test]
fn agent_listeners_in_a_core_terminal_are_dropped() {
    let ports =
        parse("L 10 127.0.0.1:63644 codex\nL 11 127.0.0.1:61031 2.1.323\nC 10 t1\nC 11 t1\n")
            .unwrap();
    assert!(ports.terminals.is_empty() && ports.terminal_pids.is_empty());
}

#[test]
fn servers_an_agent_starts_remain_visible() {
    let ports = parse(
        "L 10 127.0.0.1:63644 codex\n\
         L 11 *:5173 node\n\
         L 12 127.0.0.1:8000 python3.12\n\
         L 13 127.0.0.1:9000 codex-preview\n\
         L 14 127.0.0.1:9001 claude-dev\n\
         L 15 127.0.0.1:9002 v2.1\n\
         E 10 w1\nE 11 w1\nE 12 w1\nE 13 w1\nE 14 w1\nE 15 w1\n",
    )
    .unwrap();
    assert_eq!(
        of(&ports, "w1"),
        [
            port(5173, Bind::Any, "node"),
            port(8000, Bind::Loopback, "python3.12"),
            port(9000, Bind::Loopback, "codex-preview"),
            port(9001, Bind::Loopback, "claude-dev"),
            port(9002, Bind::Loopback, "v2.1"),
        ]
    );
}

#[test]
fn an_agent_does_not_hide_another_process_on_the_same_port() {
    let ports = parse(
        "L 10 127.0.0.1:5173 codex\n\
         L 11 [::1]:5173 node\n\
         E 10 w1\nE 11 w1\n",
    )
    .unwrap();
    assert_eq!(of(&ports, "w1"), [port(5173, Bind::Loopback, "node")]);
}
