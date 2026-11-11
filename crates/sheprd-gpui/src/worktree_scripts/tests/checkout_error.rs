use super::*;

#[test]
fn a_failed_kept_checkout_lookup_is_a_localized_host_error() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");
    let error = super::super::kept_script_checkout(
        &sheprd_client::ConnectTarget::Local,
        missing.to_str().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap_err();
    // The lookup runs through the host's scripts, as on any other host;
    // Windows has no local POSIX shell, so there it fails before Git runs.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    assert!(
        matches!(
            &error,
            crate::Error::Remote(sheprd_remote::Error::Work(
                sheprd_remote::work::Error::Script { .. }
            ))
        ),
        "expected the host's Git lookup to fail, got {error:?}"
    );
    crate::i18n::errors::tests::assert_localized(&[error]);
}
