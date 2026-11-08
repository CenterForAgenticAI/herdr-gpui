//! Ports of herdr-core terminals, placed under the workspace whose tab
//! shows each terminal.

use super::*;

#[test]
fn a_listener_in_a_core_terminal_names_its_terminal_unless_a_workspace_claims_it() {
    let scan = parse(
        "L 10 *:5173 node\n\
         L 11 127.0.0.1:3000 rails\n\
         L 12 *:8080 python3\n\
         L 13 *:9000 rogue\n\
         C 10 t7 /run/hc/default.sock\n\
         E 11 w7V\n\
         C 11 t8\n\
         C 12 box/t2\n\
         C 13 t=9\n",
    )
    .unwrap();
    // A Herdr workspace wins: a core tab in it names the workspace itself.
    assert_eq!(of(&scan, "w7V"), [port(3000, Bind::Loopback, "rails")]);
    let mut terminals: Vec<(&str, &str, Vec<u16>)> = scan
        .terminals
        .iter()
        .map(|(owner, ports)| {
            (
                owner.terminal.as_str(),
                owner.socket.as_str(),
                ports.iter().map(|port| port.number).collect(),
            )
        })
        .collect();
    terminals.sort();
    assert_eq!(
        terminals,
        [
            ("box/t2", "", vec![8080]),
            ("t7", "/run/hc/default.sock", vec![5173]),
        ]
    );
}

#[test]
fn core_ports_follow_the_workspace_each_terminal_is_shown_in() {
    let mut ports = ListeningPorts::default();
    ports.seed(
        Host::Local,
        parse(
            "L 10 *:5173 node\nL 10 [::1]:5173 node\nL 11 *:3000 rails\nL 12 *:4000 x\n\
             C 10 t1\nC 11 t2\nC 12 t3\n",
        )
        .unwrap(),
    );
    assert!(ports.core_ports("herdr-2").is_none());
    let places = vec![
        ("t2".to_owned(), "herdr-2".to_owned()),
        ("t1".to_owned(), "herdr-2".to_owned()),
        ("t3".to_owned(), "other".to_owned()),
    ];
    assert!(ports.place_terminals(places.clone()));
    let listed = ports.core_ports("herdr-2").unwrap();
    assert_eq!(
        listed.ports,
        [
            port(3000, Bind::Any, "rails"),
            port(5173, Bind::Any, "node")
        ]
    );
    assert_eq!(*listed.origin, Origin::Local);
    assert_eq!(ports.core_ports("other").unwrap().ports.len(), 1);
    // The same places and scan change nothing; a moved tab does.
    assert!(!ports.place_terminals(places));
    assert!(ports.place_terminals(vec![("t1".to_owned(), "other".to_owned())]));
    assert!(ports.core_ports("herdr-2").is_none());
    assert_eq!(ports.core_ports("other").unwrap().ports.len(), 1);
}

#[test]
fn shared_lookups_find_core_ports_where_herdr_names_none() {
    let mut ports = ListeningPorts::default();
    ports.seed(
        Host::Local,
        parse("L 10 *:5173 node\nL 11 *:3000 rails\nC 10 t1\nE 11 w1\n").unwrap(),
    );
    ports.place_terminals(vec![("t1".to_owned(), "herdr-2".to_owned())]);
    let local = Daemon::of(&ConnectTarget::Socket("/run/h/herdr-client.sock".into())).unwrap();
    // The workspace's chips and the Ports section agree on the listener.
    assert_eq!(
        ports.get(&local, "herdr-2").unwrap().ports,
        [port(5173, Bind::Any, "node")]
    );
    assert_eq!(ports.workspace_of(&local, 10), Some("herdr-2"));
    // A saved host's daemon never takes this computer's terminals.
    let remote = remote("default");
    assert!(ports.get(&remote, "herdr-2").is_none());
    assert_eq!(ports.workspace_of(&remote, 10), None);
    // A pid no terminal is placed for has no workspace.
    assert_eq!(ports.workspace_of(&local, 99), None);
}
