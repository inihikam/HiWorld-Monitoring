//! Turbo mode (Task TM1+): sampling adaptif saat spike.
//!
//! Konsep (PDD §2): event spike_cpu/mem → collector POST /config ke agent
//! dengan interval turbo; kembali normal (streak 2x + min_duration) →
//! interval normal. Per-host, best-effort (ADR TM-1..TM-6).

pub mod client;
pub mod manager;

/// Config section `[turbo]` — semua default aman (TM-6).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TurboConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Interval turbo; clamp 1000..=60000 (sama dengan agent validator).
    /// Pelajaran serde: `default` + `deserialize_with` — bila field hilang,
    /// default fn dipakai (bukan clamp fn). Default = 1000 (Q-TM2).
    #[serde(default = "default_interval", deserialize_with = "clamp_interval")]
    pub interval_ms: u64,
    /// Minimal durasi turbo sebelum boleh pop (anti-flap).
    #[serde(default = "default_min_duration")]
    pub min_duration_ms: u64,
}

fn default_min_duration() -> u64 {
    60_000
}
fn default_interval() -> u64 {
    1000
}

/// serde deserializer dengan clamp 1000..=60000 (TM-AC-002).
fn clamp_interval<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <u64 as serde::Deserialize>::deserialize(deserializer)?;
    Ok(raw.clamp(1000, 60_000))
}

impl Default for TurboConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_ms: 1000, // Q-TM2: 1 detik
            min_duration_ms: 60_000,
        }
    }
}
