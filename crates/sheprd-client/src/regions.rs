//! What a window knows about the desktop regions of its connection: the
//! latest projection and the latest frame of each region. Pure state, fed by
//! [`ClientEvent::RegionProjection`](crate::ClientEvent::RegionProjection) and
//! [`ClientEvent::RegionSurface`](crate::ClientEvent::RegionSurface), so the
//! revision and boot rules hold without a socket or a window.

mod error;
mod handle;

pub use error::Error;

use crate::protocol::{
    FrameData,
    region_surface::{ClientRegion, RIGHT_EDGE, RegionProjection, RegionSurface},
};
use std::{collections::HashMap, sync::Arc};

/// Frames kept at once. A server holds at most a few regions; more frames
/// than this are a hostile or broken peer, not a layout.
pub const MAX_SURFACES: usize = 8;

#[derive(Debug, Default, Clone)]
pub struct RegionState {
    projection: Option<Arc<RegionProjection>>,
    surfaces: HashMap<String, Arc<RegionSurface>>,
}

impl RegionState {
    pub fn projection(&self) -> Option<&RegionProjection> {
        self.projection.as_deref()
    }

    /// Takes `next` unless it is older than or the same as the projection
    /// held for the same boot. A new boot replaces everything. Returns whether
    /// it was taken.
    pub fn apply_projection(&mut self, next: Arc<RegionProjection>) -> bool {
        if let Some(current) = &self.projection {
            if current.boot_id == next.boot_id && next.revision <= current.revision {
                return false;
            }
            if current.boot_id != next.boot_id {
                self.surfaces.clear();
            }
        }
        // Frames of regions that are gone or hidden are stale when they return.
        self.surfaces.retain(|id, _| {
            next.regions
                .iter()
                .any(|region| region.region_id == *id && region.visible)
        });
        self.projection = Some(next);
        true
    }

    /// Keeps the latest frame per region. A frame may arrive before the
    /// projection that names it, so it is kept, within [`MAX_SURFACES`].
    pub fn apply_surface(&mut self, surface: Arc<RegionSurface>) -> bool {
        if !self.surfaces.contains_key(&surface.region_id) && self.surfaces.len() >= MAX_SURFACES {
            let named = |id: &str| {
                self.projection
                    .as_ref()
                    .is_some_and(|p| p.regions.iter().any(|r| r.region_id == id))
            };
            let Some(evict) = self.surfaces.keys().find(|id| !named(id)).cloned() else {
                return false;
            };
            self.surfaces.remove(&evict);
        }
        self.surfaces.insert(surface.region_id.clone(), surface);
        true
    }

    /// The region on the right edge, shown or not. Only that edge is drawn.
    pub fn right(&self) -> Option<&ClientRegion> {
        self.projection
            .as_ref()?
            .regions
            .iter()
            .find(|region| region.edge == RIGHT_EDGE)
    }

    /// The right-edge region when it is visible.
    pub fn visible_right(&self) -> Option<&ClientRegion> {
        self.right().filter(|region| region.visible)
    }

    /// The latest frame of `region`, only when it came from that region's
    /// own terminal.
    pub fn frame(&self, region: &ClientRegion) -> Option<&FrameData> {
        self.surfaces
            .get(&region.region_id)
            .filter(|surface| surface.terminal_id == region.terminal_id)
            .map(|surface| &surface.frame)
    }

    /// Whether the projection says `region` holds the keyboard.
    pub fn is_focused(&self, region: &ClientRegion) -> bool {
        region.visible
            && (region.focused
                || self.projection.as_ref().and_then(|p| p.focused.as_deref())
                    == Some(region.region_id.as_str()))
    }

    /// The terminal that key input goes to instead of a pane: the visible
    /// right region, while the projection says it is focused.
    pub fn focused_terminal(&self) -> Option<&str> {
        self.visible_right()
            .filter(|region| self.is_focused(region))
            .map(|region| region.terminal_id.as_str())
    }
}

#[cfg(test)]
mod tests;
