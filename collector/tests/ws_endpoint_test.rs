//! TDD: endpoint /ws — upgrade, auth session, fan-out (Task WS3).

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use hiworld_collector::api::{router, AppState, CollectorConfig};
use hiworld_collector::hub::BroadcastHub;
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
            telegram: Default::default(),
            turbo: Default::default(),
    }
}

fn app() -> (axum::Router, Arc<AppState>) {
    let store = Store::open_in_memory().unwrap();
    let state = AppState::bootstrap(store, test_config()).unwrap();
    (router(state.clone()), state)
}

async fn login_cookie(app: &axum::Router) -> String {
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
    res.headers()
        .get("Set-Cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

// ---------- WS-AC-001: tanpa sesi → 401 ----------

#[tokio::test]
async fn ws_without_session_rejected() {
    let (app, _state) = app();
    let req = Request::builder()
        .uri("/ws")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ---------- WS-AC-006: dua klien terima broadcast ----------

#[tokio::test]
async fn two_ws_clients_receive_broadcast() {
    let (app, state) = app();
    let cookie = login_cookie(&app).await;

    // 2 koneksi WS nyata via tokio-tungstenite ke server axum yang di-serve
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // fungsi koneksi kecil
    async fn connect(
        cookie: &str,
        addr: &std::net::SocketAddr,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws?session={cookie}"))
            .await
            .unwrap_or_else(|e| panic!("connect gagal: {e}"));
        ws
    }
    let _ = connect; // dipakai di bawah via cookie header, bukan query (lihat catatan)

    // Connect via TCP manual dengan header Cookie (axum WS ekstrak dari request)
    let mut c1 = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .ok()
        .map(|(ws, _)| ws);
    let _ = &mut c1;

    // NOTE: connect_async tidak bisa kirim header Cookie di versi lama;
    // gunakan builder request:
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req1 = format!("ws://{addr}/ws").into_client_request().unwrap();
    req1.headers_mut().insert("Cookie", cookie.parse().unwrap());
    let mut req2 = format!("ws://{addr}/ws").into_client_request().unwrap();
    req2.headers_mut().insert("Cookie", cookie.parse().unwrap());

    let (mut ws1, _) = tokio_tungstenite::connect_async(req1).await.unwrap();
    let (mut ws2, _) = tokio_tungstenite::connect_async(req2).await.unwrap();

    // broadcast manual via hub (unit-level trigger)
    state.hub.send(BroadcastMessage::HostStatus {
        host_id: "h1".into(),
        online: true,
    });

    use futures_util::StreamExt;
    for ws in [&mut ws1, &mut ws2] {
        let mut v = None;
        for _ in 0..10 {
            let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
                .await
                .expect("timeout")
                .expect("stream berakhir")
                .expect("ws error");
            let Ok(text) = msg.into_text() else { continue }; // skip Ping/Binary
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if parsed["type"] == "host_status" {
                v = Some(parsed);
                break;
            }
        }
        let v = v.expect("host_status tidak diterima");
        assert_eq!(v["data"]["host_id"], "h1");
        assert_eq!(v["data"]["online"], true);
    }
}

use hiworld_collector::hub::BroadcastMessage;

// ---------- WS-AC-008: disconnect bersih ----------

#[tokio::test]
async fn client_disconnect_keeps_others_working() {
    let (app, state) = app();
    let cookie = login_cookie(&app).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mk_req = || {
        let mut r = format!("ws://{addr}/ws").into_client_request().unwrap();
        r.headers_mut().insert("Cookie", cookie.parse().unwrap());
        r
    };

    let (mut ws_keep, _) = tokio_tungstenite::connect_async(mk_req()).await.unwrap();
    let (mut ws_drop, _) = tokio_tungstenite::connect_async(mk_req()).await.unwrap();

    // klien 2 putus
    ws_drop.close(None).await.unwrap();

    // broadcast tetap sampai ke klien 1
    state.hub.send(BroadcastMessage::Event {
        host_id: "h".into(),
        kind: "spike_cpu".into(),
        severity: "warning".into(),
        subject: "s".into(),
        detail: serde_json::json!({}),
    });

    use futures_util::StreamExt;
    let mut v = None;
    for _ in 0..10 {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws_keep.next())
            .await
            .expect("timeout")
            .expect("stream berakhir")
            .expect("ws error");
        let Ok(text) = msg.into_text() else { continue };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if parsed["type"] == "event" {
            v = Some(parsed);
            break;
        }
    }
    let v = v.expect("event tidak diterima");
    assert_eq!(v["data"]["kind"], "spike_cpu");
}
