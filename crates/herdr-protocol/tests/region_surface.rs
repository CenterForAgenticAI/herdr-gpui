#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Desktop-region messages: the frozen surface fixture, round trips, and the
//! limits that keep a hostile server's frame away from the painter.

use herdr_protocol::{
    endpoint::EndpointClientHello,
    region_surface::{
        ClientRegion, PROJECTION_KIND, RegionProjection, RegionSurface, RegionViewport,
        SURFACE_KIND, VIEWPORT_KIND,
    },
    *,
};
use sha2::{Digest, Sha256};

fn cell(symbol: &str) -> CellData {
    CellData {
        symbol: symbol.into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    }
}

/// Herdr's fixture: a 4x2 ratatui buffer with `r` at (0, 0) and `g` at (3, 1).
fn sample_surface() -> RegionSurface {
    let mut cells = vec![cell(" "); 8];
    cells[0] = cell("r");
    cells[7] = cell("g");
    RegionSurface {
        region_id: "region_1".into(),
        terminal_id: "term_1".into(),
        frame: FrameData {
            cells,
            width: 4,
            height: 2,
            cursor: Some(CursorState {
                x: 1,
                y: 1,
                visible: true,
                shape: 0,
            }),
            hyperlinks: vec![],
            graphics: vec![],
        },
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn control_data(message: ServerMessage) -> (String, String) {
    match message {
        ServerMessage::EndpointControl { kind, data } => (kind, data),
        other => panic!("expected endpoint control, got {other:?}"),
    }
}

fn sample_region() -> ClientRegion {
    ClientRegion {
        region_id: "region_1".into(),
        terminal_id: "term_1".into(),
        plugin_id: "board".into(),
        entrypoint: "board".into(),
        title: "Board".into(),
        edge: "left".into(),
        scope: "session".into(),
        size: 36,
        min_size: 12,
        max_size: 60,
        visible: true,
        focused: false,
        mouse_reporting: false,
    }
}

#[test]
fn region_surface_round_trips_through_endpoint_control() {
    let surface = sample_surface();
    let (kind, data) = control_data(surface.message().unwrap());
    assert_eq!(kind, "endpoint.region-surface.v1");
    assert_eq!(kind, SURFACE_KIND);
    assert_eq!(RegionSurface::decode(&data).unwrap(), surface);
}

#[test]
fn region_surface_v1_binary_fixture_is_frozen() {
    let mut bytes = Vec::new();
    write_message(
        &mut bytes,
        &sample_surface().message().unwrap(),
        MAX_FRAME_SIZE,
    )
    .unwrap();
    assert_eq!(
        hex(&Sha256::digest(bytes)),
        "695910daf340627cd79d5d1f952f1b05ff19818403cc1f5bba36ead1dfcebfbd"
    );
}

#[test]
fn region_surface_rejects_a_grid_that_does_not_match_its_size() {
    let mut surface = sample_surface();
    surface.frame.width = 40;
    let (_, data) = control_data(surface.message().unwrap());
    assert!(matches!(
        RegionSurface::decode(&data),
        Err(Error::RegionGrid)
    ));
}

#[test]
fn region_surface_rejects_grids_past_the_dimension_limit() {
    let mut surface = sample_surface();
    surface.frame.width = 4097;
    surface.frame.height = 0;
    surface.frame.cells.clear();
    surface.frame.cursor = None;
    let (_, data) = control_data(surface.message().unwrap());
    assert!(matches!(
        RegionSurface::decode(&data),
        Err(Error::RegionGrid)
    ));
}

#[test]
fn region_surface_rejects_a_cursor_outside_its_grid() {
    let mut surface = sample_surface();
    surface.frame.cursor = Some(CursorState {
        x: 9,
        y: 0,
        visible: true,
        shape: 0,
    });
    let (_, data) = control_data(surface.message().unwrap());
    assert!(RegionSurface::decode(&data).is_err());
}

#[test]
fn region_surface_rejects_oversized_payloads_before_decoding() {
    let data = "A".repeat(base64::encoded_len(MAX_FRAME_SIZE, false).unwrap() + 1);
    let error = RegionSurface::decode(&data).unwrap_err();
    assert!(matches!(error, Error::RegionLimit));
    assert_eq!(error.to_string(), "region surface exceeds the frame limit");
}

#[test]
fn region_surface_rejects_trailing_bytes_and_bad_base64() {
    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    let (_, data) = control_data(sample_surface().message().unwrap());
    let mut bytes = STANDARD_NO_PAD.decode(&data).unwrap();
    bytes.push(0);
    assert!(matches!(
        RegionSurface::decode(&STANDARD_NO_PAD.encode(bytes)),
        Err(Error::TrailingBytes)
    ));
    assert!(matches!(
        RegionSurface::decode("not base64!"),
        Err(Error::EncodingPayload(_))
    ));
}

#[test]
fn projection_round_trips_and_tolerates_unknown_edges() {
    let projection = RegionProjection {
        boot_id: "boot".into(),
        revision: 3,
        regions: vec![sample_region()],
        focused: None,
    };
    let (kind, data) = control_data(projection.message().unwrap());
    assert_eq!(kind, "endpoint.regions.v1");
    assert_eq!(kind, PROJECTION_KIND);
    assert_eq!(RegionProjection::decode(&data).unwrap(), projection);
    let mut value: serde_json::Value = serde_json::from_str(&data).unwrap();
    value["regions"][0]["edge"] = "diagonal".into();
    value["future"] = true.into();
    let future = RegionProjection::decode(&value.to_string()).unwrap();
    assert_eq!(future.regions[0].edge, "diagonal");
}

#[test]
fn projection_optional_fields_default() {
    let projection = RegionProjection::decode(r#"{"boot_id":"b","revision":1}"#).unwrap();
    assert!(projection.regions.is_empty());
    assert_eq!(projection.focused, None);
    assert!(RegionProjection::decode(r#"{"revision":1}"#).is_err());
}

#[test]
fn viewport_round_trips_and_rejects_bad_shapes() {
    let viewport = RegionViewport {
        region_id: "region_1".into(),
        cols: 34,
        rows: 22,
    };
    let ClientMessage::EndpointControl { kind, data } = viewport.message().unwrap() else {
        panic!("expected endpoint control");
    };
    assert_eq!(kind, "endpoint.region-viewport.v1");
    assert_eq!(kind, VIEWPORT_KIND);
    assert_eq!(RegionViewport::decode(&data).unwrap(), viewport);
    assert!(RegionViewport::decode(r#"{"region_id":7}"#).is_err());
}

#[test]
fn hello_regions_defaults_off_and_round_trips() {
    let mut hello: EndpointClientHello =
        serde_json::from_str(include_str!("fixtures/endpoint-hello-v1.json")).unwrap();
    assert!(!hello.regions);
    hello.regions = true;
    let value = serde_json::to_value(&hello).unwrap();
    assert_eq!(value["regions"], true);
    let back: EndpointClientHello = serde_json::from_value(value).unwrap();
    assert!(back.regions);
}
