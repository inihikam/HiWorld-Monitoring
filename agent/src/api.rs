//! Agent HTTP API (Task C2): /snapshot, /backlog, /config, /metrics, /health.
//!
//! Auth: Bearer token di semua endpoint KECUALI /health (ARCH-AC-020/031).
//! Rate-limit 401 (ARCH-AC-022) menyusul di task lanjutan.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use crate::config::AgentConfig;
use crate::ring_buffer::RingBuffer;
use hiworld_core::models::Snapshot;

/// State bersama yang dipegang handler API & sampler loop.
pub struct AppState {
    pub config: AgentConfig,
    pub latest: Option<Snapshot>,
    pub backlog: RingBuffer<Snapshot>,
    pub started_at_ms: u64,
}

pub type SharedState = Arc<Mutex<AppState>>;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/snapshot", get(snapshot))
        .route("/backlog", get(backlog))
        .route("/config", get(get_config).post(post_config))
        .route("/metrics", get(metrics))
        .with_state(state)
}

/// Cek bearer token → Ok(()) atau 401.
fn check_auth(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let ok = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|tok| tok == state.config.server.auth_token);
    if ok {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn health(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.lock().unwrap();
    let uptime = st
        .config
        .sampling
        .interval_ms
        .checked_mul(0) // placeholder: dihitung dari now - started_at di C3
        .unwrap_or(0);
    let _ = uptime;
    let uptime_sec = 0u64; // C3: hitung dari started_at vs SystemTime
    Json(serde_json::json!({
        "status": "ok",
        "uptime_sec": uptime_sec,
    }))
}

async fn snapshot(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let st = state.lock().unwrap();
    check_auth(&st, &headers)?;
    match &st.latest {
        Some(snap) => Ok(Json(snap.clone())),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[derive(serde::Deserialize)]
struct BacklogParams {
    since: u64,
}

async fn backlog(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(p): Query<BacklogParams>,
) -> Result<impl IntoResponse, StatusCode> {
    let st = state.lock().unwrap();
    check_auth(&st, &headers)?;
    // clone snapshot (bukan referensi) agar tidak bergantung pada lock
    let items: Vec<Snapshot> = st
        .backlog
        .backlog_since(p.since)
        .into_iter()
        .map(|(_, s)| s.clone())
        .collect();
    Ok(Json(items))
}

async fn get_config(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let st = state.lock().unwrap();
    check_auth(&st, &headers)?;
    Ok(Json(serde_json::json!({
        "interval_ms": st.config.sampling.interval_ms,
        "top_n_processes": st.config.sampling.top_n_processes,
        "collect_pss": st.config.sampling.collect_pss,
    })))
}

#[derive(serde::Deserialize)]
struct ConfigPatch {
    interval_ms: u32,
}

async fn post_config(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(patch): Json<ConfigPatch>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut st = state.lock().unwrap();
    check_auth(&st, &headers)?;
    // clamp 1000..=60000 (konsisten dengan AgentConfig::validated)
    let ms = (patch.interval_ms as u64).clamp(1000, 60_000) as u32;
    st.config.sampling.interval_ms = ms;
    Ok(Json(serde_json::json!({
        "interval_ms": st.config.sampling.interval_ms,
    })))
}

async fn metrics(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let st = state.lock().unwrap();
    check_auth(&st, &headers)?;
    match &st.latest {
        Some(snap) => Ok((
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            crate::metrics::render_prometheus(snap),
        )),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Konversi ring buffer max_age dari detik config → Duration.
pub fn ring_buffer_from_config() -> RingBuffer<Snapshot> {
    // PDD §7.1: max_age_sec = 600, max_entries = 600 (default)
    RingBuffer::new(600, Duration::from_secs(600))
}
