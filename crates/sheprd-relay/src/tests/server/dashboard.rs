//! The dashboard over HTTP: the login, the cookie it sets, and the stats
//! and metrics behind it.

use super::*;
use crate::secret::Secret;
use crate::serve::metrics_app;
use axum::{
    body::Body,
    extract::connect_info::MockConnectInfo,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn router() -> axum::Router {
    let (router, _hub) = app(&options(Limits::default()), None).unwrap();
    router.layer(MockConnectInfo(SocketAddr::from(([192, 0, 2, 1], 1))))
}

async fn login(router: &axum::Router, body: &str) -> axum::response::Response {
    let request = Request::post("/dashboard/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    router.clone().oneshot(request).await.unwrap()
}

fn location(response: &axum::response::Response) -> &str {
    response.headers()[header::LOCATION].to_str().unwrap()
}

#[tokio::test]
async fn the_page_and_stats_need_a_login() {
    let router = router();
    let page = router
        .clone()
        .oneshot(Request::get("/dashboard").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&page), "/dashboard/login");
    let stats = router
        .clone()
        .oneshot(
            Request::get("/dashboard/api/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_good_login_sets_a_cookie_that_opens_the_stats() {
    let router = router();
    let response = login(
        &router,
        &format!("user=admin&password={}", PASSWORD.replace(' ', "+")),
    )
    .await;
    assert_eq!(location(&response), "/dashboard");
    let set = response.headers()[header::SET_COOKIE].to_str().unwrap();
    let pair = set.split(';').next().unwrap().to_owned();
    assert!(
        response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );

    let stats = router
        .clone()
        .oneshot(
            Request::get("/dashboard/api/stats")
                .header(header::COOKIE, &pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats.status(), StatusCode::OK);
    let body = stats.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["totals"]["sessions"], 0);
    assert_eq!(json["limits"]["max_connections"], 10_000);
}

#[tokio::test]
async fn a_bad_login_fails_and_repeated_ones_are_limited() {
    let router = router();
    let response = login(&router, "user=admin&password=wrong+password").await;
    assert_eq!(location(&response), "/dashboard/login?failed");
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    let mut limited = false;
    for _ in 0..10 {
        let response = login(&router, "user=admin&password=wrong+password").await;
        limited |= location(&response) == "/dashboard/login?limited";
    }
    assert!(limited);
}

#[tokio::test]
async fn the_dashboard_is_absent_without_a_password() {
    let (router, _hub) = app(&Options::default(), None).unwrap();
    let response = router
        .oneshot(Request::get("/dashboard").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_honour_their_token() {
    let hub = Arc::new(Hub::new(Limits::default(), crate::hub::Access::Open));
    let open = metrics_app(Arc::clone(&hub), None)
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(open.status(), StatusCode::OK);

    let token = Secret::new(b"a metrics token!").unwrap();
    let guarded = metrics_app(Arc::clone(&hub), Some(token));
    let refused = guarded
        .clone()
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    let allowed = guarded
        .oneshot(
            Request::get("/metrics")
                .header(header::AUTHORIZATION, "Bearer a metrics token!")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    let body = allowed.into_body().collect().await.unwrap().to_bytes();
    assert!(
        std::str::from_utf8(&body)
            .unwrap()
            .contains("sheprd_relay_sessions 0")
    );
}

#[test]
fn a_client_ip_header_names_the_caller() {
    use crate::web::{Shared, Source};
    let shared = |source| Shared {
        hub: Arc::new(Hub::new(Limits::default(), crate::hub::Access::Open)),
        public_host: None,
        quic: None,
        source,
    };
    let peer = SocketAddr::from(([10, 0, 0, 1], 1));
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("do-connecting-ip", "203.0.113.9".parse().unwrap());
    headers.insert("x-forwarded-for", "198.51.100.1, 10.0.0.1".parse().unwrap());
    let header = Source::Header(axum::http::HeaderName::from_static("do-connecting-ip"));
    assert_eq!(
        shared(header).address(peer, &headers).to_string(),
        "203.0.113.9"
    );
    assert_eq!(
        shared(Source::Proxy).address(peer, &headers).to_string(),
        "10.0.0.1"
    );
    assert_eq!(shared(Source::Peer).address(peer, &headers), peer.ip());
    // A missing or garbled header falls back to the peer.
    let missing = Source::Header(axum::http::HeaderName::from_static("cf-connecting-ip"));
    assert_eq!(shared(missing).address(peer, &headers), peer.ip());
}
