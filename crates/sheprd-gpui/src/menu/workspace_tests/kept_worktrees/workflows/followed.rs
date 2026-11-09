//! Kept workspaces' scripts the window follows at once: every running
//! archive, every waiting setup, and the main checkout an archive is given.

use super::*;

/// A second kept checkout of `app` at `folder`, beside the fixture's.
fn keep_another(
    view: &mut SheprdWindow,
    folder: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<SheprdWindow>,
) -> String {
    view.core_terminals.seed_checkout_for_test(
        folder,
        Some(Identity {
            branch: Some("other".into()),
            key: Path::new(MAIN).join(".git").display().to_string(),
            label: "app".into(),
            linked: true,
        }),
    );
    let location = Location::core_terminal(Panes::single(TerminalId::new("t2").unwrap()), false);
    view.open_kept_workspace(None, Path::new(folder), location, window, cx)
        .unwrap()
}

fn trust(cx: &mut gpui::App, launch: &Launch, config: &crate::worktree_scripts::Config) {
    cx.default_global::<Trust>().grant(Grant {
        endpoint: launch.endpoint_id.clone(),
        repo_key: launch.repo_key.clone(),
        digest: config.digest.clone(),
    });
}

#[gpui::test]
fn every_running_archive_is_followed_until_it_ends(cx: &mut TestAppContext) {
    let (first_dir, second_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "[scripts]\narchive = \"make clean\"\n";
    for dir in [&first_dir, &second_dir] {
        write_scripts(dir.path(), text);
    }
    let (first, second) = (
        first_dir.path().display().to_string(),
        second_dir.path().display().to_string(),
    );
    let (view, cx, first_id) = window(cx, &first, Some(true));
    let config = crate::worktree_scripts::Config::parse(text.as_bytes()).unwrap();
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let second_id = keep_another(view, &second, window, cx);
            let archive = |view: &SheprdWindow, id: &str, path: &str| Launch {
                requested: true,
                ..kept_launch(view, ScriptKind::Archive, id, path)
            };
            let launch = archive(view, &first_id, &first);
            trust(cx, &launch, &config);
            view.start_archive_script(launch, config.clone(), cx)
                .unwrap();
            let launch = archive(view, &second_id, &second);
            view.start_archive_script(launch, config.clone(), cx)
                .unwrap();
            assert!(
                view.follows_kept_archive(&first_id),
                "the first is not forgotten"
            );
            assert!(view.follows_kept_archive(&second_id));
            // A second archive of one workspace waits for its first to end.
            let again = archive(view, &first_id, &first);
            assert!(
                view.start_archive_script(again, config.clone(), cx)
                    .is_err()
            );
            assert!(view.follows_kept_archive(&first_id));
        })
    });
}

#[gpui::test]
fn setups_wait_in_turn_rather_than_replace_each_other(cx: &mut TestAppContext) {
    let (first_dir, second_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for dir in [&first_dir, &second_dir] {
        write_scripts(dir.path(), "[scripts]\nsetup = \"npm ci\"\n");
    }
    let (first, second) = (
        first_dir.path().display().to_string(),
        second_dir.path().display().to_string(),
    );
    let (view, cx, _) = window(cx, &first, Some(true));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            keep_another(view, &second, window, cx);
            view.queue_kept_setup(kept_launch(view, ScriptKind::Setup, "", &first));
            view.queue_kept_setup(kept_launch(view, ScriptKind::Setup, "", &second));
            assert_eq!(view.queued_kept_setups().len(), 2);
            view.poll_kept_scripts(cx);
            // The first runs; the second waits for the one script slot.
            assert!(view.worktree_script.is_some());
            let waiting = view.queued_kept_setups();
            assert_eq!(waiting.len(), 1);
            assert_eq!(waiting[0].checkout.as_ref().unwrap().path, second);
        })
    });
}

#[gpui::test]
fn a_kept_archive_is_given_its_main_checkout(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().display().to_string();
    let text = "[scripts]\narchive = \"make clean\"\n";
    write_scripts(dir.path(), text);
    let (view, cx, id) = window(cx, &folder, Some(true));
    let config = crate::worktree_scripts::Config::parse(text.as_bytes()).unwrap();
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_workspace_dialog_for(
                &id,
                WorkspaceAction::DeleteWorktree,
                gpui::Point::default(),
                window,
                cx,
            );
            // As the dialog's read leaves it: the script, and Git's main checkout.
            view.menu.deletion = Some(Deletion {
                path: Some(folder.clone()),
                root: Some(MAIN.into()),
                archive: ArchiveCheck::Read(Some(config)),
                ..Deletion::new(None, false)
            });
            view.submit_workspace_dialog(window, cx);
            assert_eq!(
                view.worktree_script_root(),
                Some(MAIN),
                "{:?}",
                view.menu.error
            );
        })
    });
}
