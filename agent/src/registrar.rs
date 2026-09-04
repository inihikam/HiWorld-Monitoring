//! Registrar (Task SR2–SR3): self-register agent ke collector.
//!
//! Fallback chain host_id (SR-AC-006): /etc/hostname → `hostname` cmd → "unknown".
//! Retry loop backoff ada di sini juga (SR3) — memakai Clock trait agar testable.

/// Sumber hostname yang dapat di-mock (SR-AC-006).
/// None = gagal baca (lebih idiomatik daripada Result<_, ()>).
pub trait HostnameProvider {
    /// Baca /etc/hostname.
    fn hostname_file(&self) -> Option<String>;
    /// Jalankan `hostname` (fallback).
    fn hostname_cmd(&self) -> Option<String>;
}

/// Implementasi produksi.
pub struct SystemHostname;

impl HostnameProvider for SystemHostname {
    fn hostname_file(&self) -> Option<String> {
        std::fs::read_to_string("/etc/hostname")
            .ok()
            .map(|s| s.trim().to_string())
    }

    fn hostname_cmd(&self) -> Option<String> {
        let out = std::process::Command::new("hostname").output().ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8(out.stdout)
            .ok()
            .map(|s| s.trim().to_string())
    }
}

/// Resolve host_id dengan fallback chain. Hasil kosong/whitespace dianggap
/// gagal → lanjut ke sumber berikutnya (SR-AC-006).
pub fn resolve_host_id(provider: &dyn HostnameProvider) -> String {
    [provider.hostname_file(), provider.hostname_cmd()]
        .into_iter()
        .flatten()
        .map(|h| h.trim().to_string())
        .find(|h| !h.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

// ---------- register_once + retry backoff (Task SR3) ----------

/// Error HTTP register (dipisah agar trait RegisterHttp bisa di-mock).
#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("connect gagal: {0}")]
    Connect(String),
    #[error("HTTP {0}")]
    Http(u16),
}

/// Abstraksi HTTP agar retry loop 100% testable tanpa jaringan.
pub trait RegisterHttp {
    /// POST body JSON ke URL; return HTTP status code.
    fn post_register(&self, url: &str, body: &str) -> Result<u16, RegisterError>;
}

/// Abstraksi waktu untuk backoff (impl produksi: std::thread::sleep).
pub trait RegistrarClock {
    fn sleep(&mut self, d: std::time::Duration);
}

/// Semua dependensi registrar — injectable (SR3).
pub struct RegistrarDeps<H: RegisterHttp, C: RegistrarClock> {
    pub http: H,
    pub clock: C,
    pub host_id: String,
    pub agent_url: String,
    pub collector_url: String,
    pub register_token: String,
}

/// Hasil satu siklus registrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAttempt {
    Success,
    GaveUp,
}

/// Cap backoff: 1s → 2s → 4s → … → 60s (SR-3).
const BACKOFF_CAP_MS: u64 = 60_000;

pub struct Registrar<H: RegisterHttp, C: RegistrarClock> {
    pub deps: RegistrarDeps<H, C>,
}

impl<H: RegisterHttp, C: RegistrarClock> Registrar<H, C> {
    pub fn new(deps: RegistrarDeps<H, C>) -> Self {
        Self { deps }
    }

    /// Satu POST register.
    fn register_once(&self) -> Result<(), RegisterError> {
        let url = format!(
            "{}/api/agents/register",
            self.deps.collector_url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "host_id": self.deps.host_id,
            "agent_url": self.deps.agent_url,
            "token": self.deps.register_token,
        })
        .to_string();
        let status = self.deps.http.post_register(&url, &body)?;
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(RegisterError::Http(status))
        }
    }

    /// Loop retry tanpa batas (produksi): backoff 1s→2s→…→60s sampai sukses.
    pub fn run_blocking(&mut self) -> RegisterAttempt {
        self.run_blocking_max_attempts(usize::MAX)
    }

    /// Loop retry dengan batas attempt (test & "coba N kali lalu berhenti").
    pub fn run_blocking_max_attempts(&mut self, max_attempts: usize) -> RegisterAttempt {
        let mut backoff_ms: u64 = 1000;
        for attempt in 1..=max_attempts {
            match self.register_once() {
                Ok(()) => return RegisterAttempt::Success,
                Err(e) => {
                    tracing::warn!(
                        "register attempt {attempt}/{max_attempts} gagal: {e}; next backoff {backoff_ms}ms"
                    );
                }
            }
            if attempt < max_attempts {
                self.deps
                    .clock
                    .sleep(std::time::Duration::from_millis(backoff_ms));
                backoff_ms = (backoff_ms * 2).min(BACKOFF_CAP_MS);
            }
        }
        RegisterAttempt::GaveUp
    }
}

// ---------- impl produksi (dipakai runtime.rs, Task SR4) ----------

/// HTTP produksi via reqwest (rustls). Trait sinkron; block_on di thread blocking.
pub struct SystemRegisterHttp;

impl SystemRegisterHttp {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemRegisterHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl RegisterHttp for SystemRegisterHttp {
    fn post_register(&self, url: &str, body: &str) -> Result<u16, RegisterError> {
        // Trait sinkron; pemanggil (spawn_blocking) sudah di thread terpisah.
        // Gunakan reqwest::blocking agar tidak butuh runtime tokio di sini.
        let resp = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| RegisterError::Connect(e.to_string()))?
            .post(url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(|e| RegisterError::Connect(e.to_string()))?;
        Ok(resp.status().as_u16())
    }
}

/// Clock produksi: thread::sleep (registrar jalan di spawn_blocking).
pub struct SystemRegistrarClock;

impl RegistrarClock for SystemRegistrarClock {
    fn sleep(&mut self, d: std::time::Duration) {
        std::thread::sleep(d);
    }
}
