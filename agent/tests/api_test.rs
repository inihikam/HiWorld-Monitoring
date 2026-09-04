//! TDD: Agent HTTP API (Task C2) — ARCH-AC-020 (bearer token), ARCH-AC-031
//! (/health tanpa auth). API diuji penuh tanpa server nyata: axum
//! `oneshot` via tower::ServiceExt.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // oneshot

use hiworld_agent::api::{router, AppState};
use hiworld_agent::config::{AgentConfig, SamplingConfig, ServerConfig};
use hiworld_agent::ring_buffer::RingBuffer;
use hiworld_core::models::Snapshot;

fn test_config(token: &str) -> AgentConfig {
    AgentConfig {
        server: ServerConfig {
            bind_addr: "127.0.0.1".into(),
            port: 0,
            auth_token: token.into(),
        },
        sampling: SamplingConfig {
            interval_ms: 10_000,
            top_n_processes: 5,
            collect_pss: false,
        },
    }
}

fn app_with_snapshot(token: &str) -> (axum::Router, Arc<Mutex<AppState>>) {
    let state = Arc::new(Mutex::new(AppState {
        config: test_config(token),
        latest: None,
        backlog: RingBuffer::new(10, std::time::Duration::from_secs(600)),
        started_at_ms: 1_700_000_000_000,
    }));
    (router(state.clone()), state)
}

fn req(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().uri(uri);
    if let Some(t) = token {
        b = b.header("Authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn health_ok_without_auth() {
    let (app, _) = app_with_snapshot("secret");
    let res = app.oneshot(req("/health", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), 1_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["uptime_sec"].is_u64(), "uptime_sec harus angka");
}

#[tokio::test]
async fn snapshot_requires_token() {
    let (app, _) = app_with_snapshot("secret");

    // tanpa token → 401
    let res = app.clone().oneshot(req("/snapshot", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // token salah → 401
    let res = app
        .clone()
        .oneshot(req("/snapshot", Some("wrong")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // token benar, tapi latest belum ada → 503 (belum ada sample; kontrak API)
    let res = app
        .clone()
        .oneshot(req("/snapshot", Some("secret")))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "tanpa sample → 503"
    );
}

#[tokio::test]
async fn snapshot_returns_latest_json() {
    let (app, state) = app_with_snapshot("secret");
    // isi latest dengan snapshot fixture
    let snap: Snapshot = serde_json::from_str(
        r#"{
        "host_id": "web-01", "timestamp_ms": 1700000000000, "interval_ms": 10000,
        "system": {
            "cpu_percent": null, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 100, "mem_used_bytes": 50, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        },
        "processes": []
    }"#,
    )
    .unwrap();
    state.lock().unwrap().latest = Some(snap);

    let res = app.oneshot(req("/snapshot", Some("secret"))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), 1_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["host_id"], "web-01");
}

#[tokio::test]
async fn all_protected_endpoints_reject_bad_token() {
    let (app, _) = app_with_snapshot("secret");
    for uri in ["/snapshot", "/backlog?since=0", "/config", "/metrics"] {
        let res = app.clone().oneshot(req(uri, None)).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{uri} tanpa token");
        let res = app.clone().oneshot(req(uri, Some("salah"))).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{uri} token salah");
    }
}

#[tokio::test]
async fn backlog_returns_array() {
    let (app, _) = app_with_snapshot("secret");
    let res = app
        .oneshot(req("/backlog?since=0", Some("secret")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), 1_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.is_array(), "backlog harus array, dapat {body}");
}

#[tokio::test]
async fn config_get_and_post() {
    let (app, state) = app_with_snapshot("secret");
    let app = &app;

    // GET config
    let res = app
        .clone()
        .oneshot(req("/config", Some("secret")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), 1_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["interval_ms"], 10_000);

    // POST config → ubah interval
    let req = Request::builder()
        .method("POST")
        .uri("/config")
        .header("Authorization", "Bearer secret")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"interval_ms": 1000}"#))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        state.lock().unwrap().config.sampling.interval_ms,
        1000,
        "interval berubah setelah POST (turbo mode)"
    );

    // POST dengan interval invalid (0) → di-clamp, bukan error
    let req = Request::builder()
        .method("POST")
        .uri("/config")
        .header("Authorization", "Bearer secret")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"interval_ms": 0}"#))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        state.lock().unwrap().config.sampling.interval_ms,
        1000,
        "clamp ke minimum 1000ms"
    );
}
