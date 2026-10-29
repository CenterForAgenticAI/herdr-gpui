//! The commands a window sends about its desktop regions. Each is marked as
//! region traffic, so a connection that did not negotiate regions refuses it
//! before anything reaches the wire.

use crate::{
    ClientHandle, Result,
    method::Method,
    protocol::{ClientMessage, ClientPaneInputEvent, region_surface::RegionViewport},
};
use serde_json::json;

impl ClientHandle {
    /// Reports the inner cells this client drew for a region; `cols == 0`
    /// says it is hidden here.
    pub fn send_region_viewport(&self, boot_id: &str, viewport: &RegionViewport) -> Result<()> {
        self.enqueue_command(boot_id, viewport.message()?, None, true)
    }

    /// Keys, text, paste, or mouse for a region's terminal, on the popup
    /// input path the daemon routes by `terminal_id`.
    pub fn send_region_input(
        &self,
        boot_id: &str,
        terminal_id: &str,
        events: impl IntoIterator<Item = ClientPaneInputEvent>,
    ) -> Result<()> {
        self.enqueue_command(
            boot_id,
            ClientMessage::ClientShellPopupInput {
                terminal_id: terminal_id.into(),
                events: events.into_iter().collect(),
            },
            None,
            true,
        )
    }

    pub fn focus_region(&self, boot_id: &str, region_id: &str) -> Result<String> {
        self.region_request(boot_id, Method::RegionFocus, region_id)
    }

    pub fn hide_region(&self, boot_id: &str, region_id: &str) -> Result<String> {
        self.region_request(boot_id, Method::RegionHide, region_id)
    }

    pub fn show_region(&self, boot_id: &str, region_id: &str) -> Result<String> {
        self.region_request(boot_id, Method::RegionShow, region_id)
    }

    /// Asks for a region `size` columns wide; the daemon clamps it too.
    pub fn resize_region(&self, boot_id: &str, region_id: &str, size: u16) -> Result<String> {
        self.request_command(
            boot_id,
            Method::RegionResize,
            json!({"region_id": region_id, "size": size}),
            true,
        )
    }

    fn region_request(&self, boot_id: &str, method: Method, region_id: &str) -> Result<String> {
        self.request_command(boot_id, method, json!({"region_id": region_id}), true)
    }
}
