//! The branch of the shown herdr-core pane's checkout, read from Git on a
//! thread for the Git chip.

use super::super::checkout::{Checkout, valid};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// Polls until Git's answer lands, at most a few seconds.
fn settle(checkout: &mut Checkout, folder: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !checkout.answered(folder) && Instant::now() < deadline {
        checkout.poll(Some(folder), Instant::now());
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_shown_panes_branch_is_read_from_git_and_forgotten_with_the_pane() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "feature/x"]);
    let mut checkout = Checkout::default();
    assert_eq!(checkout.branch(dir.path()), None);
    settle(&mut checkout, dir.path());
    assert_eq!(checkout.branch(dir.path()), Some("feature/x"));
    // Another folder's answer is not this one's.
    assert_eq!(checkout.branch(Path::new("/elsewhere")), None);
    assert!(checkout.poll(None, Instant::now()));
    assert_eq!(checkout.branch(dir.path()), None);
}

#[test]
fn a_folder_outside_a_checkout_has_no_branch() {
    let dir = tempfile::tempdir().unwrap();
    let mut checkout = Checkout::default();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !checkout.answered(dir.path()) && Instant::now() < deadline {
        checkout.poll(Some(dir.path()), Instant::now());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(checkout.answered(dir.path()));
    assert_eq!(checkout.branch(dir.path()), None);
}

#[test]
fn only_plain_bounded_branch_names_are_carried() {
    assert_eq!(valid("main"), Some("main".into()));
    assert_eq!(valid(""), None);
    assert_eq!(valid("a\u{1b}b"), None);
    assert_eq!(valid(&"b".repeat(1025)), None);
}
