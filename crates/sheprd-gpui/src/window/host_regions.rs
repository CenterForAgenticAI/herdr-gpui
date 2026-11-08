//! The window in pieces, for a host that lays them out itself: each region
//! draws through the same methods as the whole window, so state and listeners
//! stay on `SheprdWindow`. The sizes a layout needs come from [`Sizes`], since
//! only this view knows them.

use super::SheprdWindow;
use crate::fonts::StyledFont as _;
use crate::sidebar::{ActivityPanel, BAR_WIDTH, SidebarMode};
use gpui::{AnyElement, AnyView, Context, Div, Empty, Window, div, prelude::*, px, rgb};

/// A piece of the window a host places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    ActivityBar,
    /// The row over the activity bar and sidebar: the traffic lights' room,
    /// and Back and Forward.
    Header,
    /// Menus, toasts and what else draws over the window, in its coordinates,
    /// so the host paints it over the whole window, outside its layout. It takes
    /// no clicks of its own.
    Overlay,
    Sidebar,
    StatusBar,
}

/// The sizes the regions want, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    pub activity_bar: f32,
    pub header: f32,
    /// The column holding the activity bar and sidebar.
    pub left: f32,
    pub status_bar: f32,
}

impl SheprdWindow {
    pub(crate) fn region_sizes(&self, window: &Window) -> Sizes {
        let width = f32::from(window.viewport_size().width);
        Sizes {
            activity_bar: if self.activity_bar_leads() {
                BAR_WIDTH
            } else {
                0.
            },
            header: crate::titlebar::HEIGHT,
            left: self.sidebar_column_width(width).unwrap_or(0.),
            status_bar: if self.status_bar_visible {
                self.status_bar_height()
            } else {
                0.
            },
        }
    }

    /// Text in a region starts as it does at the window's root.
    fn region_root(&self) -> Div {
        div()
            .size_full()
            .bg(self.fill().ground())
            .min_h_0()
            .text_color(rgb(self.theme.foreground))
            .text_font(&self.config.ui)
            .text_size(px(self.config.ui.size))
    }

    pub(crate) fn render_region(
        &mut self,
        region: Region,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match region {
            Region::ActivityBar if self.activity_bar_leads() => self
                .region_root()
                .child(self.render_activity_bar(cx))
                .into_any_element(),
            Region::Header => self
                .region_root()
                .child(self.sidebar_header(window, cx))
                .into_any_element(),
            Region::Overlay => {
                // See-through: it only holds what draws over the window.
                let root = div().size_full().relative().flex().flex_col();
                let root = root.child(AnyView::from(self.mark_layer.clone()));
                self.with_overlays(root, window, cx).into_any_element()
            }
            Region::Sidebar => self.render_sidebar_region(window, cx),
            Region::StatusBar if self.status_bar_visible => self
                .region_root()
                .child(self.render_status_bar(cx))
                .into_any_element(),
            _ => Empty.into_any_element(),
        }
    }

    fn render_sidebar_region(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.activity_section().is_some() || self.mod_app().is_some() {
            return Empty.into_any_element();
        }
        let indicators = self.sidebar_view.read(cx).indicators();
        let sidebar = match self.sidebar_mode() {
            SidebarMode::Expanded => self.render_sidebar(indicators, window, cx),
            SidebarMode::Rail => self.render_rail(indicators, window, cx),
            SidebarMode::Hidden => return Empty.into_any_element(),
        };
        self.region_root()
            .flex()
            .flex_col()
            .when(self.activity_bar_leads(), |panel| {
                panel.child(self.panel_title(ActivityPanel::Workspaces, cx))
            })
            .child(div().flex().flex_1().min_h_0().child(sidebar))
            .into_any_element()
    }

    /// What draws over the window, last so it covers the rest of `root`.
    pub(super) fn with_overlays(
        &mut self,
        root: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let root = root
            .children(self.render_tab_ghost(cx))
            .children(self.render_toasts(window, cx))
            .children(self.render_file_transfer(window, cx))
            .children(self.render_menu_leaving(window, cx));
        match self.menu.page.is_some() {
            true => root.child(self.render_menu(window, cx)),
            false => root,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
