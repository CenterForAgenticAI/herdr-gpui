//! Herdr 1 counts as installed when the login shell's PATH has it, though
//! the service's own PATH leaves that directory out: a phone then does not
//! start Herdr 2 in its place.

use std::path::Path;

/// A program name nothing else on a test machine has.
const PROGRAM: &str = if cfg!(windows) {
    "sheprd-test-herdr-on-login-path.exe"
} else {
    "sheprd-test-herdr-on-login-path"
};

#[test]
fn a_program_only_on_the_login_shells_path_is_installed() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join(PROGRAM);
    std::fs::write(&program, b"").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let login = std::env::join_paths([dir.path(), Path::new("/nowhere")]).unwrap();
    assert!(crate::hub::wake::installed(PROGRAM, Some(&login)));
    assert!(!crate::hub::wake::installed(PROGRAM, None));
    let elsewhere = std::env::join_paths([Path::new("/nowhere")]).unwrap();
    assert!(!crate::hub::wake::installed(PROGRAM, Some(&elsewhere)));
}
