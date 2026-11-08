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
