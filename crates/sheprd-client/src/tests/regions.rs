//! Desktop regions through the session and the socket: the hello asks only
//! when told to, region traffic needs both the hello and the welcome, and a
//! connection that did not negotiate regions never writes region commands.

use super::*;
use crate::protocol::region_surface::{
    self, ClientRegion, RegionProjection, RegionSurface, RegionViewport,
};

fn region() -> ClientRegion {
    ClientRegion {
        region_id: "region_1".into(),
        terminal_id: "term_1".into(),
        plugin_id: "board".into(),
        entrypoint: "board".into(),
        title: "Board".into(),
        edge: "right".into(),
        scope: "session".into(),
        size: 30,
        min_size: 10,
        max_size: 60,
        visible: true,
        focused: false,
        mouse_reporting: false,
    }
}

fn projection(revision: u64) -> ServerMessage {
    RegionProjection {
        boot_id: "boot-v1".into(),
        revision,
        regions: vec![region()],
        focused: None,
    }
    .message()
    .unwrap()
}

fn surface() -> ServerMessage {
    RegionSurface {
        region_id: "region_1".into(),
        terminal_id: "term_1".into(),
        frame: baseline().frame,
    }
    .message()
    .unwrap()
}

fn welcome(regions: bool) -> String {
    let mut welcome: Value = serde_json::from_str(WELCOME).unwrap();
    welcome["methods"] = json!(["pane.focus", "region.focus", "region.resize"]);
    if regions {
        welcome["capabilities"] = json!([region_surface::CAPABILITY]);
    }
    welcome.to_string()
}

/// A ready session that asked for regions when `requested`, against a
/// daemon that offered them when `offered`.
fn session(requested: bool, offered: bool) -> Session {
    let mut session = Session::new(true, false);
    session.regions.requested = requested;
    for (kind, data) in [
        (ENDPOINT_WELCOME_KIND, welcome(offered)),
        (ENDPOINT_SNAPSHOT_KIND, SNAPSHOT.into()),
    ] {
        session
            .handle_message(
                ServerMessage::EndpointControl {
                    kind: kind.into(),
                    data,
                },
                |_| Ok(()),
            )
            .unwrap();
    }
    session
}

fn region_events(session: &mut Session, message: ServerMessage) -> Vec<ClientEvent> {
    let mut events = Vec::new();
    session
        .handle_message(message, |event| {
            events.push(event);
            Ok(())
        })
        .unwrap();
    events
}

fn hello_from(server: &mut Stream) -> EndpointClientHello {
    server
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let ClientMessage::EndpointControl { kind, data } = receive(server) else {
        panic!("not a hello");
    };
    assert_eq!(kind, ENDPOINT_HELLO_KIND);
    serde_json::from_str(&data).unwrap()
}

fn connected(hello: Hello, offered: bool) -> (Client, Stream, thread::JoinHandle<Result<()>>) {
    let (client, mut server, worker, _) = test_client_hello(hello, false, Pace::default());
    assert_eq!(hello_from(&mut server).regions, hello.regions);
    for (kind, data) in [
        (ENDPOINT_WELCOME_KIND, welcome(offered)),
        (ENDPOINT_SNAPSHOT_KIND, SNAPSHOT.into()),
    ] {
        send(
            &mut server,
            ServerMessage::EndpointControl {
                kind: kind.into(),
                data,
            },
        );
    }
    assert!(matches!(event(&client), ClientEvent::Connected(_)));
    assert!(matches!(event(&client), ClientEvent::Snapshot(_)));
    (client, server, worker)
}

const REGIONS: Hello = Hello {
    surface_active: true,
    regions: true,
};

fn viewport() -> RegionViewport {
    RegionViewport {
        region_id: "region_1".into(),
        cols: 30,
        rows: 20,
    }
}

#[test]
fn the_default_hello_does_not_ask_for_regions() {
    let (_client, mut server, _worker) = test_client();
    assert!(!hello_from(&mut server).regions);
    let (_client, mut server, _worker, _) = test_client_hello(REGIONS, false, Pace::default());
    assert!(hello_from(&mut server).regions);
}

#[test]
fn region_messages_need_both_the_hello_and_the_welcome() {
    for (requested, offered) in [(false, false), (false, true), (true, false)] {
        let mut session = session(requested, offered);
        assert!(!session.regions.negotiated);
        assert!(region_events(&mut session, projection(1)).is_empty());
        assert!(region_events(&mut session, surface()).is_empty());
    }
    let mut session = session(true, true);
    assert!(session.regions.negotiated);
    assert!(matches!(
        region_events(&mut session, projection(1)).as_slice(),
        [ClientEvent::RegionProjection(p)] if p.revision == 1 && p.regions == [region()]
    ));
    assert!(matches!(
        region_events(&mut session, surface()).as_slice(),
        [ClientEvent::RegionSurface(s)] if s.terminal_id == "term_1"
    ));
}

#[test]
fn malformed_region_messages_are_dropped_without_ending_the_connection() {
    let mut session = session(true, true);
    for (kind, data) in [
        (region_surface::PROJECTION_KIND, "{\"revision\":\"x\"}"),
        (region_surface::SURFACE_KIND, "not base64!"),
        (region_surface::SURFACE_KIND, ""),
    ] {
        let message = ServerMessage::EndpointControl {
            kind: kind.into(),
            data: data.into(),
        };
        assert!(region_events(&mut session, message).is_empty(), "{kind}");
    }
    assert!(!region_events(&mut session, projection(2)).is_empty());
}

#[test]
fn without_negotiation_region_commands_never_reach_the_wire() {
    for (hello, offered) in [(REGIONS, false), (Hello::default(), true)] {
        let (client, mut server, worker) = connected(hello, offered);
        let handle = &client.handle;
        handle.send_region_viewport("boot-v1", &viewport()).unwrap();
        handle
            .send_region_input(
                "boot-v1",
                "term_1",
                [ClientPaneInputEvent::TextCommit("x".into())],
            )
            .unwrap();
        let focus = handle.focus_region("boot-v1", "region_1").unwrap();
        for expected in [None, None, Some(focus)] {
            assert!(matches!(
                event(&client),
                ClientEvent::CommandRejected {
                    request_id,
                    reason: Error::Regions(crate::regions::Error::NotNegotiated),
                } if request_id == expected
            ));
        }
        // The next frame on the wire is the pane input queued after them.
        handle
            .send_input(
                "boot-v1",
                "w1:p1",
                [ClientPaneInputEvent::TextCommit("p".into())],
            )
            .unwrap();
        assert!(matches!(
            receive(&mut server),
            ClientMessage::ClientShellPaneInput { pane_id, .. } if pane_id == "w1:p1"
        ));
        handle.disconnect();
        worker.join().unwrap().unwrap();
    }
}

#[test]
fn a_negotiated_connection_writes_viewports_input_and_region_requests() {
    let (client, mut server, worker) = connected(REGIONS, true);
    let handle = &client.handle;
    handle.send_region_viewport("boot-v1", &viewport()).unwrap();
    let ClientMessage::EndpointControl { kind, data } = receive(&mut server) else {
        panic!("not a viewport");
    };
    assert_eq!(kind, region_surface::VIEWPORT_KIND);
    assert_eq!(RegionViewport::decode(&data).unwrap(), viewport());
    handle
        .send_region_input(
            "boot-v1",
            "term_1",
            [ClientPaneInputEvent::TextCommit("x".into())],
        )
        .unwrap();
    assert!(matches!(
        receive(&mut server),
        ClientMessage::ClientShellPopupInput { terminal_id, events }
            if terminal_id == "term_1" && events == [ClientPaneInputEvent::TextCommit("x".into())]
    ));
    let id = handle.resize_region("boot-v1", "region_1", 42).unwrap();
    let ClientMessage::ClientShellEndpointRequest { request, .. } = receive(&mut server) else {
        panic!("not a request");
    };
    let request: Value = serde_json::from_str(&request).unwrap();
    assert_eq!(request["id"], id.as_str());
    assert_eq!(request["method"], "region.resize");
    assert_eq!(
        request["params"],
        json!({"region_id": "region_1", "size": 42})
    );
    // A method the daemon did not advertise is refused like any other, at once.
    let hide = handle.hide_region("boot-v1", "region_1").unwrap();
    assert!(matches!(
        event(&client),
        ClientEvent::CommandRejected { request_id: Some(rejected), reason: Error::UnsupportedMethod }
            if rejected == hide
    ));
    send(
        &mut server,
        ServerMessage::ClientShellEndpointResponseChunk {
            boot_id: "boot-v1".into(),
            request_id: id.clone(),
            final_chunk: true,
            data: json!({"id": id, "result": {}}).to_string().into_bytes(),
        },
    );
    assert!(matches!(event(&client), ClientEvent::Response { request_id, .. } if request_id == id));
    handle.disconnect();
    worker.join().unwrap().unwrap();
}
