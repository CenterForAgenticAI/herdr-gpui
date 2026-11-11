use super::*;

#[test]
fn another_window_cannot_mutate_until_the_os_lease_is_released() {
    let dir = tempfile::tempdir().unwrap();
    let site = crate::actions::Site {
        host: sheprd_remote::teleport::Host::new(&sheprd_client::ConnectTarget::Local).unwrap(),
        target: sheprd_client::ConnectTarget::Local,
        main_root: String::new(),
        database: dir.path().join("private/state.sqlite3"),
    };
    let lease = take(&site).unwrap();
    assert!(matches!(take(&site), Err(crate::Error::Busy)));
    drop(lease);
    assert!(take(&site).is_ok());
}
