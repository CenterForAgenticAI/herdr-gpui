use super::*;
use std::path::PathBuf;

fn run(repository: &Path, args: &[&str]) {
    let status = sheprd_client::new_command("git")
        .current_dir(repository)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A repository on `main` with `a.txt` and `old.txt` committed.
fn repository(parent: &Path) -> PathBuf {
    let repository = parent.join("project");
    std::fs::create_dir(&repository).unwrap();
    run(&repository, &["init", "--quiet"]);
    std::fs::write(repository.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(repository.join("old.txt"), "kept\n".repeat(20)).unwrap();
    run(&repository, &["add", "."]);
    run(&repository, &["commit", "--quiet", "-m", "start"]);
    repository
}

fn listed(answer: Answer) -> FileList {
    match answer {
        Answer::Files(list) => list,
        other => panic!("{other:?}"),
    }
}

fn file<'a>(list: &'a FileList, path: &str) -> &'a ChangedFile {
    list.files.iter().find(|file| file.path == path).unwrap()
}

#[test]
fn uncommitted_changes_list_edits_renames_and_untracked_files() {
    let dir = tempfile::tempdir().unwrap();
    let repository = repository(dir.path());
    std::fs::write(repository.join("a.txt"), "one\n2\nthree\nfour\n").unwrap();
    run(&repository, &["mv", "old.txt", "new.txt"]);
    std::fs::write(repository.join("notes.md"), "hello\nworld\n").unwrap();
    std::fs::write(repository.join("blob.bin"), [0_u8, 1, 2]).unwrap();

    let wanted = Wanted::Files {
        scope: Scope::Uncommitted,
    };
    let list = listed(answer(&repository, &wanted));
    assert_eq!(list.total, 4, "{list:?}");
    assert_eq!(list.base, None);
    let edited = file(&list, "a.txt");
    assert_eq!(edited.status, FileStatus::Modified);
    assert_eq!((edited.added, edited.removed), (Some(2), Some(1)));
    let renamed = file(&list, "new.txt");
    assert_eq!(renamed.status, FileStatus::Renamed);
    assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));
    let notes = file(&list, "notes.md");
    assert_eq!(
        (notes.status, notes.added),
        (FileStatus::Untracked, Some(2))
    );
    assert_eq!(file(&list, "blob.bin").shown, Shown::Binary);
}

#[test]
fn a_listed_files_diff_is_numbered_and_another_path_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repository = repository(dir.path());
    std::fs::write(repository.join("a.txt"), "one\n2\nthree\n").unwrap();
    std::fs::write(repository.join("notes.md"), "hello\n").unwrap();
    let scope = Scope::Uncommitted;
    let Answer::Diff(diff) = answer(
        &repository,
        &Wanted::Diff {
            scope,
            path: "a.txt".into(),
        },
    ) else {
        panic!()
    };
    let kinds: Vec<LineKind> = diff.lines.iter().map(|line| line.kind).collect();
    assert_eq!(
        kinds,
        [
            LineKind::Hunk,
            LineKind::Context,
            LineKind::Removed,
            LineKind::Added,
            LineKind::Context
        ]
    );
    assert_eq!(diff.lines[2].text, "two");
    assert_eq!((diff.lines[2].old, diff.lines[2].new), (Some(2), None));
    assert_eq!((diff.lines[3].old, diff.lines[3].new), (None, Some(2)));
    assert_eq!((diff.lines[4].old, diff.lines[4].new), (Some(3), Some(3)));

    let Answer::Diff(untracked) = answer(
        &repository,
        &Wanted::Diff {
            scope,
            path: "notes.md".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(untracked.lines[1].text, "hello");
    assert_eq!(untracked.lines[1].kind, LineKind::Added);

    for path in ["old.txt", "../secret", "/etc/hosts"] {
        let wanted = Wanted::Diff {
            scope,
            path: path.into(),
        };
        assert_eq!(
            answer(&repository, &wanted),
            Answer::Failed(Failure::NoFile)
        );
    }
}

#[test]
fn a_branch_is_compared_with_its_base_uncommitted_work_included() {
    let dir = tempfile::tempdir().unwrap();
    let repository = repository(dir.path());
    run(&repository, &["switch", "--quiet", "-c", "feature"]);
    std::fs::write(repository.join("b.txt"), "b\n").unwrap();
    run(&repository, &["add", "b.txt"]);
    run(&repository, &["commit", "--quiet", "-m", "b"]);
    std::fs::write(repository.join("a.txt"), "changed\n").unwrap();
    let list = listed(answer(
        &repository,
        &Wanted::Files {
            scope: Scope::Branch,
        },
    ));
    assert_eq!(list.base.as_deref(), Some("main"));
    assert_eq!(file(&list, "b.txt").status, FileStatus::Added);
    assert_eq!(file(&list, "a.txt").status, FileStatus::Modified);
}

#[test]
fn a_folder_outside_git_and_github_questions_say_so() {
    let dir = tempfile::tempdir().unwrap();
    let wanted = Wanted::Files {
        scope: Scope::Uncommitted,
    };
    assert_eq!(
        answer(dir.path(), &wanted),
        Answer::Failed(Failure::NoRepository)
    );
    assert_eq!(
        answer(dir.path(), &Wanted::Checks),
        Answer::Checks(Default::default())
    );
    assert_eq!(
        answer(dir.path(), &Wanted::CheckLog { job: 1 }),
        Answer::Failed(Failure::NoLog)
    );
}

#[test]
fn a_repository_without_a_commit_lists_its_files_and_has_no_base() {
    let dir = tempfile::tempdir().unwrap();
    let repository = dir.path().join("fresh");
    std::fs::create_dir(&repository).unwrap();
    run(&repository, &["init", "--quiet"]);
    std::fs::write(repository.join("first.txt"), "x\n").unwrap();
    let list = listed(answer(
        &repository,
        &Wanted::Files {
            scope: Scope::Uncommitted,
        },
    ));
    assert_eq!(list.total, 1);
    let wanted = Wanted::Files {
        scope: Scope::Branch,
    };
    assert_eq!(
        answer(&repository, &wanted),
        Answer::Failed(Failure::NoBase)
    );
}

#[test]
fn hunk_headers_give_each_sides_start() {
    assert_eq!(hunk_start("@@ -3,4 +5,6 @@ fn main"), Some((3, 5)));
    assert_eq!(hunk_start("@@ -0,0 +1 @@"), Some((0, 1)));
    assert_eq!(hunk_start("diff --git a/x b/x"), None);
}
