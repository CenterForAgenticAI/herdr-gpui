//! The editor a herdr-core split runs: the same script a Herdr pane's
//! shell is typed, as a program and its arguments.

use super::*;

/// Runs `argv` with `editor` (a program name) as `$EDITOR`, and returns the
/// arguments that editor got.
fn ran(argv: &[String], editor: &str) -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("args");
    let program = dir.path().join(editor);
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n",
            out.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .env_remove("VISUAL")
        .env("EDITOR", &program)
        .status()
        .unwrap();
    assert!(status.success(), "{argv:?}");
    std::fs::read_to_string(out)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_split_runs_the_editor_at_the_line_with_no_socket() {
    // No quoting applies: the path is an argument of its own.
    let file = "/w/it's $(here)`x`;y.rs";
    let split = argv(&target(file, Some(12)), None).unwrap();
    assert_eq!(ran(&split, "vim"), ["+12", file]);
    assert_eq!(ran(&split, "nvim"), ["+12", file]);
    let split = argv(&target("/w/a.rs", None), None).unwrap();
    assert_eq!(ran(&split, "vim"), ["+1", "/w/a.rs"]);
}

#[test]
fn a_relative_path_is_refused() {
    assert!(matches!(
        argv(&target("a.rs", None), None),
        Err(crate::Error::Editor(Error::Path))
    ));
}
