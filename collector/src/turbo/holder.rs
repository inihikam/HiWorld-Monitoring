//! TurboHolder (TM4): gabungan manager + client + agent info.
//! Poller memegang ini; Action → POST best-effort (ADR TM-3).

use crate::turbo::client::{AgentConfigClient, AgentConfigSetter};
use crate::turbo::manager::{Action, SystemTurboClock, TurboManager};
pub struct TurboHolder {
    manager: TurboManager<SystemTurboClock>,
    client: AgentConfigClient,
    agent_url: String,
    agent_token: String,
    turbo_interval_ms: u64,
    normal_interval_ms: u64,
}

impl TurboHolder {
    pub fn new(
        manager: TurboManager<SystemTurboClock>,
        client: AgentConfigClient,
        agent_url: String,
        agent_token: String,
        turbo_interval_ms: u64,
    ) -> Self {
        Self {
            manager,
            client,
            agent_url,
            agent_token,
            turbo_interval_ms,
            normal_interval_ms: 10_000, // interval poller (default config)
        }
    }

    /// Set interval normal aktual (dari CollectorConfig.poll_interval_ms).
    pub fn set_normal_interval(&mut self, ms: u64) {
        self.normal_interval_ms = ms;
    }

    /// Event dari detector → push bila spike (async, best-effort).
    pub async fn on_event(&mut self, kind: &str, host: &str) {
        if let Some(Action::PushTurbo) = self.manager.on_event(kind, host) {
            tracing::info!(
                "turbo: push host={host} interval={}ms",
                self.turbo_interval_ms
            );
            if let Err(e) = self
                .client
                .set_interval(
                    self.agent_url.clone(),
                    self.agent_token.clone(),
                    self.turbo_interval_ms,
                )
                .await
            {
                tracing::warn!("turbo push gagal (tetap turbo, retry nanti): {e}");
            }
        }
    }

    /// Snapshot normal → pop bila anti-flap terpenuhi.
    pub async fn on_snapshot(&mut self, host: &str) {
        if let Some(Action::PopTurbo) = self.manager.on_snapshot(host) {
            tracing::info!(
                "turbo: pop host={host} interval={}ms",
                self.normal_interval_ms
            );
            if let Err(e) = self
                .client
                .set_interval(
                    self.agent_url.clone(),
                    self.agent_token.clone(),
                    self.normal_interval_ms,
                )
                .await
            {
                tracing::warn!("turbo pop gagal: {e}");
            }
        }
    }
}
