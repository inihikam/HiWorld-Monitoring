//! Collector HTTP API (Task D5): REST + session auth + register.
//!
//! - `/health` tanpa auth (ARCH-AC-031)
//! - `/api/agents/register` — provisioning token (Q1, ARCH-AC-021)
//! - `/api/login` — bcrypt verify → session cookie (ARCH-AC-023)
//! - `/api/account/password` — ganti password (butuh sesi + password lama)
//! - `/api/hosts`, `/api/history`, `/api/events` — butuh sesi
//! - `/ws` — broadcast realtime (dikerjakan di web-dashboard task berikutnya
//!   karena butuh klien WS untuk diuji; fondasi broadcast ada di sini)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::auth::Auth;
use crate::store::Store;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CollectorConfig {
    pub bind_addr: String,
    pub port: u16,
    #[serde(default)]
    pub static_dir: Option<String>,
    pub provisioning_token: String,
    pub bootstrap_admin_user: String,
    pub bootstrap_admin_pass: String,
    pub db_path: String,
    /// Token bearer yang dipakai collector memanggil agent.
    pub agent_token: String,
    /// Interval poll ke agent (ms).
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    /// Konfigurasi spike detection (SD1 — docs/specs/spike-detection.md §3.4).
    #[serde(default)]
    pub detector: DetectorConfig,
}

fn default_poll_interval() -> u64 {
    10_000
}

/// Threshold & parameter spike detection (semua bisa dioverride per deploy).
/// Default = nilai PDD §3.4 (Default impl manual, BUKAN derive — derive
/// menghasilkan 0.0 dan mematahkan kasus section hilang).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct DetectorConfig {
    #[serde(default = "def_cpu_threshold")]
    pub cpu_spike_threshold_percent: f64,
    #[serde(default = "def_mem_threshold")]
    pub mem_spike_threshold_percent: f64,
    #[serde(default = "def_disk_threshold")]
    pub disk_full_threshold_percent: f64,
    #[serde(default = "def_baseline_window")]
    pub baseline_window_min: u32,
    #[serde(default = "def_min_samples")]
    pub min_samples_for_baseline: u32,
}

fn def_cpu_threshold() -> f64 {
    85.0
}
fn def_mem_threshold() -> f64 {
    10.0
}
fn def_disk_threshold() -> f64 {
    90.0
}
fn def_baseline_window() -> u32 {
    30
}
fn def_min_samples() -> u32 {
    3
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            cpu_spike_threshold_percent: def_cpu_threshold(),
            mem_spike_threshold_percent: def_mem_threshold(),
            disk_full_threshold_percent: def_disk_threshold(),
            baseline_window_min: def_baseline_window(),
            min_samples_for_baseline: def_min_samples(),
        }
    }
}

pub struct AppState {
    /// rusqlite Connection tidak Sync → Mutex (v1; v2: r2d2 pool bila butuh paralelisme)
    pub store: Mutex<Store>,
    pub config: CollectorConfig,
    pub auth: Mutex<Auth>,
    pub started_at_ms: u64,
    /// sesi aktif: token acak → username
    pub sessions: Mutex<HashMap<String, String>>,
    /// Broadcast realtime (WS1/WS3) — klien dashboard subscribe di sini.
    pub hub: crate::hub::BroadcastHub,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    /// Buka store + auth + bootstrap admin dari config.
    ///
    /// CATATAN v1: auth memakai koneksi DB TERPISAH (in-memory) dari store —
    /// cukup karena keduanya dalam satu proses. TODO D6+: satukan file DB
    /// (WAL mendukung multi-koneksi).
    pub fn bootstrap(store: Store, config: CollectorConfig) -> crate::store::Result<Arc<Self>> {
        let auth =
            Auth::bootstrap_in_memory(&config.bootstrap_admin_user, &config.bootstrap_admin_pass)
                .map_err(|e| {
                eprintln!("auth bootstrap gagal: {e}");
                rusqlite::Error::InvalidQuery
            })?;
        Ok(Arc::new(Self {
            store: Mutex::new(store),
            config,
            auth: Mutex::new(auth),
            started_at_ms: now_ms(),
            sessions: Mutex::new(HashMap::new()),
            hub: crate::hub::BroadcastHub::new(1024),
        }))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/agents/register", post(register))
        .route("/api/login", post(login))
        .route("/api/account/password", post(change_password))
        .route("/api/hosts", get(list_hosts))
        .route("/api/history", get(history))
        .route("/api/events", get(events))
        .route("/ws", get(crate::ws::ws_handler))
        .with_state(state)
}

// ---------- health ----------

#[axum::debug_handler]
async fn health(State(state): State<SharedState>) -> impl IntoResponse {
    let uptime_sec = now_ms().saturating_sub(state.started_at_ms) / 1000;
    Json(serde_json::json!({ "status": "ok", "uptime_sec": uptime_sec }))
}

// ---------- register (agent → collector) ----------

#[derive(serde::Deserialize)]
struct RegisterBody {
    host_id: String,
    agent_url: String,
    token: String,
}

async fn register(
    State(state): State<SharedState>,
    Json(body): Json<RegisterBody>,
) -> Result<impl IntoResponse, StatusCode> {
    if body.token != state.config.provisioning_token {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let id = state
        .store
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .register_host(&body.host_id, &body.agent_url)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "id": id, "registered": true })))
}

// ---------- session auth ----------

fn session_user(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("Cookie")?.to_str().ok()?;
    let token = cookie.split(';').find_map(|c| {
        let c = c.trim();
        c.strip_prefix("hiworld_session=")
    })?;
    state.sessions.lock().unwrap().get(token).cloned()
}

fn require_session(state: &AppState, headers: &HeaderMap) -> Result<String, StatusCode> {
    session_user(state, headers).ok_or(StatusCode::UNAUTHORIZED)
}

fn random_token() -> String {
    // v1: waktu + pid + counter. v2: rand::rngs::OsRng (buat issue).
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", now_ms(), std::process::id(), n)
}

#[derive(serde::Deserialize)]
struct LoginBody {
    username: String,
    password: String,
}

async fn login(
    State(state): State<SharedState>,
    Json(body): Json<LoginBody>,
) -> Result<impl IntoResponse, StatusCode> {
    let ok = state
        .auth
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .verify_login(&body.username, &body.password)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !ok {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let token = random_token();
    state
        .sessions
        .lock()
        .unwrap()
        .insert(token.clone(), body.username);
    Ok((
        [(
            axum::http::header::SET_COOKIE,
            format!("hiworld_session={token}; HttpOnly; SameSite=Lax; Path=/"),
        )],
        Json(serde_json::json!({ "login": true })),
    ))
}

#[derive(serde::Deserialize)]
struct ChangePasswordBody {
    old_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = require_session(&state, &headers)?;
    state
        .auth
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .change_password(&user, &body.old_password, &body.new_password)
        .map_err(|e| match e {
            crate::auth::AuthError::WrongOldPassword => StatusCode::UNAUTHORIZED,
            crate::auth::AuthError::EmptyPassword | crate::auth::AuthError::TooShort => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;
    Ok(Json(serde_json::json!({ "changed": true })))
}

// ---------- data endpoints (butuh sesi) ----------

async fn list_hosts(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    require_session(&state, &headers)?;
    let hosts = state
        .store
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .list_hosts()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        hosts
            .iter()
            .map(|h| {
                serde_json::json!({
                    "id": h.id,
                    "host_id": h.host_id,
                    "agent_url": h.agent_url,
                    "registered_at_ms": h.registered_at_ms,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

#[derive(serde::Deserialize)]
struct HistoryParams {
    host: String,
    from: u64,
    to: u64,
}

async fn history(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(p): Query<HistoryParams>,
) -> Result<impl IntoResponse, StatusCode> {
    require_session(&state, &headers)?;
    let rows = state
        .store
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .query_system_history(&p.host, p.from, p.to)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "timestamp_ms": r.timestamp_ms,
                    "cpu_percent": if r.cpu_percent.is_nan() { serde_json::Value::Null } else { serde_json::json!(r.cpu_percent) },
                    "mem_used_bytes": r.mem_used_bytes,
                    "mem_total_bytes": r.mem_total_bytes,
                    "mem_percent": r.mem_percent,
                    "load_1m": r.load_1m,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

#[derive(serde::Deserialize)]
struct EventsParams {
    #[serde(default)]
    host: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

async fn events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(p): Query<EventsParams>,
) -> Result<impl IntoResponse, StatusCode> {
    require_session(&state, &headers)?;
    let rows = state
        .store
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .list_events(p.host.as_deref(), p.limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        rows.into_iter()
            .map(|(host, ts, kind, sev, subject, detail)| {
                serde_json::json!({
                    "host_id": host,
                    "timestamp_ms": ts,
                    "kind": kind,
                    "severity": sev,
                    "subject": subject,
                    "detail": detail,
                })
            })
            .collect::<Vec<_>>(),
    ))
}
