//! TDD: hello bootstrap (Task WS4, WS-AC-002).
//! Klien connect → pesan pertama = hello (hosts + latest snapshot).

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

use hiworld_collector::api::{router, AppState, CollectorConfig};
use hiworld_collector::store::Store;
use hiworld_core::models::{Snapshot, SystemMetrics};

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

fn snapshot_of(host: &str, ts: u64) -> Snapshot {
    serde_json::from_str(&format!(
        r#"{{
        "host_id": "{host}", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": 42.0, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        }},
        "processes": []
    }}"#
    ))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn hello_is_first_message_with_latest_snapshot() {
    // siapkan store dengan host + snapshot terakhir
    let store = Store::open_in_memory().unwrap();
    store
        .register_host("web-01", "http://127.0.0.1:9100")
        .unwrap();
    store
        .insert_snapshot(&snapshot_of("web-01", 1_000))
        .unwrap();

    let state = AppState::bootstrap(store, test_config()).unwrap();
    // isi latest untuk hello (simulasi sampler sudah jalan)
    *state.latest_snapshots.lock().unwrap() = vec![snapshot_of("web-01", 1_000)]
        .into_iter()
        .map(|s| (s.host_id.clone(), s))
        .collect();

    let app = router(state.clone());

    // serve di port acak
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // login → cookie
    let login = Request::builder()
        .method("POST")
        .uri(format!("http://{addr}/api/login"))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"initial-pass-123"}"#,
        ))
        .unwrap();
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();
    let res = client.request(login).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
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

    // connect WS dengan cookie
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://{addr}/ws").into_client_request().unwrap();
    req.headers_mut().insert("Cookie", cookie.parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();

    // pesan PERTAMA harus hello
    use futures_util::StreamExt;
    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
        .await
        .expect("timeout")
        .expect("stream berakhir")
        .expect("ws error");
    let text = msg.into_text().unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["type"], "hello", "pesan pertama = hello");
    let hosts = v["data"]["hosts"].as_array().expect("hosts array");
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0]["host_id"], "web-01");
    assert_eq!(hosts[0]["latest"]["timestamp_ms"], 1_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn hello_without_snapshots_still_works() {
    // host terdaftar tapi belum ada snapshot (baru register)
    let store = Store::open_in_memory().unwrap();
    store
        .register_host("fresh", "http://127.0.0.1:9100")
        .unwrap();

    let state = AppState::bootstrap(store, test_config()).unwrap();
    let app = router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let login = Request::builder()
        .method("POST")
        .uri(format!("http://{addr}/api/login"))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"initial-pass-123"}"#,
        ))
        .unwrap();
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();
    let res = client.request(login).await.unwrap();
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

    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://{addr}/ws").into_client_request().unwrap();
    req.headers_mut().insert("Cookie", cookie.parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();

    use futures_util::StreamExt;
    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
        .await
        .expect("timeout")
        .expect("stream berakhir")
        .expect("ws error");
    let text = msg.into_text().unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["type"], "hello");
    let hosts = v["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 1);
    assert!(
        hosts[0]["latest"].is_null(),
        "host tanpa snapshot → latest null"
    );
}
