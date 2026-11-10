use super::kept::kept;
use super::*;
use crate::merged_cleanup::kept::kept_candidates;
use std::path::Path;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .arg("-c")
        .arg(format!(
            "core.hooksPath={}",
            dir.join("unused-hooks").display()
        ))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn kept_cleanup_checks_its_folder_even_when_the_cached_branch_moves_elsewhere() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    let linked = dir.path().join("linked");
    let other = dir.path().join("other");
    std::fs::create_dir(&main).unwrap();
    git(&main, &["init", "-b", "main"]);
    git(&main, &["commit", "--allow-empty", "-m", "initial"]);
    git(
        &main,
        &["worktree", "add", "-b", "feature", linked.to_str().unwrap()],
    );
    let mut workspace = kept("space-1", true, sheprd_session::Keeper::App);
    workspace.cwd = linked.to_str().unwrap().into();
    workspace.worktree.as_mut().unwrap().key = main.join(".git").to_str().unwrap().into();
    let workspaces = [workspace];
    let mut pr = merged(State::Merged);
    pr.head_ref_oid = git(&linked, &["rev-parse", "HEAD"]);
    let candidate = kept_candidates(&workspaces, |input| {
        assert!(input.checkout.is_none(), "keep the PR cache key unchanged");
        Some(&pr)
    })
    .next()
    .unwrap();
    let path = linked.to_str().unwrap();
    let cancelled = AtomicBool::new(false);
    assert_eq!(candidate.input.checkout.as_deref(), Some(path));
    assert!(verify_kept_removal(&candidate, path, &cancelled).is_ok());
    // Same HEAD, another branch: HEAD comparison alone is insufficient.
    git(&linked, &["switch", "-c", "unmerged"]);
    git(
        &main,
        &["worktree", "add", other.to_str().unwrap(), "feature"],
    );
    assert!(probe(&candidate.input, &cancelled).is_err());
    assert!(verify_kept_removal(&candidate, path, &cancelled).is_err());
    assert!(matches!(
        verify_kept_removal(&candidate, other.to_str().unwrap(), &cancelled),
        Err(crate::Error::StaleWorkspace)
    ));
    assert!(linked.exists());
    git(&other, &["switch", "--detach"]);
    git(&linked, &["switch", "feature"]);
    git(
        &linked,
        &["commit", "--allow-empty", "-m", "new unmerged work"],
    );
    assert!(matches!(
        verify_kept_removal(&candidate, path, &cancelled),
        Err(crate::Error::StaleWorkspace)
    ));
}
