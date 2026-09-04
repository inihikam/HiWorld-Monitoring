//! AgentConfigClient (Task TM3): POST {agent_url}/config — ubah interval runtime.
//! Async reqwest (dipanggil dari poller async — pola TelegramClient async).

/// Trait untuk unit test manager→client (TM-5). Owned param — bebas lifetime.
pub trait AgentConfigSetter: Send + Sync {
    fn set_interval(
        &self,
        agent_url: String,
        agent_token: String,
        interval_ms: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>;
}

pub struct AgentConfigClient {
    http: reqwest::Client,
}

impl AgentConfigClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for AgentConfigClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentConfigSetter for AgentConfigClient {
    fn set_interval(
        &self,
        agent_url: String,
        agent_token: String,
        interval_ms: u64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> {
        let http = self.http.clone();
        let url = format!("{agent_url}/config");
        Box::pin(async move {
            let res = http
                .post(&url)
                .header("Authorization", format!("Bearer {agent_token}"))
                .json(&serde_json::json!({ "interval_ms": interval_ms }))
                .send()
                .await
                .map_err(|e| format!("turbo config network error: {e}"))?;
            match res.status() {
                s if s.is_success() => Ok(()),
                s => Err(format!("turbo config http {s}")),
            }
        })
    }
}
