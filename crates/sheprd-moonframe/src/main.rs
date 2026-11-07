//! All of Sheprd in a window Moonframe lays out from `app/main.luau`, so the
//! layout around it can change without a rebuild.

use {
    moonframe::{
        Data, HostView, Moonframe, ViewInit,
        gpui_kit::{prelude::*, *},
    },
    sheprd_gpui::embed::{Host, Region, Sizes, Startup},
    std::{cell::RefCell, process::ExitCode, rc::Rc},
};

/// What is read before the event loop, and the host the first region makes.
struct Shared {
    host: Option<Host>,
    startup: Option<Startup>,
}

fn main() -> ExitCode {
    // Read before the event loop, as the app's own startup does.
    let shared = Rc::new(RefCell::new(Shared {
        host: None,
        startup: Some(sheprd_gpui::embed::prepare()),
    }));
    Moonframe::new("sheprd-moonframe")
        .title("Sheprd")
        .app_id("dev.sheprd.moonframe")
        .version(env!("CARGO_PKG_VERSION"))
        .app_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/app"))
        .assets(sheprd_gpui::embed::assets())
        // The sparkles: ask an agent to change the layout or add something.
        .keep(["feedback"])
        .view("sheprd", move |init: &ViewInit, window, cx| {
            SheprdRegion::new(init, &shared, window, cx)
        })
        .run()
}

/// One region of the main window, where Luau placed it.
struct SheprdRegion {
    _sizes: Option<Subscription>,
    view: AnyView,
}

impl SheprdRegion {
    fn new(
        init: &ViewInit,
        shared: &Rc<RefCell<Shared>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let host = host(shared, window, cx);
        let name = init.props().get("region").and_then(Data::as_str);
        let region = match name {
            Some("activity") => Some(Region::ActivityBar),
            Some("header") => Some(Region::Header),
            Some("overlay") => Some(Region::Overlay),
            Some("sidebar") => Some(Region::Sidebar),
            Some("status") => Some(Region::StatusBar),
            _ => None,
        };
        let view = match region {
            Some(region) => host.region(region, cx),
            None => host.main(),
        };
        #[cfg(feature = "drive")]
        if region.is_none() {
            drive(&view, window, cx);
        }
        // The main region tells the layout what size the others want.
        let sizes = region.is_none().then(|| {
            let messenger = init.messenger();
            let mut last = None;
            let mut report = move |sizes: Sizes| {
                if last.replace(sizes) != Some(sizes) {
                    messenger.send(Data::map([
                        ("activity_bar", f64::from(sizes.activity_bar).into()),
                        ("header", f64::from(sizes.header).into()),
                        ("left", f64::from(sizes.left).into()),
                        ("status_bar", f64::from(sizes.status_bar).into()),
                    ]));
                }
            };
            report(host.sizes(window, cx));
            host.observe_sizes(window, cx, move |_, sizes, _| report(sizes))
        });
        Self {
            _sizes: sizes,
            view,
        }
    }
}

fn host(shared: &RefCell<Shared>, window: &mut Window, cx: &mut App) -> Host {
    let shared = &mut *shared.borrow_mut();
    let startup = &mut shared.startup;
    shared
        .host
        .get_or_insert_with(|| Host::new(startup.take(), window, cx))
        .clone()
}

impl HostView for SheprdRegion {}

impl Render for SheprdRegion {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // `Terminal` keeps Moonframe's Escape off: it belongs to the terminal.
        div()
            .key_context("Terminal")
            .size_full()
            .child(self.view.clone())
    }
}

/// Sends `SPIKE_DRIVE` to the view: `keys:a b enter` or `drag:x1,y1,x2,y2`.
#[cfg(feature = "drive")]
fn drive(view: &AnyView, window: &Window, cx: &mut App) {
    use sheprd_gpui::embed::Drive;
    let Ok(spec) = std::env::var("SPIKE_DRIVE") else {
        return;
    };
    let input = if let Some(keys) = spec.strip_prefix("keys:") {
        Drive::Keys(keys.to_owned())
    } else {
        let numbers: Vec<f32> = spec
            .trim_start_matches("drag:")
            .split(',')
            .filter_map(|number| number.parse().ok())
            .collect();
        let [x1, y1, x2, y2] = numbers[..] else {
            return;
        };
        Drive::Drag {
            from: (x1, y1),
            to: (x2, y2),
        }
    };
    let delay = std::env::var("SPIKE_DRIVE_DELAY")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(7);
    sheprd_gpui::embed::drive(
        view.clone(),
        input,
        std::time::Duration::from_secs(delay),
        window,
        cx,
    );
}
