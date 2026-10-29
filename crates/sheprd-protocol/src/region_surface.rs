//! Adapted from herdr src/protocol/region_surface.rs (Apache-2.0, see
//! NOTICE.md). Modified: decoders return this crate's typed errors and check
//! decoded frames with [`FrameData::validate`]; encoders are kept for mock
//! peers and the frozen fixture.
//!
//! Optional desktop-region projection, surface and viewport messages. Every
//! message rides the frozen `EndpointControl { kind, data }` variant. A server
//! sends region traffic only to a client whose hello set `regions`, and a
//! client sends viewports and region input only to a server that advertised
//! [`CAPABILITY`]. Unknown kinds are ignored on both sides.

use crate::{
    ClientMessage, Error, FrameData, MAX_FRAME_SIZE, Result, ServerMessage, decode_payload,
    encode_message,
};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};

pub const CAPABILITY: &str = "region_surface";
/// Server to client, JSON [`RegionProjection`].
pub const PROJECTION_KIND: &str = "endpoint.regions.v1";
/// Server to client, base64 (no padding) of bincode [`RegionSurface`].
pub const SURFACE_KIND: &str = "endpoint.region-surface.v1";
/// Client to server, JSON [`RegionViewport`].
pub const VIEWPORT_KIND: &str = "endpoint.region-viewport.v1";
/// The edge this client draws. Regions on other edges are skipped.
pub const RIGHT_EDGE: &str = "right";

const MAX_GRID_DIMENSION: u16 = 4096;
const MAX_GRID_CELLS: usize = 1_000_000;

/// The regions one server holds, as a client needs them for layout and input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionProjection {
    pub boot_id: String,
    pub revision: u64,
    #[serde(default)]
    pub regions: Vec<ClientRegion>,
    #[serde(default)]
    pub focused: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRegion {
    pub region_id: String,
    pub terminal_id: String,
    pub plugin_id: String,
    pub entrypoint: String,
    pub title: String,
    /// Edge name, kept as text so an unknown edge is skipped, not fatal.
    pub edge: String,
    pub scope: String,
    /// Requested width in cells.
    pub size: u16,
    pub min_size: u16,
    pub max_size: u16,
    pub visible: bool,
    pub focused: bool,
    /// The region terminal asked for mouse reports.
    #[serde(default)]
    pub mouse_reporting: bool,
}

/// Binary layout frozen with [`SURFACE_KIND`]; field order is the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionSurface {
    pub region_id: String,
    pub terminal_id: String,
    pub frame: FrameData,
}

/// Inner size a client drew for a region. `cols == 0` means hidden there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionViewport {
    pub region_id: String,
    pub cols: u16,
    pub rows: u16,
}

impl RegionProjection {
    pub fn decode(data: &str) -> Result<Self> {
        Ok(serde_json::from_str(data)?)
    }

    /// The message a server would send; used by mock peers.
    pub fn message(&self) -> Result<ServerMessage> {
        Ok(ServerMessage::EndpointControl {
            kind: PROJECTION_KIND.into(),
            data: serde_json::to_string(self)?,
        })
    }
}

impl RegionSurface {
    /// Decodes one region surface. Oversized payloads and frames whose cells
    /// do not match their size are rejected before they reach a painter.
    pub fn decode(data: &str) -> Result<Self> {
        let max_encoded = base64::encoded_len(MAX_FRAME_SIZE, false).unwrap_or(usize::MAX);
        if data.len() > max_encoded {
            return Err(Error::RegionLimit);
        }
        let surface: Self = decode_payload(&STANDARD_NO_PAD.decode(data)?)?;
        let frame = &surface.frame;
        let cells = usize::from(frame.width) * usize::from(frame.height);
        if frame.width > MAX_GRID_DIMENSION
            || frame.height > MAX_GRID_DIMENSION
            || cells > MAX_GRID_CELLS
            || frame.cells.len() != cells
        {
            return Err(Error::RegionGrid);
        }
        frame.validate()?;
        Ok(surface)
    }

    /// The message a server would send; used by mock peers.
    pub fn message(&self) -> Result<ServerMessage> {
        let bytes = encode_message(self, MAX_FRAME_SIZE)?;
        Ok(ServerMessage::EndpointControl {
            kind: SURFACE_KIND.into(),
            data: STANDARD_NO_PAD.encode(&bytes[4..]),
        })
    }
}

impl RegionViewport {
    pub fn decode(data: &str) -> Result<Self> {
        Ok(serde_json::from_str(data)?)
    }

    /// The client message reporting this viewport.
    pub fn message(&self) -> Result<ClientMessage> {
        Ok(ClientMessage::EndpointControl {
            kind: VIEWPORT_KIND.into(),
            data: serde_json::to_string(self)?,
        })
    }
}
