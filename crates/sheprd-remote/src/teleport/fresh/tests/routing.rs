use super::*;

#[test]
fn preparing_a_kept_destination_ships_commit_and_returns_git_routing() {
    let fixture = fixture();
    let origin = Origin {
        place: Place {
            endpoint_id: "source".into(),
            label: "Source".into(),
            host: host(),
        },
        workspace_id: "kept-source".into(),
        repo_key: fixture.key.clone(),
        repo_label: "project".into(),
    };
    let destination = HostRepositories {
        place: Place {
            endpoint_id: "destination".into(),
            label: "Destination".into(),
            host: host(),
        },
        repositories: Some(vec![Repository {
            key: fixture.destination.clone(),
            label: "project".into(),
            workspace_id: "kept-destination".into(),
        }]),
        retired: Vec::new(),
        lands_in: sheprd_session::Keeper::App,
    };
    let cancelled = AtomicBool::new(false);
    let prepared = prepare(
        &origin,
        &destination,
        &fixture.feature,
        &mut |_| {},
        &cancelled,
    )
    .unwrap();
    let work::Repository::Herdr2 { checkout } = prepared.work_repository() else {
        panic!("kept IDs must never reach Herdr 1");
    };
    assert!(prepared.repository.workspace_id.is_empty());
    assert_eq!(
        git(Path::new(checkout), &["rev-parse", &fixture.feature]),
        fixture.feature
    );
    assert!(prepared.reference.is_some());
    prepared.finish(&cancelled);
    assert_eq!(
        git(
            Path::new(&fixture.destination),
            &["for-each-ref", "refs/herdr-teleport"]
        ),
        ""
    );
}
