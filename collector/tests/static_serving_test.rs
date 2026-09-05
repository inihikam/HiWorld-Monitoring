//! TDD: static serving + SPA fallback (Task WD2, WD-AC-002, WD-AC-003).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use hiworld_collector::api::{AppState, CollectorConfig};
use hiworld_collector::store::Store;

fn config_with_static(dir: Option<String>) -> CollectorConfig {
    CollectorConfig {
        bind_addr: "127.0.0.1".into(),
        port: 0,
        static_dir: dir,
        provisioning_token: "prov".into(),
        bootstrap_admin_user: "admin".into(),
        bootstrap_admin_pass: "initial-pass-123".into(),
        db_path: ":memory:".into(),
        agent_token: "agent-t".into(),
        poll_interval_ms: 10_000,
        detector: Default::default(),
            telegram: Default::default(),
            turbo: Default::default(),
    }
}

fn app_with(cfg: CollectorConfig) -> axum::Router {
    let store = Store::open_in_memory().unwrap();
    let dir = cfg.static_dir.clone();
    let state = AppState::bootstrap(store, cfg).unwrap();
    hiworld_collector::api::router_with_static(state, &dir)
}

fn make_web_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "wd2-web-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst) + (tag.len() as u32) * 1000
    ));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><html><head><title>hiworld</title></head><body>app</body></html>",
    )
    .unwrap();
    std::fs::write(dir.join("assets/app-abc123.js"), "console.log(1)").unwrap();
    dir
}

#[tokio::test]
async fn root_serves_index_html() {
    let dir = make_web_dir("a");
    let dir_str = dir.to_string_lossy().to_string();
    let app = app_with(config_with_static(Some(dir_str)));

    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("hiworld"), "index.html disajikan");
}

#[tokio::test]
async fn assets_served() {
    let dir = make_web_dir("b");
    let dir_str = dir.to_string_lossy().to_string();
    let app = app_with(config_with_static(Some(dir_str)));

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/app-abc123.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let mime = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(
        mime.starts_with("text/javascript") || mime.starts_with("application/javascript"),
        "mime JS benar, dapat: {mime}"
    );
}

#[tokio::test]
async fn spa_fallback_unknown_path_serves_index() {
    let dir = make_web_dir("c");
    let dir_str = dir.to_string_lossy().to_string();
    let app = app_with(config_with_static(Some(dir_str)));

    // route frontend hash-free (mis. user refresh di /hosts/abc)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/hosts/abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "SPA fallback → index");
    let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("hiworld"));
}

#[tokio::test]
async fn spa_fallback_does_not_shadow_api() {
    let dir = make_web_dir("d");
    let dir_str = dir.to_string_lossy().to_string();
    let app = app_with(config_with_static(Some(dir_str)));

    // API route TIDAK boleh tertimpa fallback → tetap berjalan
    // /api/hosts tanpa sesi → 401 (bukan index.html 200)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "API tidak tertimpa");

    // unknown /api/xyz → 404 (bukan index)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/xyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND, "API unknown tetap 404");
}

#[tokio::test]
async fn without_static_dir_root_is_404() {
    let app = app_with(config_with_static(None));

    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "tanpa static_dir → 404"
    );
    // dan server tidak panic — sudah terbukti jika test sampai sini
}
