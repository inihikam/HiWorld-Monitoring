//! TelegramClient (Task TA3): kirim pesan via Bot API.
//! Trait TelegramHttp injectable — mock di unit test, wiremock di integration.

use crate::telegram::TelegramConfig;

/// Abstraksi HTTP agar bridge testable tanpa jaringan (ADR TA-6).
pub trait TelegramHttp: Send + Sync {
    /// Kirim plain text ke chat; Ok(()) = 2xx.
    fn send_message(&self, chat_id: &str, text: &str) -> Result<(), String>;
}

/// Blanket impl: wrapper Arc otomatis TelegramHttp (pola poller store).
impl<T: TelegramHttp> TelegramHttp for std::sync::Arc<T> {
    fn send_message(&self, chat_id: &str, text: &str) -> Result<(), String> {
        (**self).send_message(chat_id, text)
    }
}

/// Client nyata: reqwest blocking (dipanggil dalam spawn_blocking — pola poller, ADR TA-2).
pub struct TelegramClient {
    base_url: String, // default https://api.telegram.org/bot<token>
    http: reqwest::blocking::Client,
}

impl TelegramClient {
    pub fn new(cfg: &TelegramConfig) -> Self {
        Self {
            base_url: format!("https://api.telegram.org/bot{}", cfg.bot_token),
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Untuk test: base URL custom (wiremock).
    pub fn with_base_url(_cfg: &TelegramConfig, base_url: String) -> Self {
        Self {
            base_url,
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl TelegramHttp for TelegramClient {
    fn send_message(&self, chat_id: &str, text: &str) -> Result<(), String> {
        let url = format!("{}/sendMessage", self.base_url);
        let res = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                // TA-5: plain text — tanpa parse_mode
            }))
            .send()
            .map_err(|e| format!("telegram network error: {e}"))?;

        match res.status() {
            s if s.is_success() => Ok(()),
            s => Err(format!("telegram http {s}")),
        }
    }
}
