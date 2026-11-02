//! Desktop regions: a plugin's terminal docked on the window's right edge,
//! drawn from the daemon's region projection and frames (`region_surface`).
//! The viewed machine's region wins the edge; while another machine is
//! viewed, this Mac's own region fills it when that machine has none (see
//! `source`). A window without the capability lays out exactly as before.
//! The panel follows the daemon:
//! it shows a region only while the projection says it is visible, and the
//! keyboard goes to it only while the projection also says it is focused.
//! Regions on other edges are not drawn and get no viewport report.
// i18n: migrated

mod render;
mod source;
#[cfg(test)]
mod tests;

use source::Owner;
pub(crate) use source::moves_focus;

use crate::{SheprdWindow, i18n::fl, motion, navigation::NavigationTarget, window::Flash};
use gpui::{Bounds, Context, Pixels, Window};
use sheprd_client::{
    ClientHandle, RegionState, SendError,
    protocol::{
        ClientPaneInputEvent,
        region_surface::{ClientRegion, RegionCommand, RegionViewport},
    },
};
use std::{collections::HashMap, time::Instant};

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
    /// The region drawn now and whose it is; typed input may go to it.
    drawn: Option<(Owner, String)>,
    /// The region last drawn, kept to slide it out.
    leaving: Option<Shown>,
    /// A local region holding the keyboard while another machine is viewed.
    /// The window keeps this focus: that server is not the one being viewed.
    local_focus: Option<String>,
    /// The last viewport sent to each endpoint, with the boot it went to.
    reported: HashMap<String, (String, RegionViewport)>,
    slide: motion::Toggle,
}

/// A region the panel draws: whose it is, the region, and its width.
type Shown = (Owner, ClientRegion, u16);

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
    /// Lets go of a local region's keyboard focus. True when it held it.
    pub(crate) fn release_local_focus(&mut self) -> bool {
        self.local_focus.take().is_some()
    }

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

impl SheprdWindow {
    /// The right region this window draws now, whose it is, and its width
    /// in cells.
    fn desktop_region_layout(&self, window: &Window) -> Option<Shown> {
        let owner = self.right_owner()?;
        let region = self.source_regions(owner)?.visible_right()?;
        let size = self
            .desktop_region
            .drag
            .map_or(region.size, |drag| drag.size);
        columns(region, size, self.desktop_region_row(window))
            .map(|cols| (owner, region.clone(), cols))
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
    /// input may go to, and a zero viewport for each right region not drawn.
    fn track_desktop_region(&mut self, layout: Option<&Shown>, now: Instant) {
        let owner = layout.map_or(Owner::Viewed, |(owner, ..)| *owner);
        let boot = self
            .source_regions(owner)
            .and_then(RegionState::projection)
            .map(|projection| projection.boot_id.clone());
        let first = self.desktop_region.boot != boot;
        self.desktop_region.boot = boot;
        self.desktop_region.slide.set(layout.is_some(), now, first);
        self.desktop_region.drawn =
            layout.map(|(owner, region, _)| (*owner, region.region_id.clone()));
        if let Some(shown) = layout {
            self.desktop_region.leaving = Some(shown.clone());
        } else if !self.desktop_region.slide.moving(now) {
            self.desktop_region.leaving = None;
        }
        // A right region hidden, or covered by the viewed machine's, has no
        // room here; the drawn one reports its cells as it is placed.
        for owner in Owner::ALL {
            if layout.is_some_and(|(drawn, ..)| *drawn == owner) {
                continue;
            }
            let hidden = self
                .source_regions(owner)
                .and_then(RegionState::right)
                .map(|region| RegionViewport {
                    region_id: region.region_id.clone(),
                    cols: 0,
                    rows: 0,
                });
            if let Some(hidden) = hidden {
                self.report_desktop_region(owner, hidden);
            }
        }
    }

    /// Whether the panel is still sliding in or out.
    pub(crate) fn desktop_region_moving(&self) -> bool {
        self.desktop_region.slide.moving(motion::now())
    }

    /// Tells `owner`'s server the cells this window draws for its region,
    /// once per change and connection.
    fn report_desktop_region(&mut self, owner: Owner, viewport: RegionViewport) {
        if self.source_regions(owner).is_none() {
            return;
        }
        let Some((endpoint, handle, boot)) = self.region_connection(owner) else {
            return;
        };
        let sent = (boot.to_owned(), viewport);
        if self.desktop_region.reported.get(endpoint) == Some(&sent) {
            return;
        }
        let result = handle.send_region_viewport(&sent.0, &sent.1);
        let endpoint = endpoint.to_owned();
        match result {
            Ok(()) => {
                self.desktop_region.reported.insert(endpoint, sent);
            }
            Err(error) => tracing::debug!(%error, "Region viewport not sent"),
        }
    }

    /// The region terminal typed input goes to: the drawn right region,
    /// while it holds the keyboard.
    pub(crate) fn region_input_target(&self) -> Option<&str> {
        let (owner, drawn) = self.desktop_region.drawn.as_ref()?;
        let region = self.source_regions(*owner)?.visible_right()?;
        (drawn == &region.region_id && self.region_focused(*owner, region))
            .then_some(region.terminal_id.as_str())
    }

    /// Sends `event` to the drawn region's terminal `terminal_id`, on the
    /// connection of the server running it. False when it was not sent and
    /// the window said why.
    pub(crate) fn send_region_input(
        &mut self,
        terminal_id: &str,
        event: ClientPaneInputEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let owner = self.drawn_owner().unwrap_or(Owner::Viewed);
        let Some((_, handle, boot)) = self.region_connection(owner) else {
            return true;
        };
        if let Err(error) = handle.send_region_input(boot, terminal_id, [event]) {
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

    /// Queues one region request on the viewed machine's connection; a
    /// failure shows in the window.
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

    /// Sends `command` for a region of this Mac's server while another
    /// machine is viewed; a failure shows in the window.
    fn region_command(&mut self, command: RegionCommand, cx: &mut Context<Self>) -> bool {
        let Some((_, handle, boot)) = self.region_connection(Owner::Local) else {
            return false;
        };
        if let Err(error) = handle.send_region_command(boot, &command) {
            self.local_error = Some(fl!(
                "desktop-region-request-failed",
                error = error.to_string()
            ));
            cx.notify();
            return false;
        }
        true
    }

    /// The right region a command acts on and whose it is, or a flash
    /// saying why there is none: the region drawn, else the viewed machine's
    /// hidden one, else this Mac's.
    fn commanded_region(&mut self, cx: &mut Context<Self>) -> Option<(Owner, ClientRegion)> {
        self.live.snapshot.as_ref()?;
        let owner = self.right_owner().or_else(|| {
            Owner::ALL.into_iter().find(|owner| {
                self.source_regions(*owner)
                    .and_then(RegionState::right)
                    .is_some()
            })
        });
        let found =
            owner.and_then(|owner| Some((owner, self.source_regions(owner)?.right()?.clone())));
        if found.is_none() {
            let offered = Owner::ALL
                .into_iter()
                .any(|owner| self.source_regions(owner).is_some());
            let text = match offered {
                true => fl!("desktop-region-none"),
                false => fl!("desktop-region-unsupported"),
            };
            self.show_flash(Flash::warning(text), cx);
        }
        found
    }

    /// Hides the right region, or shows it again.
    pub(crate) fn toggle_desktop_region(&mut self, cx: &mut Context<Self>) {
        let Some((owner, region)) = self.commanded_region(cx) else {
            return;
        };
        let region_id = region.region_id;
        match (owner, region.visible) {
            (Owner::Local, true) => {
                self.region_command(RegionCommand::Hide { region_id }, cx);
            }
            (Owner::Local, false) => {
                self.region_command(RegionCommand::Show { region_id }, cx);
            }
            (Owner::Viewed, visible) => {
                self.region_request(
                    |handle, boot| match visible {
                        true => handle.hide_region(boot, &region_id),
                        false => handle.show_region(boot, &region_id),
                    },
                    cx,
                );
            }
        }
    }

    /// Hides `owner`'s region `region_id`, as its header's button does.
    fn hide_desktop_region(&mut self, owner: Owner, region_id: &str, cx: &mut Context<Self>) {
        match owner {
            Owner::Viewed => {
                self.region_request(|handle, boot| handle.hide_region(boot, region_id), cx);
            }
            Owner::Local => {
                let region_id = region_id.to_owned();
                self.region_command(RegionCommand::Hide { region_id }, cx);
            }
        }
    }

    /// Gives the keyboard to the right region, showing it first when it is
    /// hidden, or back to the focused pane when the region has it.
    pub(crate) fn focus_desktop_region(&mut self, cx: &mut Context<Self>) {
        let Some((owner, region)) = self.commanded_region(cx) else {
            return;
        };
        if owner == Owner::Local {
            let focused = self.region_focused(owner, &region);
            let region_id = region.region_id;
            if !region.visible
                && !self.region_command(
                    RegionCommand::Show {
                        region_id: region_id.clone(),
                    },
                    cx,
                )
            {
                return;
            }
            self.desktop_region.local_focus = (!focused).then_some(region_id);
            cx.notify();
            return;
        }
        self.release_local_region(cx);
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
