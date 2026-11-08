//! Where the daemon looks for `sheprd-acp`: beside itself first, then where
//! Sheprd looks on an SSH host, then `PATH`.

use std::path::{Path, PathBuf};

#[test]
fn a_chat_program_is_found_beside_the_daemon_then_where_sheprd_looks() {
    let path = std::env::join_paths(["/usr/bin", "relative/bin"]).unwrap();
    let dirs = crate::hub::program_dirs(
        Some(Path::new(
            "/home/me/.local/share/herdr-remote-control/bin/1.0/herdr-remote-control",
        )),
        Some(Path::new("/home/me")),
        Some(&path),
    );
    assert_eq!(
        dirs,
        [
            "/home/me/.local/share/herdr-remote-control/bin/1.0",
            "/home/me/.herdr/bin",
            "/home/me/.cargo/bin",
            "/home/me/.local/bin",
            "/usr/bin",
        ]
        .map(PathBuf::from)
    );
    // A relative home names no folder.
    let dirs = crate::hub::program_dirs(None, Some(Path::new("me")), None);
    assert!(dirs.is_empty());
}

#[test]
#[cfg(unix)]
fn a_file_without_the_execute_bit_is_passed_over() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sheprd-acp");
    std::fs::write(&file, b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!crate::hub::runnable(&file));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(crate::hub::runnable(&file));
    assert!(
        !crate::hub::runnable(dir.path()),
        "a folder is not a program"
    );
}
