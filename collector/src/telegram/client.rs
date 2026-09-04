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
/// reqwest::blocking::Client TIDAK boleh dibuat/di-drop dalam async context —
/// jadi disimpan lazy: struct hanya data, HTTP client dibuat on-demand di
/// thread blocking saat send (OnceLock reuse antar panggilan).
pub struct TelegramClient {
    base_url: String, // default https://api.telegram.org/bot<token>
    http_async: reqwest::Client,
}

static HTTP: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();

/// WAJIB dipanggil di context blocking (dari run_forever spawn_blocking)
/// SEBELUM handle() pertama — reqwest::blocking tidak boleh dibuat/di-drop
/// di async context (ADR TA-2).
pub fn init_shared_http() {
    let _ = HTTP.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    });
}

fn shared_http() -> &'static reqwest::blocking::Client {
    init_shared_http(); // fallback bila lupa init — best effort
    HTTP.get().expect("shared http")
}

impl TelegramClient {
    pub fn new(cfg: &TelegramConfig) -> Self {
        Self {
            base_url: format!("https://api.telegram.org/bot{}", cfg.bot_token),
            http_async: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Untuk test: base URL custom (wiremock).
    pub fn with_base_url(_cfg: &TelegramConfig, base_url: String) -> Self {
        Self {
            base_url,
            http_async: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl TelegramClient {
    /// Versi async — dipakai di context async (produksi poller async).
    pub async fn send_message_async(&self, chat_id: &str, text: &str) -> Result<(), String> {
        let url = format!("{}/sendMessage", self.base_url);
        let res = self
            .http_async
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
            }))
            .send()
            .await
            .map_err(|e| format!("telegram network error: {e}"))?;
        match res.status() {
            s if s.is_success() => Ok(()),
            s => Err(format!("telegram http {s}")),
        }
    }
}

impl TelegramHttp for TelegramClient {
    fn send_message(&self, chat_id: &str, text: &str) -> Result<(), String> {
        let url = format!("{}/sendMessage", self.base_url);
        let res = shared_http()
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
