//! WebSocket endpoint (Task WS3): /ws — auth session, subscribe hub, forward.
//!
//! Format pesan JSON: {"type": "snapshot"|"event"|"host_status", "data": {...}}
//! Auth: session cookie (sama dengan REST) — 401 bila tidak ada (WS-AC-001).

use axum::extract::{FromRequestParts, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;

use crate::api::{AppState, SharedState};
use crate::hub::BroadcastMessage;

/// HTTP fallback bila upgrade gagal / tanpa sesi.
fn unauthorized() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "session required"})),
    )
        .into_response()
}

/// Cek session cookie dari request headers.
fn session_ok(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| {
            c.split(';').find_map(|part| {
                let part = part.trim();
                part.strip_prefix("hiworld_session=")
            })
        })
        .is_some_and(|token| state.sessions.lock().unwrap().contains_key(token))
}

/// Handler upgrade: 401 bila tanpa sesi; else upgrade + subscribe hub.
/// Handler /ws: session dicek SEBELUM upgrade extractor → 401 murni bila
/// tanpa sesi (WS-AC-001). Extractor dijalankan manual agar urutan terkendali.
pub async fn ws_handler(
    State(state): State<SharedState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    if !session_ok(&state, req.headers()) {
        return unauthorized();
    }

    let (mut parts, _body) = req.into_parts();
    let ws = match axum::extract::ws::WebSocketUpgrade::from_request_parts(&mut parts, &state).await
    {
        Ok(ws) => ws,
        Err(err) => return err.into_response(),
    };

    ws.on_upgrade(move |socket| ws_loop(state, socket))
}

/// Loop forward: hub → klien. Klien lambat → Lagged → skip (WS-5).
async fn ws_loop(state: SharedState, mut socket: axum::extract::ws::WebSocket) {
    let mut rx = state.hub.subscribe();

    // WS4: hello bootstrap — daftar SEMUA host terdaftar (store) + snapshot
    // terakhir bila ada (latest_snapshots RAM).
    {
        let registered: Vec<String> = state
            .store
            .lock()
            .unwrap()
            .list_hosts()
            .map(|hs| hs.into_iter().map(|h| h.host_id).collect())
            .unwrap_or_default();
        let hosts: Vec<serde_json::Value> = {
            let latest = state.latest_snapshots.lock().unwrap();
            registered
                .iter()
                .map(|host_id| {
                    serde_json::json!({
                        "host_id": host_id,
                        "latest": latest.get(host_id),
                    })
                })
                .collect()
        }; // guard dropped di sini — sebelum .await
        let hello = serde_json::json!({
            "type": "hello",
            "data": { "hosts": hosts },
        })
        .to_string();
        if socket
            .send(axum::extract::ws::Message::Text(hello.into()))
            .await
            .is_err()
        {
            return;
        }
    }

    // keepalive ping 30s (WS-10) — task terpisah, abort saat loop selesai
    let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<()>(1);
    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if ping_tx.send(()).await.is_err() {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            // pesan dari hub
            recv = rx.recv() => {
                match recv {
                    Ok(msg) => {
                        let json = to_json(&msg);
                        if socket.send(axum::extract::ws::Message::Text(json.into())).await.is_err() {
                            break; // klien putus
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        // klien lambat — skip pesan yang tertinggal (WS-5)
                        tracing::warn!("ws client lagged, skipped {n} messages");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            // klien kirim apa pun (termasuk Pong) → baca agar buffer tidak penuh
            client_msg = socket.recv() => {
                match client_msg {
                    None | Some(Err(_)) => break, // klien putus
                    Some(Ok(_)) => {}             // abaikan pesan dari klien
                }
            }
            // keepalive tick → kirim Ping
            _ = ping_rx.recv() => {
                if socket.send(axum::extract::ws::Message::Ping(bytes::Bytes::new())).await.is_err() {
                    break;
                }
            }
        }
    }

    ping_task.abort();
}

fn to_json(msg: &crate::hub::BroadcastMessage) -> String {
    match msg {
        crate::hub::BroadcastMessage::Snapshot(s) => serde_json::json!({
            "type": "snapshot",
            "data": s,
        }),
        crate::hub::BroadcastMessage::Event {
            host_id,
            kind,
            severity,
            subject,
            detail,
        } => serde_json::json!({
            "type": "event",
            "data": {
                "host_id": host_id,
                "kind": kind,
                "severity": severity,
                "subject": subject,
                "detail": detail,
            }
        }),
        crate::hub::BroadcastMessage::HostStatus { host_id, online } => serde_json::json!({
            "type": "host_status",
            "data": {
                "host_id": host_id,
                "online": online,
            }
        }),
    }
    .to_string()
}

/// Mirror task (WS5): subscribe hub → update AppState.latest_snapshots.
/// Dijalankan sekali di main; sumber "hello" & REST tetap konsisten realtime.
/// Lagged (kiri-kiri pesan saat banjir) → diabaikan, pesan berikutnya tetap
/// diproses (state cukup: yang penting snapshot TERBARU).
pub fn spawn_mirror_task(state: SharedState) -> tokio::task::JoinHandle<()> {
    let mut rx = state.hub.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(BroadcastMessage::Snapshot(s)) => {
                    state
                        .latest_snapshots
                        .lock()
                        .unwrap()
                        .insert(s.host_id.clone(), s);
                }
                Ok(_) => {} // event/host_status tidak masuk latest_snapshots
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("mirror lagged, skipped {n}");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
