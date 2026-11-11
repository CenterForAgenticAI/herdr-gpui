use super::*;

#[test]
fn a_failed_kept_checkout_lookup_is_localized() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");
    let error = super::super::kept_script_checkout(
        &sheprd_client::ConnectTarget::Local,
        missing.to_str().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap_err();
    // The lookup runs through the kept host's Git, whichever host that is.
    assert!(
        matches!(error, crate::Error::Remote(_)),
        "expected a kept-host checkout lookup failure, got {error:?}"
    );
    crate::i18n::errors::tests::assert_localized(&[error]);
}
