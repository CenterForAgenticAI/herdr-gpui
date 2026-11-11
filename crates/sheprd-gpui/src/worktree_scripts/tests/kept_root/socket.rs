use super::*;

#[test]
fn a_custom_herdr_socket_does_not_redirect_local_core_script_files() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "-q", "-b", "main"]);
    git(
        temp.path(),
        &["commit", "-q", "--allow-empty", "-m", "start"],
    );
    let inner = temp.path().join("src");
    std::fs::create_dir(&inner).unwrap();
    let target = sheprd_client::ConnectTarget::Socket("/forwarded/herdr.sock".into());
    let checkout = crate::worktree_scripts::kept_script_checkout(
        &target,
        inner.to_str().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        std::fs::canonicalize(checkout.path).unwrap(),
        std::fs::canonicalize(temp.path()).unwrap()
    );
    assert_eq!(
        std::fs::canonicalize(checkout.root.unwrap()).unwrap(),
        std::fs::canonicalize(temp.path()).unwrap()
    );
    assert!(sheprd_remote::teleport::Host::new(&target).is_err());
}
