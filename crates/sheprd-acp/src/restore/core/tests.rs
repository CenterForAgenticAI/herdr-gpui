use super::*;

const ID: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn an_endpoint_has_one_owner_until_it_exits_not_until_a_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let id = ID.parse().unwrap();
    let first = lock_in(dir.path(), &id).unwrap();
    assert!(lock_in(dir.path(), &id).is_err());
    drop(first);
    let next = lock_in(dir.path(), &id).unwrap();
    assert!(lock_in(dir.path(), &id).is_err());
    drop(next);
    assert!(dir.path().join(format!("{ID}.lock")).exists());
}

#[test]
fn recovery_requires_every_part_of_the_original_context() {
    let cwd = std::env::current_dir().unwrap();
    let record = restore::Record {
        adapter: false,
        agent: sheprd_chat_protocol::Agent::Codex,
        cwd: cwd.clone(),
        endpoint: ID.parse().unwrap(),
        pane: "t9".to_owned(),
        session: "exact-session".to_owned(),
        socket: cwd.join("core.sock"),
    };
    let env = std::collections::BTreeMap::from([
        (ENDPOINT_ENV, std::ffi::OsString::from(ID)),
        ("HC_TERMINAL_ID", "t9".into()),
        ("HC_TERMINAL_SOCKET", record.socket.clone().into_os_string()),
    ]);
    assert!(matches_context(&record, &record.endpoint, &cwd, |name| env
        .get(name)
        .cloned()));
    for key in env.keys() {
        let mut wrong = env.clone();
        wrong.insert(key, "other".into());
        assert!(!matches_context(&record, &record.endpoint, &cwd, |name| {
            wrong.get(name).cloned()
        }));
        wrong.remove(key);
        assert!(!matches_context(&record, &record.endpoint, &cwd, |name| {
            wrong.get(name).cloned()
        }));
    }
    assert!(!matches_context(
        &record,
        &record.endpoint,
        &cwd.join("other"),
        |name| env.get(name).cloned()
    ));
    let other = "abcdef0123456789abcdef0123456789".parse().unwrap();
    assert!(!matches_context(&record, &other, &cwd, |name| env
        .get(name)
        .cloned()));
}
