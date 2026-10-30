//! Desktop regions: a plugin's terminal docked on the window's right edge,
//! drawn from the daemon's region projection and frames (`region_surface`).
//! Only the window's own connection negotiates them, so a window without
//! the capability lays out exactly as before. The panel follows the daemon:
//! it shows a region only while the projection says it is visible, and the
//! keyboard goes to it only while the projection also says it is focused.
//! Regions on other edges are not drawn and get no viewport report.
// i18n: migrated

mod render;
#[cfg(test)]
mod tests;

use crate::{HerdrWindow, i18n::fl, motion, navigation::NavigationTarget, window::Flash};
use gpui::{Bounds, Context, Pixels, Window};
use herdr_client::{
    ClientHandle, SendError,
    protocol::{
        ClientPaneInputEvent,
        region_surface::{ClientRegion, RegionViewport},
    },
};
use std::time::Instant;

/// Columns the panes keep beside a region. Herdr's own layout hides a
/// region that would leave fewer (`MIN_TAB_SURFACE_WIDTH`).
pub(crate) const MIN_PANE_COLUMNS: u16 = 20;
/// Rows a region needs to be drawn at all, as in Herdr (`MIN_REGION_HEIGHT`).
pub(crate) const MIN_ROWS: u16 = 3;
/// The line between the panes and the panel.
const BORDER: f32 = 1.;

/// A window's right-edge panel: what it drew, what it told the daemon, and
/// the drag that is resizing it.
#[derive(Debug, Default)]
pub(crate) struct Panel {
    /// Where the region's terminal was last laid out.
    bounds: Bounds<Pixels>,
    /// The projection's boot the panel last saw. A new one shows the panel
    /// where it is, without sliding.
    boot: Option<String>,
    drag: Option<Drag>,
    /// The region drawn now, which typed input may go to.
    drawn: Option<String>,
    /// The region last drawn and its width, kept to slide it out.
    leaving: Option<(ClientRegion, u16)>,
    /// The last viewport sent, with the boot it was sent to.
    reported: Option<(String, RegionViewport)>,
    slide: motion::Toggle,
}

/// A size the user dragged to, drawn until the daemon's projection catches
/// up with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Drag {
    size: u16,
    /// The projection revision when the button was released; a newer
    /// projection carries the daemon's answer.
    released: Option<u64>,
}

impl Panel {
    /// Forgets a released drag once a newer projection has arrived.
    fn settle(&mut self, revision: Option<u64>) {
        if let Some(Drag {
            released: Some(at), ..
        }) = self.drag
            && revision != Some(at)
        {
            self.drag = None;
        }
    }
}

/// The width in cells a region is drawn at, `size` held to its own
/// minimum and maximum, in a row `row` cells wide; `None` when it is zero or
/// would leave the panes fewer than [`MIN_PANE_COLUMNS`]. A daemon's size
/// is data: a hostile one can hide the panel but never squeeze the panes.
pub(crate) fn columns(region: &ClientRegion, size: u16, row: u16) -> Option<u16> {
    if size == 0 {
        return None;
    }
    let size = bounded(region, size);
    (row.checked_sub(size)? >= MIN_PANE_COLUMNS).then_some(size)
}

/// The size a drag to `wanted` cells asks for: within the region's bounds
/// and leaving the panes their minimum, or `None` when no size fits.
pub(crate) fn dragged(region: &ClientRegion, wanted: f32, row: u16) -> Option<u16> {
    if !wanted.is_finite() {
        return None;
    }
    let room = row.checked_sub(MIN_PANE_COLUMNS)?;
    let wanted = wanted.round().clamp(0., f32::from(u16::MAX)) as u16;
    columns(region, bounded(region, wanted).min(room), row)
}

/// `size` within the region's minimum and maximum, read in whichever order
/// they came, and never zero.
fn bounded(region: &ClientRegion, size: u16) -> u16 {
    let low = region.min_size.min(region.max_size).max(1);
    let high = region.max_size.max(region.min_size).max(low);
    size.clamp(low, high)
}

impl HerdrWindow {
    /// The right region this window draws now, and its width in cells.
    fn desktop_region_layout(&self, window: &Window) -> Option<(ClientRegion, u16)> {
        if !self.live.supports_regions {
            return None;
        }
        let region = self.live.regions.visible_right()?;
        let size = self
            .desktop_region
            .drag
            .map_or(region.size, |drag| drag.size);
        columns(region, size, self.desktop_region_row(window)).map(|cols| (region.clone(), cols))
    }

    /// The cells across the window body beside the sidebar, which the panes
    /// and the panel share.
    fn desktop_region_row(&self, window: &Window) -> u16 {
        let width = f32::from(window.viewport_size().width);
        let sidebar = self.sidebar_column_width(width).unwrap_or(0.);
        let padding = self.config.terminal_padding.x;
        let cells =
            (width - sidebar - padding.start() - padding.end() - BORDER) / self.cell_width.max(1.);
        cells.floor().clamp(0., 4096.) as u16
    }

    /// Records what this frame draws: the slide toward it, the region typed
    /// input may go to, and, when nothing is drawn, a zero viewport.
    fn track_desktop_region(&mut self, layout: Option<&(ClientRegion, u16)>, now: Instant) {
        let projection = self.live.regions.projection();
        let boot = projection.map(|projection| projection.boot_id.clone());
        let first = self.desktop_region.boot != boot;
        self.desktop_region.boot = boot;
        self.desktop_region.slide.set(layout.is_some(), now, first);
        self.desktop_region.drawn = layout.map(|(region, _)| region.region_id.clone());
        if let Some(shown) = layout {
            self.desktop_region.leaving = Some(shown.clone());
            return;
        }
        if !self.desktop_region.slide.moving(now) {
            self.desktop_region.leaving = None;
        }
        let hidden = self
            .live
            .regions
            .right()
            .filter(|_| self.live.supports_regions)
            .map(|region| RegionViewport {
                region_id: region.region_id.clone(),
                cols: 0,
                rows: 0,
            });
        if let Some(hidden) = hidden {
            self.report_desktop_region(hidden);
        }
    }

    /// Whether the panel is still sliding in or out.
    pub(crate) fn desktop_region_moving(&self) -> bool {
        self.desktop_region.slide.moving(motion::now())
    }

    /// Tells the daemon the cells this window draws for a region, once per
    /// change and connection.
    fn report_desktop_region(&mut self, viewport: RegionViewport) {
        if !self.live.supports_regions {
            return;
        }
        let (Some(handle), Some(snapshot)) = (
            &self.endpoints[self.selected_endpoint].connection.handle,
            &self.live.snapshot,
        ) else {
            return;
        };
        let sent = (snapshot.boot_id.clone(), viewport);
        if self.desktop_region.reported.as_ref() == Some(&sent) {
            return;
        }
        match handle.send_region_viewport(&sent.0, &sent.1) {
            Ok(()) => self.desktop_region.reported = Some(sent),
            Err(error) => tracing::debug!(%error, "Region viewport not sent"),
        }
    }

    /// The region terminal typed input goes to: the drawn right region,
    /// while the projection says it holds the keyboard.
    pub(crate) fn region_input_target(&self) -> Option<&str> {
        if !self.live.supports_regions {
            return None;
        }
        let region = self.live.regions.visible_right()?;
        (self.desktop_region.drawn.as_deref() == Some(region.region_id.as_str())
            && self.live.regions.is_focused(region))
        .then_some(region.terminal_id.as_str())
    }

    /// Sends `event` to the region terminal `terminal_id`. False when it
    /// was not sent and the window said why.
    pub(crate) fn send_region_input(
        &mut self,
        terminal_id: &str,
        event: ClientPaneInputEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let (Some(handle), Some(snapshot)) = (
            &self.endpoints[self.selected_endpoint].connection.handle,
            &self.live.snapshot,
        ) else {
            return true;
        };
        if let Err(error) = handle.send_region_input(&snapshot.boot_id, terminal_id, [event]) {
            self.local_error = Some(fl!(
                "desktop-region-input-failed",
                error = error.to_string()
            ));
            cx.notify();
            return false;
        }
        self.note_input();
        true
    }

    /// Queues one region request on the window's connection; a failure
    /// shows in the window.
    fn region_request(
        &mut self,
        request: impl FnOnce(&ClientHandle, &str) -> Result<String, SendError>,
        cx: &mut Context<Self>,
    ) -> bool {
        let (Some(handle), Some(snapshot)) = (
            &self.endpoints[self.selected_endpoint].connection.handle,
            &self.live.snapshot,
        ) else {
            return false;
        };
        if let Err(error) = request(handle, &snapshot.boot_id) {
            self.local_error = Some(fl!(
                "desktop-region-request-failed",
                error = error.to_string()
            ));
            cx.notify();
            return false;
        }
        true
    }

    /// The right region a command acts on, or a flash saying why there is
    /// none.
    fn commanded_region(&mut self, cx: &mut Context<Self>) -> Option<ClientRegion> {
        self.live.snapshot.as_ref()?;
        let region = self
            .live
            .regions
            .right()
            .filter(|_| self.live.supports_regions)
            .cloned();
        if region.is_none() {
            let text = match self.live.supports_regions {
                true => fl!("desktop-region-none"),
                false => fl!("desktop-region-unsupported"),
            };
            self.show_flash(Flash::warning(text), cx);
        }
        region
    }

    /// Hides the right region, or shows it again.
    pub(crate) fn toggle_desktop_region(&mut self, cx: &mut Context<Self>) {
        let Some(region) = self.commanded_region(cx) else {
            return;
        };
        let id = region.region_id;
        self.region_request(
            |handle, boot| match region.visible {
                true => handle.hide_region(boot, &id),
                false => handle.show_region(boot, &id),
            },
            cx,
        );
    }

    /// Hides the region `region_id`, as its header's button does.
    pub(crate) fn hide_desktop_region(&mut self, region_id: &str, cx: &mut Context<Self>) {
        self.region_request(|handle, boot| handle.hide_region(boot, region_id), cx);
    }

    /// Gives the keyboard to the right region, showing it first when it is
    /// hidden, or back to the focused pane when the region has it.
    pub(crate) fn focus_desktop_region(&mut self, cx: &mut Context<Self>) {
        let Some(region) = self.commanded_region(cx) else {
            return;
        };
        if self.live.regions.is_focused(&region) {
            self.focus_pane_from_region(cx);
            return;
        }
        let id = region.region_id;
        self.region_request(
            |handle, boot| {
                // Requests run in order, so the region is shown before focus.
                if !region.visible {
                    handle.show_region(boot, &id)?;
                }
                handle.focus_region(boot, &id)
            },
            cx,
        );
    }

    /// Returns the keyboard from a region to the focused pane: focusing a
    /// pane clears the daemon's region focus.
    pub(crate) fn focus_pane_from_region(&mut self, cx: &mut Context<Self>) {
        let pane = self
            .live
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.focused_pane_id.clone());
        if let Some(pane) = pane {
            self.navigate(NavigationTarget::Pane(&pane), cx);
        }
    }
}
