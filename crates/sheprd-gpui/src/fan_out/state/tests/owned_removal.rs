//! Keep removes an owned lane's checkout only once its agent is ended, and
//! never while another program runs there. A stand-in `herdr-core` on an
//! isolated `HOME` answers; Git is never reached when removal is refused.

use super::*;
use std::{os::unix::fs::PermissionsExt, path::Path};

/// This computer with `HOME` at `home`, whose `herdr-core` logs each call
/// and answers `ls` with `terminals`.
fn host(home: &Path, terminals: &str) -> Host {
    let bin = home.join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = home.join("calls.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncase \"$1\" in\nls) cat '{}' ;;\nkill) echo '{{\"terminals\":[]}}' > '{}' ;;\nesac\n",
        log.display(),
        home.join("inventory").display(),
        home.join("inventory").display(),
    );
    std::fs::write(home.join("inventory"), terminals).unwrap();
    let core = bin.join("herdr-core");
    std::fs::write(&core, script).unwrap();
    std::fs::set_permissions(&core, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut host = origin().host;
    host.env = vec![("HOME".into(), home.display().to_string())];
    host
}

fn calls(home: &Path) -> Vec<String> {
    std::fs::read_to_string(home.join("calls.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn owned() -> job::Owned {
    job::Owned {
        agent: "lane-a".into(),
        kind: AgentKind::Claude,
        terminal: Some("t9".into()),
    }
}

fn lane(dir: &Path) -> work::Worktree {
    work::Worktree {
        path: dir.join("lane").display().to_string(),
        opened: work::Opened::Herdr2,
    }
}

#[test]
fn a_shell_in_the_lane_keeps_its_checkout_and_nothing_is_killed() {
    let dir = tempfile::tempdir().unwrap();
    let tree = lane(dir.path());
    let home = dir.path().join("home");
    let shell = format!(
        r#"{{"terminals":[{{"address":"t3","state":"live","cwd":"{}","argv":["sh"]}}]}}"#,
        tree.path
    );
    let host = host(&home, &shell);
    let removed = job::remove(
        &host,
        &tree,
        "/tmp/repo/.git",
        "lane",
        Some(&owned()),
        &AtomicBool::new(false),
    );
    assert!(matches!(removed, Err(Error::AgentUncertain)), "{removed:?}");
    assert!(calls(&home).iter().all(|call| call.starts_with("ls ")));
}

#[test]
fn a_running_lane_agent_is_ended_before_its_checkout_goes() {
    let dir = tempfile::tempdir().unwrap();
    let tree = lane(dir.path());
    let home = dir.path().join("home");
    let agent = format!(
        r#"{{"terminals":[{{"address":"t9","state":"live","cwd":"{}","argv":["claude"],"labels":{{"sheprd.fanout.lane":"lane-a"}},"foreground":{{"name":"claude","argv":["claude"],"pid":9}}}}]}}"#,
        tree.path
    );
    let host = host(&home, &agent);
    // No receipt was made for this folder, so the checkout itself is kept;
    // what matters is that the agent ended before Git was asked.
    let _ = job::remove(
        &host,
        &tree,
        "/tmp/repo/.git",
        "lane",
        Some(&owned()),
        &AtomicBool::new(false),
    );
    let calls = calls(&home);
    assert!(
        calls.iter().any(|call| call == "kill --json -- t9"),
        "{calls:?}"
    );
}
