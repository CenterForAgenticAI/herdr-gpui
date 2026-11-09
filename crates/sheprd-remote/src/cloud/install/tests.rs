#![allow(clippy::unwrap_used)]
use super::*;

/// Stand in for `coder ssh`: run the appended remote command locally.
fn local(home: &std::path::Path) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(r#"eval "$1""#)
        .arg("sh")
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin");
    command
}

#[test]
fn the_probe_finds_herdr_where_the_bridge_looks() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    // The test's PATH and the probe's fixed roots are system-wide; a fresh
    // HOME does not hide Herdr installed in /usr/bin or /bin by a package.
    let system = [
        "/usr/bin/herdr",
        "/bin/herdr",
        "/opt/homebrew/bin/herdr",
        "/usr/local/bin/herdr",
        "/home/linuxbrew/.linuxbrew/bin/herdr",
        "/nix/var/nix/profiles/default/bin/herdr",
        "/run/current-system/sw/bin/herdr",
    ]
    .iter()
    .filter(|path| cfg!(target_os = "linux") || !path.starts_with("/home/linuxbrew/"))
    .any(|path| std::path::Path::new(path).exists());
    if !system {
        assert!(!installed(local(home.path()), &|| false).unwrap());
    }
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("herdr"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(bin.join("herdr"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(installed(local(home.path()), &|| false).unwrap());
}

#[test]
fn failures_report_the_output_tail_without_terminal_escapes() {
    // One stream: stdout and stderr drain on separate threads, so their
    // interleaving is not ordered.
    let mut failing = Command::new("/bin/sh");
    failing.args([
        "-c",
        r#"{ for i in 1 2 3 4 5 6 7 8; do echo "line $i"; done; printf '\033[31mboom\033[0m\n'; } >&2; exit 9; #"#,
    ]);
    let Err(Error::Install(Failure::Output(text))) = install(failing, &|| false) else {
        panic!("expected an install failure");
    };
    assert!(text.ends_with("boom"), "{text}");
    assert!(!text.contains('\u{1b}'));
    assert_eq!(text.lines().count(), TAIL_LINES);
    assert!(!text.contains("line 1\n"));
}

#[test]
fn a_missing_curl_is_named_and_cancellation_kills_the_child() {
    let mut no_curl = Command::new("/bin/sh");
    no_curl.args(["-c", "exit 4; #"]);
    assert!(matches!(
        install(no_curl, &|| false),
        Err(Error::Install(Failure::NoCurl))
    ));
    // `sleep` is not the script's last command, so every shell forks it: a
    // grandchild that still holds the output pipes after the child is
    // killed. Cancelling only once it has had time to start makes that so.
    let mut slow = Command::new("/bin/sh");
    slow.args(["-c", "sleep 30; exit 0; #"]);
    let started = Instant::now();
    let cancelled = || started.elapsed() >= Duration::from_millis(500);
    assert!(matches!(install(slow, &cancelled), Err(Error::Cancelled)));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn output_is_bounded() {
    let mut chatty = Command::new("/bin/sh");
    chatty.args(["-c", "yes | head -c 1000000; exit 1; #"]);
    let Err(Error::Install(Failure::Output(text))) = install(chatty, &|| false) else {
        panic!("expected an install failure");
    };
    assert!(text.len() <= OUTPUT_LIMIT);
}

/// A machine whose `curl` serves Herdr's installer, which puts `herdr` in
/// place, and fails every other download, such as herdr-core's.
fn machine_without_core_downloads() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let tools = home.path().join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    let curl = tools.join("curl");
    std::fs::write(
        &curl,
        r##"#!/bin/sh
case "$*" in
*herdr.dev/install.sh*) printf '%s\n' 'mkdir -p "$HOME/.local/bin" && printf "#!/bin/sh\n" > "$HOME/.local/bin/herdr" && chmod 755 "$HOME/.local/bin/herdr"';;
*) exit 22;;
esac
"##,
    )
    .unwrap();
    std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    home
}

/// [`local`] with the machine's own `curl` first on PATH.
fn on(home: &std::path::Path) -> Command {
    let mut command = local(home);
    command.env(
        "PATH",
        format!("{}:/usr/bin:/bin", home.join("tools").display()),
    );
    command
}

#[test]
fn a_core_that_cannot_be_installed_leaves_herdr_installed() {
    let home = machine_without_core_downloads();
    install(on(home.path()), &|| false).unwrap();
    // The core's download fails; the step still succeeds.
    add_core(Ok::<_, Error>(on(home.path())), &|| false).unwrap();
    assert!(home.path().join(".local/bin/herdr").exists());
    assert!(!home.path().join(".local/bin/herdr-core").exists());
    assert!(installed(local(home.path()), &|| false).unwrap());
}

#[test]
fn adding_the_core_stops_when_cancelled() {
    let home = machine_without_core_downloads();
    assert!(matches!(
        add_core(Ok::<_, Error>(on(home.path())), &|| true),
        Err(Error::Cancelled)
    ));
}

#[test]
fn a_core_command_that_cannot_be_made_leaves_setup_going() {
    let unmade = || Err::<Command, _>(Error::Cancelled);
    add_core(unmade(), &|| false).unwrap();
    assert!(matches!(
        add_core(unmade(), &|| true),
        Err(Error::Cancelled)
    ));
}
