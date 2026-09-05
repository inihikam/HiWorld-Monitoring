//! TDD: integrasi poller → detector → events → API (Task SD6).
//! Wiremock agent mengirim snapshot dengan spike → assert baris di events
//! + muncul di /api/events.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use hiworld_collector::api::{router, AppState, CollectorConfig};
use hiworld_collector::store::Store;

/// Tiga koneksi ke SATU file DB (WAL) — meniru produksi
/// (main store + poller store + detector store).
fn shared_db() -> (Store, Store, Store, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "sd6-shared-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    (
        Store::open(&path).unwrap(), // state (API)
        Store::open(&path).unwrap(), // poller main
        Store::open(&path).unwrap(), // detector (active_events + baseline)
        path,
    )
}

fn agent_snapshot_json(ts: u64, pid: i32, cpu: f64) -> String {
    format!(
        r#"{{
        "host_id": "spike-host", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": 30.0, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0,
            "disks": [{{"mount":"/","device":"/dev/sda1","total_bytes":100,
                       "used_bytes":95,"percent":95.0,"read_bytes":0,"write_bytes":0}}],
            "net": []
        }},
        "processes": [
            {{"pid": {pid}, "comm": "nginx", "cmdline": "nginx: worker",
              "user": "www-data", "systemd_unit": "nginx.service",
              "cpu_percent": {cpu}, "mem_rss_bytes": 50000000, "mem_pss_bytes": null}}
        ]
    }}"#
    )
}

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
            telegram: Default::default(),
            turbo: Default::default(),
    }
}

#[tokio::test]
async fn poller_detects_and_stores_events() {
    let agent = MockServer::start().await;
    // snapshot dengan cpu spike 91.2% dan disk 95%
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot_json(1_000, 1234, 91.2)),
        )
        .mount(&agent)
        .await;

    let (state_store, poller_store, detector_store, db_path) = shared_db();
    poller_store
        .register_host("spike-host", &agent.uri())
        .unwrap();

    let cfg = test_config();
    let state = AppState::bootstrap(state_store, cfg).unwrap();
    let mut poller = hiworld_collector::poller::Poller::with_detector(
        poller_store,
        "agent-t".into(),
        std::time::Duration::from_millis(10),
        state.config.detector.clone(),
        detector_store,
    );

    poller.poll_once_all().await.unwrap();

    // SD-AC-030: events masuk ke store lewat poller (satu DB bersama)
    let events = state
        .store
        .lock()
        .unwrap()
        .list_events(Some("spike-host"), 100)
        .unwrap();
    let kinds: Vec<&str> = events.iter().map(|(_, _, k, _, _, _)| k.as_str()).collect();
    assert!(
        kinds.contains(&"spike_cpu"),
        "spike_cpu harus tercatat; kinds = {kinds:?}"
    );
    assert!(
        kinds.contains(&"disk_almost_full"),
        "disk_almost_full harus tercatat; kinds = {kinds:?}"
    );
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn events_visible_via_api() {
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot_json(1_000, 1234, 91.2)),
        )
        .mount(&agent)
        .await;

    let (state_store, poller_store, detector_store, _db_path) = shared_db();
    poller_store
        .register_host("spike-host", &agent.uri())
        .unwrap();

    let cfg = test_config();
    let state = AppState::bootstrap(state_store, cfg).unwrap();
    let mut poller = hiworld_collector::poller::Poller::with_detector(
        poller_store,
        "agent-t".into(),
        std::time::Duration::from_millis(10),
        state.config.detector.clone(),
        detector_store,
    );
    poller.poll_once_all().await.unwrap();

    // SD-AC-031: event tampil di /api/events (dengan session)
    let app = router(state);
    let login = Request::builder()
        .method("POST")
        .uri("/api/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"initial-pass-123"}"#,
        ))
        .unwrap();
    let res = app.clone().oneshot(login).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = res
        .headers()
        .get("Set-Cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let req = Request::builder()
        .uri("/api/events")
        .header("Cookie", &cookie)
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), 5_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    let arr = body.as_array().unwrap();
    assert!(
        arr.iter().any(|e| e["kind"] == "spike_cpu"),
        "spike_cpu harus tampil di API"
    );
}

#[tokio::test]
async fn dedup_across_polls_via_sqlite() {
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot_json(2_000, 1234, 91.2)),
        )
        .expect(2)
        .mount(&agent)
        .await;

    let (state_store, poller_store, detector_store, db_path) = shared_db();

    let cfg = test_config();
    let state = AppState::bootstrap(state_store, cfg.clone()).unwrap();
    poller_store
        .register_host("spike-host", &agent.uri())
        .unwrap();
    let mut poller = hiworld_collector::poller::Poller::with_detector(
        poller_store,
        "agent-t".into(),
        std::time::Duration::from_millis(10),
        cfg.detector,
        detector_store,
    );

    poller.poll_once_all().await.unwrap();
    poller.poll_once_all().await.unwrap();

    // spike_cpu hanya 1 baris (dedup), bukan 2 — events di poller store;
    // buka koneksi baru ke file DB yang sama (WAL multi-koneksi, realistis)
    let check_store = Store::open(&db_path).unwrap();
    let events = check_store.list_events(Some("spike-host"), 100).unwrap();
    let spike_count = events
        .iter()
        .filter(|(_, _, k, _, _, _)| k == "spike_cpu")
        .count();
    assert_eq!(
        spike_count, 1,
        "dedup Opsi B: spike aktif tidak diterbitkan ulang; dapat {spike_count}"
    );
    let _ = std::fs::remove_file(&db_path);
}
