//! TDD: POST /api/logout (Task WD1, WD-AC-012).
//! Login → logout → endpoint terlindungi 401 kembali.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use hiworld_collector::api::{router, AppState, CollectorConfig};
use hiworld_collector::store::Store;

fn test_config() -> CollectorConfig {
    CollectorConfig {
        bind_addr: "127.0.0.1".into(),
        port: 0,
        static_dir: None,
        provisioning_token: "prov".into(),
        bootstrap_admin_user: "admin".into(),
        bootstrap_admin_pass: "initial-pass-123".into(),
        db_path: ":memory:".into(),
        agent_token: "agent-t".into(),
        poll_interval_ms: 10_000,
        detector: Default::default(),
    }
}

fn app() -> axum::Router {
    let store = Store::open_in_memory().unwrap();
    let state = AppState::bootstrap(store, test_config()).unwrap();
    router(state)
}

async fn login(app: &axum::Router) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"initial-pass-123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "login harus sukses");
    res.headers()
        .get("Set-Cookie")
        .expect("Set-Cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn logout_invalidates_session() {
    let app = app();
    let cookie = login(&app).await;

    // sebelum logout: /api/hosts 200
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .header("Cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // logout
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logout")
                .header("Cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "logout 200");

    // sesudah logout: /api/hosts 401
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .header("Cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "session invalid");
}

#[tokio::test]
async fn logout_without_cookie_still_200() {
    // logout tanpa sesi idempotent — tidak error
    let app = app();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
