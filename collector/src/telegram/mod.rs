//! Telegram alert (Task TA1+): konfigurasi, klien, format, bridge.
//!
//! Konsep (PDD §2): collector menyimak event hasil detector → filter
//! severity + rate limit → kirim via Bot API. Telegram best-effort:
//! DB tetap source of truth (ADR TA-3).

pub mod bridge;
pub mod client;
pub mod format;

use serde::Deserialize;

/// Severity event untuk filter (ordering: info < warning < critical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl<'de> serde::Deserialize<'de> for Severity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Severity::parse(&s)) // unknown → Warning fallback (TA-AC-003)
    }
}

impl Severity {
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "info" => Severity::Info,
            "critical" => Severity::Critical,
            _ => Severity::Warning, // fallback default (TA-AC-003)
        }
    }
}

/// Config section `[telegram]` (PDD §2.2) — semua default aman (TA-7).
#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
    /// default impl manual via serde(default = ...) — lihat impl Default
    #[serde(default = "default_min_severity", skip_serializing)]
    pub min_severity: Severity,
    #[serde(default = "default_max_per_minute")]
    pub max_per_minute: u32,
}

fn default_min_severity() -> Severity {
    Severity::Warning
}
fn default_max_per_minute() -> u32 {
    20
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: String::new(),
            chat_id: String::new(),
            min_severity: Severity::Warning,
            max_per_minute: 20,
        }
    }
}

impl TelegramConfig {
    /// Bridge hanya jalan bila enabled + token + chat lengkap (TA-AC-002).
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.bot_token.is_empty() && !self.chat_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_parse() {
        assert_eq!(Severity::parse("info"), Severity::Info);
        assert_eq!(Severity::parse("CRITICAL"), Severity::Critical);
        assert_eq!(Severity::parse("unknown"), Severity::Warning);
    }
}
