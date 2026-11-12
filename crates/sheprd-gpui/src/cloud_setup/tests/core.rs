//! A new device without Herdr 1, as a dispatch makes one: its setup script
//! starts on its herdr-core, in the workspace the app keeps there.
#![allow(clippy::unwrap_used)]

use super::{Pending, PendingSetups};
use crate::{
    browser::Location,
    cloud::CloudProvider,
    core_terminals::{Purpose, panes::Panes},
    sidebar::layout_tests::{SidebarFixture, fixture_window},
};
use gpui::{AppContext, TestAppContext};
use sheprd_backend::TerminalId;
use std::{path::Path, time::Instant};

#[gpui::test]
fn a_device_without_herdr_1_runs_its_script_in_its_kept_workspace(cx: &mut TestAppContext) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| fixture_window(window, cx));
        SidebarFixture(view)
    });
    let view = cx.update(|_, cx| fixture.read(cx).0.clone());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.live.snapshot = None;
            view.live.missing_installation = true;
            let now = Instant::now();
            let device = view.endpoints[0].id.clone();
            let pending = |id: &str| {
                let provider = CloudProvider::ALL[0];
                Pending::new(id.into(), "Box".into(), provider, "echo hi\n".into(), now)
            };
            view.queue_cloud_setup(pending(&device), cx);
            // No kept workspace yet: nothing to start the tab in.
            view.poll_cloud_setup(now, cx);
            assert_eq!(PendingSetups::waiting(cx), std::slice::from_ref(&device));
            assert!(view.core_terminals.last_wanted_for_test().is_none());
            let location =
                Location::core_terminal(Panes::single(TerminalId::new("t1").unwrap()), false);
            let kept = view
                .open_kept_workspace(None, Path::new("/home/box"), location, window, cx)
                .unwrap();
            view.poll_cloud_setup(now, cx);
            assert!(PendingSetups::waiting(cx).is_empty(), "taken once");
            assert!(view.cloud_setup.is_none(), "no Herdr 1 request");
            let (purpose, spawn) = view.core_terminals.last_wanted_for_test().unwrap();
            let Purpose::Tab { workspace, .. } = purpose else {
                panic!("a tab of the kept workspace");
            };
            assert_eq!(workspace, &kept);
            assert_eq!(spawn.name.as_deref(), Some("setup"));
            assert!(spawn.env.iter().any(|(key, value)| {
                key == crate::worktree_scripts::SCRIPT_ENV && value == "echo hi\n"
            }));
            assert!(
                spawn
                    .env
                    .iter()
                    .any(|(key, _)| key == crate::cloud_setup::launch::PROVIDER_ENV)
            );
        })
    });
}
