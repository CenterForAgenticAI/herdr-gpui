//! `POST /api/v2/term/{pane}/convert`: a refusal is a `409` the phone shows.

use super::*;

#[tokio::test]
async fn a_pane_its_tab_does_not_offer_is_a_conflict_with_a_message() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path());
    let (_, token) = signed_in(&state, 7);
    let request = post("/api/v2/term/t2-p1/convert", json!({}));
    let (status, _, body) = send(&state, terminal::authorized(request, &token)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    // Windows refuses every move first, as its typed line needs `sh`.
    if cfg!(unix) {
        assert!(body.contains("the pane runs no agent"), "{body}");
    }
}

#[tokio::test]
async fn moving_needs_a_signed_in_phone() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path());
    let (status, _, _) = send(&state, post("/api/v2/term/t2-p1/convert", json!({}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
