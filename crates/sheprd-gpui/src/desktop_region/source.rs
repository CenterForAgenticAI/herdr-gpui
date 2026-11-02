//! Whose region the right edge shows. The viewed machine's region wins its
//! edge. While another machine is viewed, this Mac's own server fills the
//! edge it leaves free, when that server keeps streaming its regions to a
//! connection that is not being viewed (`region_background`). The window,
//! not that server, holds keyboard focus on such a region, and it sends
//! that server region commands, not requests, which it would refuse.

use super::Panel;
use crate::{
    SheprdWindow,
    endpoint::{Endpoint, LOCAL},
};
use gpui::Context;
use serde_json::Value;
use sheprd_client::{ClientHandle, Method, RegionState, protocol::region_surface::ClientRegion};

/// Whose region the panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    /// The machine the window views.
    Viewed,
    /// This Mac's server, while another machine is viewed.
    Local,
}

impl Owner {
    /// In the order they claim an edge.
    pub(super) const ALL: [Self; 2] = [Self::Viewed, Self::Local];
}

/// Whether `method` with `params` moves the keyboard to a workspace, tab or
/// pane of the viewed machine, which takes it off a local region.
pub(crate) fn moves_focus(method: Method, params: &Value) -> bool {
    match method {
        Method::WorkspaceFocus
        | Method::TabFocus
        | Method::PaneFocus
        | Method::PaneFocusDirection => true,
        Method::WorkspaceCreate | Method::TabCreate | Method::PaneSplit => params["focus"] == true,
        _ => false,
    }
}

impl SheprdWindow {
    /// The regions `owner` offers now, or `None` when it offers none.
    pub(super) fn source_regions(&self, owner: Owner) -> Option<&RegionState> {
        let live = match owner {
            Owner::Viewed => &self.live,
            Owner::Local => &self.local_background()?.live,
        };
        live.supports_regions.then_some(&live.regions)
    }

    /// This Mac's endpoint, while another machine is viewed and this Mac's
    /// server streams its regions in the background.
    fn local_background(&self) -> Option<&Endpoint> {
        if self.selected_endpoint == 0 {
            return None;
        }
        self.endpoints.first().filter(|endpoint| {
            endpoint.id == LOCAL
                && endpoint.live.supports_regions
                && endpoint.live.supports_region_background
        })
    }

    /// The endpoint id, handle and boot that `owner`'s region traffic goes to.
    pub(super) fn region_connection(&self, owner: Owner) -> Option<(&str, &ClientHandle, &str)> {
        let (endpoint, live) = match owner {
            Owner::Viewed => (self.endpoints.get(self.selected_endpoint)?, &self.live),
            Owner::Local => {
                let endpoint = self.local_background()?;
                (endpoint, &endpoint.live)
            }
        };
        Some((
            endpoint.id.as_str(),
            endpoint.connection.handle.as_ref()?,
            live.snapshot.as_ref()?.boot_id.as_str(),
        ))
    }

    /// Whose region the right edge shows: the viewed machine's, else this
    /// Mac's.
    pub(super) fn right_owner(&self) -> Option<Owner> {
        Owner::ALL.into_iter().find(|owner| {
            self.source_regions(*owner)
                .and_then(RegionState::visible_right)
                .is_some()
        })
    }

    /// Whose region the panel drew this frame.
    pub(super) fn drawn_owner(&self) -> Option<Owner> {
        self.desktop_region.drawn.as_ref().map(|(owner, _)| *owner)
    }

    /// Whether `owner`'s `region` holds the keyboard: as its server says for
    /// the viewed machine, as this window says for this Mac's.
    pub(super) fn region_focused(&self, owner: Owner, region: &ClientRegion) -> bool {
        match owner {
            Owner::Viewed => self.live.regions.is_focused(region),
            Owner::Local => {
                region.visible
                    && self.desktop_region.local_focus.as_deref() == Some(region.region_id.as_str())
            }
        }
    }

    /// Takes the keyboard off a local region, as clicking a pane does.
    pub(crate) fn release_local_region(&mut self, cx: &mut Context<Self>) {
        if Panel::release_local_focus(&mut self.desktop_region) {
            cx.notify();
        }
    }
}
