//! The panel's drawing: a header with the region's title and a hide
//! button, the region's terminal painted from its latest frame, and the
//! edge that resizes it. Clicks focus the region; once it is focused, mouse
//! input reaches its terminal as the projection allows.

use super::{BORDER, Drag, MIN_ROWS, dragged};
use crate::{
    HerdrWindow,
    config::corners,
    i18n::fl,
    motion,
    notifications::safe_text,
    panel_resize::{self, PanelDrag, Side},
};
use gpui::{prelude::*, *};
use herdr_client::protocol::{
    ClientMouseButton, ClientMouseGeometry, ClientMouseKind, ClientMousePosition,
    ClientPaneInputEvent,
    region_surface::{ClientRegion, RegionViewport},
};

/// The header's height above the region's terminal.
const HEADER: f32 = 26.;
/// A region title is daemon data; only this much of it is shown.
const TITLE_LIMIT: usize = 120;

impl HerdrWindow {
    /// The right-edge panel, sliding in or out, or `None` when there is no
    /// region to draw.
    pub(crate) fn render_desktop_region(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let now = motion::now();
        let revision = self.live.regions.projection().map(|p| p.revision);
        self.desktop_region.settle(revision);
        let layout = self.desktop_region_layout(window);
        self.track_desktop_region(layout.as_ref(), now);
        let (region, cols) = layout.or_else(|| self.desktop_region.leaving.clone())?;
        let width = f32::from(cols) * self.cell_width + BORDER;
        let shown = self.desktop_region.slide.shown(now);
        let focused = self.desktop_region.drawn.is_some() && self.live.regions.is_focused(&region);
        let border = match focused {
            true => self.theme.primary(),
            false => self.theme.surface,
        };
        let panel = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px((1. - shown) * width))
            .w(px(width))
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(border))
            .bg(self.fill().base(self.terminal_theme().background))
            .child(self.desktop_region_header(&region, focused, cx))
            .child(self.desktop_region_terminal(&region, cx))
            .child(panel_resize::handle(
                "desktop-region-resize",
                Side::Right,
                PanelDrag::DesktopRegion,
                |_, _, _| {},
            ));
        let label = fl!("desktop-region-label", title = title(&region));
        Some(
            div()
                .id("desktop-region")
                .debug_selector(|| "desktop-region".into())
                .role(Role::Group)
                .aria_label(label)
                .relative()
                .flex_none()
                .w(px(width))
                .h_full()
                .overflow_hidden()
                .on_drag_move(
                    cx.listener(|this, event: &DragMoveEvent<PanelDrag>, window, cx| {
                        if *event.drag(cx) == PanelDrag::DesktopRegion {
                            this.drag_desktop_region(
                                event.bounds.right(),
                                event.event.position.x,
                                window,
                                cx,
                            );
                        }
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.release_desktop_region(cx)),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.release_desktop_region(cx)),
                )
                .child(panel)
                .into_any_element(),
        )
    }

    fn desktop_region_header(
        &self,
        region: &ClientRegion,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let (hover, muted) = (theme.active, theme.muted);
        let text = match focused {
            true => theme.foreground,
            false => theme.muted,
        };
        let id = region.region_id.clone();
        let hide = div()
            .id("desktop-region-hide")
            .debug_selector(|| "desktop-region-hide".into())
            .role(Role::Button)
            .aria_label(fl!("desktop-region-hide"))
            .flex_none()
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(corners::SMALL))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(hover)))
            .child(
                svg()
                    .path("icons/close.svg")
                    .size(px(12.))
                    .text_color(rgb(muted)),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.hide_desktop_region(&id, cx);
            }));
        div()
            .id("desktop-region-header")
            .debug_selector(|| "desktop-region-header".into())
            .flex_none()
            .h(px(HEADER))
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(10.))
            .pr(px(4.))
            .bg(rgb(theme.surface))
            .text_color(rgb(text))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.focus_drawn_region(cx);
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(SharedString::from(title(region))),
            )
            .child(hide)
    }

    fn desktop_region_terminal(
        &self,
        region: &ClientRegion,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let entity = cx.entity();
        let painter = self.painter.clone();
        let surface = self.live.regions.surface(region).cloned();
        let region_id = region.region_id.clone();
        let cell_width = self.cell_width;
        let cell_height = self.config.terminal.line_height();
        let font = self.config.terminal.font();
        let ground = self.ground();
        div()
            .id("desktop-region-terminal")
            .debug_selector(|| "desktop-region-terminal".into())
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(Self::desktop_region_mouse_down),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(Self::desktop_region_mouse_down),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(Self::desktop_region_mouse_down),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(Self::desktop_region_mouse_up),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(Self::desktop_region_mouse_up),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(Self::desktop_region_mouse_up),
            )
            .on_scroll_wheel(cx.listener(Self::desktop_region_scroll))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            this.place_desktop_region(&region_id, bounds, cell_height);
                        });
                    },
                    move |bounds, _, window, cx| {
                        let Some(surface) = &surface else {
                            return;
                        };
                        // A frame wider than the panel stays inside it.
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            painter.borrow_mut().paint_frame(
                                &surface.frame,
                                bounds.origin,
                                Some(bounds.size),
                                ground,
                                cell_width,
                                &font,
                                &[],
                                &[],
                                None,
                                None,
                                window,
                                cx,
                            );
                        });
                    },
                )
                .size_full(),
            )
    }

    /// Reports the cells laid out for `region_id`: zero while it is not the
    /// region drawn, or too short to draw.
    fn place_desktop_region(&mut self, region_id: &str, bounds: Bounds<Pixels>, cell_height: f32) {
        self.desktop_region.bounds = bounds;
        let (cols, rows) = cells(bounds, self.cell_width, cell_height);
        let drawn = self.desktop_region.drawn.as_deref() == Some(region_id);
        let (cols, rows) = match drawn && rows >= MIN_ROWS {
            true => (cols, rows),
            false => (0, 0),
        };
        self.report_desktop_region(RegionViewport {
            region_id: region_id.into(),
            cols,
            rows,
        });
    }

    /// Focuses the drawn right region unless it already has the keyboard.
    /// True when it already had it.
    fn focus_drawn_region(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(region) = self.live.regions.visible_right() else {
            return false;
        };
        if self.desktop_region.drawn.as_deref() != Some(region.region_id.as_str()) {
            return false;
        }
        if self.live.regions.is_focused(region) {
            return true;
        }
        let id = region.region_id.clone();
        self.region_request(|handle, boot| handle.focus_region(boot, &id), cx);
        false
    }

    fn desktop_region_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        // The click that focuses the region is not sent to it.
        if self.focus_drawn_region(cx)
            && let Some(button) = button(event.button)
        {
            self.region_mouse(
                ClientMouseKind::Down(button),
                event.position,
                event.modifiers,
                1,
                cx,
            );
        }
    }

    fn desktop_region_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(button) = button(event.button) {
            self.region_mouse(
                ClientMouseKind::Up(button),
                event.position,
                event.modifiers,
                1,
                cx,
            );
        }
    }

    fn desktop_region_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = crate::terminal::wheel_rows(
            event,
            self.config.terminal.line_height(),
            self.mouse_scroll_lines(),
        );
        let lines = rows.abs().round().clamp(0., f32::from(u16::MAX)) as u16;
        if lines == 0 {
            return;
        }
        let kind = match rows > 0. {
            true => ClientMouseKind::ScrollUp,
            false => ClientMouseKind::ScrollDown,
        };
        self.region_mouse(kind, event.position, event.modifiers, lines, cx);
    }

    /// Sends a mouse event at `position` to the focused region's terminal,
    /// only while it reports the mouse; scrolling always goes.
    fn region_mouse(
        &mut self,
        kind: ClientMouseKind,
        position: Point<Pixels>,
        modifiers: Modifiers,
        lines: u16,
        cx: &mut Context<Self>,
    ) {
        let Some(terminal) = self.region_input_target().map(str::to_owned) else {
            return;
        };
        let scroll = matches!(
            kind,
            ClientMouseKind::ScrollUp | ClientMouseKind::ScrollDown
        );
        let reporting = self
            .live
            .regions
            .visible_right()
            .is_some_and(|region| region.mouse_reporting);
        if !scroll && !reporting {
            return;
        }
        let bounds = self.desktop_region.bounds;
        let cell_height = self.config.terminal.line_height();
        let event = mouse(
            kind,
            position,
            modifiers,
            lines,
            bounds,
            self.cell_width,
            cell_height,
        );
        self.send_region_input(&terminal, event, cx);
    }

    fn drag_desktop_region(
        &mut self,
        right: Pixels,
        x: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(region) = self.live.regions.visible_right().cloned() else {
            return;
        };
        let wanted = (f32::from(right - x) - BORDER) / self.cell_width.max(1.);
        let Some(size) = dragged(&region, wanted, self.desktop_region_row(window)) else {
            return;
        };
        let current = self
            .desktop_region
            .drag
            .map_or(region.size, |drag| drag.size);
        if current == size {
            return;
        }
        self.desktop_region.drag = Some(Drag {
            size,
            released: None,
        });
        let id = region.region_id;
        self.region_request(|handle, boot| handle.resize_region(boot, &id, size), cx);
        cx.notify();
    }

    /// Ends a drag: its size stays drawn until the daemon answers it.
    fn release_desktop_region(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self
            .desktop_region
            .drag
            .filter(|drag| drag.released.is_none())
        else {
            return;
        };
        let projection = self.live.regions.projection();
        let landed = self
            .live
            .regions
            .visible_right()
            .is_some_and(|region| region.size == drag.size);
        self.desktop_region.drag = (!landed).then(|| Drag {
            released: Some(projection.map_or(0, |p| p.revision)),
            ..drag
        });
        cx.notify();
    }
}

/// The region's title as shown: bounded and safe, its plugin when untitled.
fn title(region: &ClientRegion) -> String {
    let title = match region.title.trim().is_empty() {
        true => &region.plugin_id,
        false => &region.title,
    };
    safe_text(title, TITLE_LIMIT)
}

/// Whole cells in `bounds`. A width laid out as a whole number of cells
/// counts as that many, despite rounding.
fn cells(bounds: Bounds<Pixels>, cell_width: f32, cell_height: f32) -> (u16, u16) {
    let count = |length: Pixels, cell: f32| {
        (f32::from(length) / cell.max(1.) + 1e-3)
            .floor()
            .clamp(0., 4096.) as u16
    };
    (
        count(bounds.size.width, cell_width),
        count(bounds.size.height, cell_height),
    )
}

fn button(button: MouseButton) -> Option<ClientMouseButton> {
    match button {
        MouseButton::Left => Some(ClientMouseButton::Left),
        MouseButton::Right => Some(ClientMouseButton::Right),
        MouseButton::Middle => Some(ClientMouseButton::Middle),
        _ => None,
    }
}

/// A mouse event at `position` in the region's terminal laid out at
/// `bounds`, in its cells.
fn mouse(
    kind: ClientMouseKind,
    position: Point<Pixels>,
    modifiers: Modifiers,
    lines: u16,
    bounds: Bounds<Pixels>,
    cell_width: f32,
    cell_height: f32,
) -> ClientPaneInputEvent {
    let (cols, rows) = cells(bounds, cell_width, cell_height);
    let cell = |offset: Pixels, size: f32, count: u16| {
        (f32::from(offset) / size.max(1.))
            .floor()
            .clamp(0., f32::from(count.saturating_sub(1))) as u16
    };
    ClientPaneInputEvent::Mouse {
        kind,
        position: ClientMousePosition::Cell {
            column: cell(position.x - bounds.origin.x, cell_width, cols),
            row: cell(position.y - bounds.origin.y, cell_height, rows),
        },
        geometry: Some(ClientMouseGeometry {
            cols,
            rows,
            width_px: f32::from(bounds.size.width).max(0.) as u32,
            height_px: f32::from(bounds.size.height).max(0.) as u32,
        }),
        modifiers: u8::from(modifiers.shift)
            | (u8::from(modifiers.control) << 1)
            | (u8::from(modifiers.alt) << 2)
            | (u8::from(modifiers.platform) << 3),
        lines,
    }
}
