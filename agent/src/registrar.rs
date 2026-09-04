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
