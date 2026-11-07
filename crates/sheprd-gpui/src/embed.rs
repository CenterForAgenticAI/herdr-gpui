//! The main view, for another GPUI app's window to host. [`prepare`] reads
//! what [`crate::app`] reads before its event loop, and [`Host`] installs it
//! and makes the view, on the Local target, in the regions a layout places.
//!
//! Not yet the whole app: the host owns the window, so what finds a main window
//! by its root view (the control socket, system notifications, opening links)
//! is left out here, as are the updater and the app menus.

pub use crate::window::host_regions::{Region, Sizes};

use crate::{
    Hide, HideOthers, Quit, SheprdWindow, ShowAll, ShowLogs,
    app::{InitialAppearance, StartupAppearance, Stores, light_appearance, shared_sound},
    bind_keys,
    config::Config,
};
use gpui::{
    AnyView, App, AppContext as _, Context, Empty, Entity, IntoElement, Render, Subscription,
    WeakEntity, Window,
};
#[cfg(feature = "integration-test")]
use gpui::{
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
    point, px,
};
use sheprd_client::ConnectTarget;
#[cfg(feature = "integration-test")]
use std::time::Duration;

/// What is read before the event loop; see [`prepare`].
pub struct Startup {
    appearance: StartupAppearance,
    stores: Stores,
}

/// Reads settings and stores. Call it before the host's event loop, so the
/// first frame does no disk work on the UI thread.
pub fn prepare() -> Startup {
    sheprd_client::set_command_environment(crate::login_env::apply);
    sheprd_remote::config::set_loader(Config::load_remote);
    // Before anything resolves a config path or the daemon's socket.
    crate::login_env::adopt_config_home();
    Startup {
        appearance: StartupAppearance::load(Config::load_startup),
        stores: Stores::load(true),
    }
}

fn install(startup: Startup, cx: &mut App) {
    startup.stores.install(cx);
    cx.set_global(startup.appearance.select(light_appearance(cx)));
    let config = &cx.global::<InitialAppearance>().config;
    let (ssh, language) = (config.ssh.clone(), config.language.clone());
    crate::device_tags::DeviceTags::install(cx);
    crate::config::ssh_hosts::install(&ssh, cx);
    crate::i18n::install(&language, cx);
    crate::caffeine::install(cx);
    crate::endpoint::watch_network();
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &ShowLogs, cx| crate::log_window::open(cx));
    bind_keys(cx);
}

/// The app's icons and other assets, for the host's application.
pub fn assets() -> impl gpui::AssetSource {
    crate::icons::Icons
}

/// The main window as views for a host's layout; see [`Region`].
#[derive(Clone)]
pub struct Host {
    window: Entity<SheprdWindow>,
}

impl Host {
    /// Makes the view on the Local target. `startup` is what [`prepare`] read,
    /// `None` when an earlier host already installed it.
    pub fn new(startup: Option<Startup>, window: &mut Window, cx: &mut App) -> Self {
        if let Some(startup) = startup {
            install(startup, cx);
        }
        let sound = shared_sound(cx);
        let window = cx.new(|cx| {
            let mut view = SheprdWindow::new(
                ConnectTarget::Local,
                window,
                cx,
                #[cfg(feature = "integration-test")]
                false,
            );
            view.hosted = true;
            view.use_sound(sound);
            view
        });
        Self { window }
    }

    /// The terminals, tabs and pages: everything the regions leave.
    pub fn main(&self) -> AnyView {
        self.window.clone().into()
    }

    pub fn region(&self, region: Region, cx: &mut App) -> AnyView {
        let window = self.window.clone();
        cx.new(|cx: &mut Context<RegionView>| {
            // Regions show the window's state, so they redraw with it.
            let observing = cx.observe(&window, |_, _, cx| cx.notify());
            RegionView {
                _observing: observing,
                region,
                window: window.downgrade(),
            }
        })
        .into()
    }

    pub fn sizes(&self, window: &Window, cx: &App) -> Sizes {
        self.window.read(cx).region_sizes(window)
    }

    /// Calls `changed` whenever the main window changes, with its sizes.
    pub fn observe_sizes<T: 'static>(
        &self,
        window: &mut Window,
        cx: &mut Context<T>,
        mut changed: impl FnMut(&mut T, Sizes, &mut Context<T>) + 'static,
    ) -> Subscription {
        cx.observe_in(&self.window, window, move |this, view, window, cx| {
            let sizes = view.read(cx).region_sizes(window);
            changed(this, sizes, cx);
        })
    }
}

/// A [`Region`] drawn through the main window, as `SidebarView` is.
struct RegionView {
    _observing: Subscription,
    region: Region,
    window: WeakEntity<SheprdWindow>,
}

impl Render for RegionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let region = self.region;
        self.window
            .update(cx, |view, cx| view.render_region(region, window, cx))
            .unwrap_or_else(|_| Empty.into_any_element())
    }
}

/// What [`drive`] does to the hosted view, in window coordinates.
#[cfg(feature = "integration-test")]
pub enum Drive {
    /// Presses at `from`, drags to `to`, and releases.
    Drag { from: (f32, f32), to: (f32, f32) },
    /// Types each keystroke in turn, a space between them.
    Keys(String),
    /// Types a keystroke such as `cmd-shift-p`.
    Key(String),
}

/// Sends `input` through the window as the platform would, after `delay`, for
/// checking that a pane offset and a host's focus leave input working.
#[cfg(feature = "integration-test")]
pub fn drive(view: AnyView, input: Drive, delay: Duration, window: &Window, cx: &mut App) {
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(delay).await;
            let _ = cx.update(|window, cx| match input {
                Drive::Drag { from, to } => {
                    let from = point(px(from.0), px(from.1));
                    let to = point(px(to.0), px(to.1));
                    let events = [
                        PlatformInput::MouseMove(MouseMoveEvent {
                            position: from,
                            pressed_button: None,
                            modifiers: Modifiers::default(),
                        }),
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Left,
                            position: from,
                            modifiers: Modifiers::default(),
                            click_count: 1,
                            first_mouse: false,
                        }),
                        PlatformInput::MouseMove(MouseMoveEvent {
                            position: to,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: Modifiers::default(),
                        }),
                        PlatformInput::MouseUp(MouseUpEvent {
                            button: MouseButton::Left,
                            position: to,
                            modifiers: Modifiers::default(),
                            click_count: 1,
                        }),
                    ];
                    for (step, event) in events.into_iter().enumerate() {
                        let result = window.dispatch_event(event, cx);
                        if let Ok(view) = view.clone().downcast::<SheprdWindow>() {
                            let view = view.read(cx);
                            tracing::info!(
                                step,
                                prevented = result.default_prevented,
                                selection = view.selection.is_some(),
                                menu = view.menu.page.is_some(),
                                "drive: after event"
                            );
                        }
                    }
                    if let Ok(view) = view.downcast::<SheprdWindow>() {
                        let view = view.read(cx);
                        tracing::info!(
                            bounds = ?view.bounds,
                            grid = ?view.grid_position(from),
                            ready = view.live.surface_ready(),
                            selection = view.selection.is_some(),
                            menu = view.menu.page.is_some(),
                            "drive: terminal geometry"
                        );
                    }
                }
                Drive::Keys(text) => {
                    if let Ok(view) = view.clone().downcast::<SheprdWindow>() {
                        let view = view.read(cx);
                        tracing::info!(
                            focused = view.focus.is_focused(window),
                            contains = view.focus.contains_focused(window, cx),
                            ready = view.live.surface_ready(),
                            "drive: before typing"
                        );
                    }
                    for key in text.split_whitespace() {
                        if let Ok(keystroke) = Keystroke::parse_with_os(key, crate::keymap::HOST_OS)
                        {
                            let handled = window.dispatch_keystroke(keystroke, cx);
                            tracing::info!(key, handled, "drive: keystroke");
                        }
                    }
                }
                Drive::Key(text) => match Keystroke::parse_with_os(&text, crate::keymap::HOST_OS) {
                    Ok(keystroke) => {
                        let handled = window.dispatch_keystroke(keystroke, cx);
                        tracing::info!(text, handled, "drive: keystroke");
                    }
                    Err(error) => tracing::warn!(%error, "drive: bad keystroke"),
                },
            });
        })
        .detach();
}
