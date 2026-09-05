//! Runtime integration (Task SR4): menyatukan sampler + registrar + API.
//!
//! Tiga task paralel, saling tidak blocking (SR-AC-008):
//! - sampler loop → isi AppState.latest + backlog
//! - registrar   → retry register ke collector (jika [collector] ada)
//! - API server  → dipanggil dari main.rs

use std::time::Duration;

use crate::api::SharedState;
use crate::config::AgentConfig;
use crate::registrar::{
    resolve_host_id, RegisterAttempt, Registrar, RegistrarDeps, SystemHostname, SystemRegisterHttp,
    SystemRegistrarClock,
};
use crate::sampler::{Sampler, SystemClock, SystemSource};
use hiworld_core::models::Snapshot;

/// Spawn sampler loop: sample tiap interval → update state + backlog.
/// Interval & sampling config diambil dari state.config (turbo-ready).
/// Return JoinHandle agar test bisa abort.
/// Spawn sampler loop di THREAD OS KHUSUS — Sampler persist (prev_cpu/
/// prev_procs bertahan) sehingga delta CPU valid (ADR-1). Snapshot masuk
/// AppState via channel + thread penerus. Interval dari config (turbo-ready).
pub fn spawn_sampler_loop(state: SharedState, min_interval_ms_override: Option<u64>) {
    // Sampler PERSIST: jalankan di THREAD OS KHUSUS (bukan tokio task).
    // Sampler memegang Box<dyn ProcSource> + &mut dyn Clock (tidak Send) —
    // satu thread yang sama memilikinya selamanya → delta CPU valid (ADR-1).
    // Snapshot dikirim ke async world via std::sync::mpsc.
    let (tx, rx) = std::sync::mpsc::channel::<Snapshot>();
    std::thread::spawn(move || {
        let source = SystemSource::new();
        let mut clock = SystemClockTokio;
        let interval_ms = min_interval_ms_override.unwrap_or(10_000).clamp(1, 60_000);
        let mut sampler = Sampler::new(
            Box::new(source),
            &mut clock,
            Duration::from_millis(interval_ms),
            10,
            false,
        );
        loop {
            match sampler.sample_once() {
                Ok(snap) => {
                    if tx.send(snap).is_err() {
                        break; // penerima sudah mati
                    }
                }
                Err(e) => tracing::error!("sample gagal: {e}"),
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
    });
    // penerus snapshot → AppState (async side); recv() blocking di
    // thread khusus agar tidak memblok executor
    let state_rx = state.clone();
    std::thread::spawn(move || {
        while let Ok(snap) = rx.recv() {
            let mut st = state_rx.lock().unwrap();
            st.backlog.push(snap.timestamp_ms, snap.clone());
            st.latest = Some(snap);
        }
    });
}

/// Build registrar task bila [collector] ada; None = tidak ada yang di-spawn
/// (SR-AC-005). `max_attempts_override` untuk test (None = infinite, produksi).
pub fn build_registrar_task_with(
    cfg: &AgentConfig,
    max_attempts_override: Option<usize>,
) -> Option<tokio::task::JoinHandle<()>> {
    let col = cfg.collector.clone()?;
    let host_id = resolve_host_id(&SystemHostname);
    // Q-SR1: agent_url = bind_addr:port, kecuali advertised_url di-set
    let agent_url = col
        .advertised_url
        .clone()
        .unwrap_or_else(|| format!("http://{}:{}", cfg.server.bind_addr, cfg.server.port));

    Some(tokio::spawn(async move {
        let deps = RegistrarDeps {
            http: SystemRegisterHttp::new(),
            clock: SystemRegistrarClock,
            host_id,
            agent_url,
            collector_url: col.url.clone(),
            register_token: col.register_token.clone(),
        };
        let mut registrar = Registrar::new(deps);
        // sekali sukses cukup (Q-SR2) — produksi: retry sampai sukses
        let outcome = tokio::task::spawn_blocking(move || match max_attempts_override {
            Some(n) => registrar.run_blocking_max_attempts(n),
            None => registrar.run_blocking(),
        })
        .await;
        match outcome {
            Ok(RegisterAttempt::Success) => {
                tracing::info!("self-register sukses");
            }
            _ => tracing::error!("self-register berhenti tanpa sukses"),
        }
    }))
}

/// Pasang registrar pipeline; dipanggil main bila [collector] ada.
pub fn spawn_registrar(cfg: &AgentConfig) -> tokio::task::JoinHandle<()> {
    spawn_registrar_with(cfg, None)
}

/// Varian dengan batas attempt (test).
pub fn spawn_registrar_with(
    cfg: &AgentConfig,
    max_attempts: Option<usize>,
) -> tokio::task::JoinHandle<()> {
    match build_registrar_task_with(cfg, max_attempts) {
        Some(h) => h,
        None => tokio::spawn(async {}), // no-op handle
    }
}

// ---------- impl produksi untuk trait registrar ----------

pub struct SystemClockTokio;

impl crate::sampler::Clock for SystemClockTokio {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    fn sleep(&mut self, _d: Duration) {
        // sampler loop memakai tokio::time::sleep sendiri; ini stub
    }
}
