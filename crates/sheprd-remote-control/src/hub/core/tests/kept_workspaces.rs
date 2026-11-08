//! The workspaces the app keeps for Herdr 2 terminals, on the phone.

use super::*;

#[test]
fn kept_workspaces_lead_in_their_order_with_their_names_even_when_empty() {
    let saved = serde_json::from_value(json!({
        "workspaces": [
            {"id": "space-2", "scope": "local:/test.sock", "label": "api", "cwd": "/code/api"},
            {"id": "space-1", "scope": "local:/test.sock", "label": "web", "cwd": "/code/web"},
            {"id": "space-9", "scope": "local:/other.sock", "label": "elsewhere", "cwd": "/x"}
        ],
        "tabs": [
            {"id": 4, "scope": "local:/test.sock", "workspace_id": "space-1", "title": "build",
             "location": {"kind": "core_terminal", "panes": "t1"}}
        ]
    }))
    .unwrap();
    let records = ["t1", "t2"].map(|address| serde_json::from_value(record(address)).unwrap());
    let layout = projection::project(
        "fake-core",
        &records,
        Some(&saved),
        Some("local:/test.sock"),
    );
    let rows: Vec<(&str, &str, &str, usize)> = layout
        .workspaces
        .iter()
        .map(|row| {
            (
                row.id.as_str(),
                row.label.as_str(),
                row.cwd.as_str(),
                row.terminals.len(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("space-2", "api", "/code/api", 0),
            ("space-1", "web", "/code/web", 1),
            (UNLISTED, "Herdr 2", "/code/app", 1),
        ],
        "another host's workspace is left out"
    );
    assert_eq!(layout.workspaces[1].terminals[0].label, "build");
}

#[test]
fn kept_workspaces_lead_the_desktop_documents_spaces_too() {
    let dir = tempfile::tempdir().unwrap();
    let tabs = dir.path().join("browser-tabs.json");
    std::fs::write(
        &tabs,
        json!({"tabs": [], "workspaces": [
            {"id": "space-1", "scope": "local:/test.sock", "label": "api", "cwd": "/code/api"}
        ]})
        .to_string(),
    )
    .unwrap();
    let mut poller = crate::hub::core::poller::Poller::default();
    let doc = sheprd_core_client::wire::Doc {
        value: json!({"spaces": [{"id": "s", "folder": "/f", "name": "desk", "tabs": []}]}),
        version: 1,
    };
    poller.read_spaces("fake-core", Ok(doc)).unwrap();
    let layout = poller.project("fake-core", &[], Some(&tabs), Some("local:/test.sock"));
    let labels: Vec<&str> = layout
        .workspaces
        .iter()
        .map(|workspace| workspace.label.as_str())
        .collect();
    assert_eq!(labels, ["api", "desk"]);
}
