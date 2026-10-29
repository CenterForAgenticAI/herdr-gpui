//! Desktop regions on one connection: whether the hello asked for them, whether
//! the welcome agreed, and decoding the two server kinds into events. Region
//! traffic a connection did not negotiate is dropped, and so is a malformed
//! region message: a bad side panel never ends the connection.

use crate::{
    Result,
    event::ClientEvent,
    protocol::{
        endpoint::EndpointServerWelcome,
        region_surface::{
            CAPABILITY, PROJECTION_KIND, RegionProjection, RegionSurface, SURFACE_KIND,
        },
    },
};
use std::sync::Arc;

/// What a connection asks for in its hello beyond its geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    /// Whether the daemon should stream this client's surface from the start.
    pub surface_active: bool,
    /// Ask for desktop regions. One connection per window should; a
    /// connection kept for another group never does.
    pub regions: bool,
}

impl Default for Hello {
    fn default() -> Self {
        Self {
            surface_active: true,
            regions: false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Regions {
    /// The hello set `regions`.
    pub(crate) requested: bool,
    /// The hello asked and the welcome advertised [`CAPABILITY`].
    pub(crate) negotiated: bool,
}

impl Regions {
    pub(crate) fn accept_welcome(&mut self, welcome: &EndpointServerWelcome) {
        self.negotiated = self.requested && welcome.capabilities.iter().any(|c| c == CAPABILITY);
    }

    pub(crate) fn receive(
        &self,
        kind: &str,
        data: &str,
        mut emit: impl FnMut(ClientEvent) -> Result<()>,
    ) -> Result<()> {
        if !self.negotiated {
            tracing::debug!(kind, "region control ignored: not negotiated");
            return Ok(());
        }
        let event = if kind == PROJECTION_KIND {
            RegionProjection::decode(data).map(|p| ClientEvent::RegionProjection(Arc::new(p)))
        } else {
            RegionSurface::decode(data).map(|s| ClientEvent::RegionSurface(Arc::new(s)))
        };
        match event {
            Ok(event) => emit(event),
            Err(error) => {
                tracing::debug!(kind, %error, "malformed region control dropped");
                Ok(())
            }
        }
    }
}

/// Whether `kind` is one of the region messages a server sends.
pub(crate) fn server_kind(kind: &str) -> bool {
    kind == PROJECTION_KIND || kind == SURFACE_KIND
}
