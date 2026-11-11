use super::*;

#[test]
fn a_failed_kept_checkout_lookup_reports_a_localized_host_listing_error() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");
    // The lookup runs Git on the workspace's host, so a local folder goes
    // through the same host query as an SSH one.
    let error = super::super::kept_script_checkout(
        &sheprd_client::ConnectTarget::Local,
        missing.to_str().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap_err();
    let crate::Error::Remote(sheprd_remote::Error::Work(sheprd_remote::work::Error::Script {
        step: sheprd_remote::work::Step::List,
        ..
    })) = &error
    else {
        panic!("expected a host Git listing failure, got {error:?}");
    };
    crate::i18n::errors::tests::assert_localized(&[error]);
}
