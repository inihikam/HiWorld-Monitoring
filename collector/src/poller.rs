//! Poller collector (Task D4): poll semua agent terdaftar, simpan ke store,
//! tandai offline saat gagal, backfill dari backlog agent saat gap (Q2/ADR-5).

use std::collections::HashMap;
use std::time::Duration;

use hiworld_core::models::Snapshot;

use crate::store::Store;

#[derive(Debug, Clone, Default)]
pub struct HostStatus {
    pub online: bool,
    pub consecutive_failures: u32,
    pub last_ok_ms: Option<u64>,
    pub last_error: Option<String>,
}

impl HostStatus {
    pub fn is_offline(&self) -> bool {
        !self.online
    }
}

pub struct Poller {
    store: Store,
    agent_token: String,
    interval: Duration,
    client: reqwest::Client,
    statuses: HashMap<String, HostStatus>,
    last_seen: HashMap<String, u64>,
    /// Batas gap untuk memicu backfill: bila (snapshot_ts - last_seen) melebihi
    /// 2× interval, tarik /backlog?since=last_seen. Di bawah itu cukup /snapshot.
    backfill_threshold: Duration,
}

impl Poller {
    pub fn new(store: Store, agent_token: String, interval: Duration) -> Self {
        Self {
            store,
            agent_token,
            interval,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
            statuses: HashMap::new(),
            last_seen: HashMap::new(),
            backfill_threshold: interval * 2,
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn set_last_seen(&mut self, host_id: &str, ts_ms: u64) {
        self.last_seen.insert(host_id.to_string(), ts_ms);
    }

    pub fn host_status(&self, host_id: &str) -> HostStatus {
        self.statuses.get(host_id).cloned().unwrap_or_default()
    }

    /// Poll semua host terdaftar satu putaran (dipakai test & loop produksi).
    pub async fn poll_once_all(&mut self) -> Result<(), String> {
        let hosts = self.store.list_hosts().map_err(|e| e.to_string())?;
        for host in hosts {
            self.poll_host(&host.host_id, &host.agent_url).await;
        }
        Ok(())
    }

    /// Loop produksi: poll tiap interval selamanya.
    pub async fn run_forever(mut self) {
        let mut ticker = tokio::time::interval(self.interval);
        loop {
            ticker.tick().await;
            if let Err(e) = self.poll_once_all().await {
                tracing::error!("poll loop error: {e}");
            }
        }
    }

    async fn poll_host(&mut self, host_id: &str, agent_url: &str) {
        let url = format!("{agent_url}/snapshot");
        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.agent_token))
            .send()
            .await;

        match res {
            Ok(resp) if resp.status().is_success() => match resp.json::<Snapshot>().await {
                Ok(snap) => {
                    let snap_ts = snap.timestamp_ms;
                    let since = self.last_seen.get(host_id).copied();
                    if let Err(e) = self
                        .store_snapshot_and_backfill(host_id, agent_url, snap, since)
                        .await
                    {
                        self.mark_failure(host_id, &format!("store/backfill: {e}"));
                        return;
                    }
                    self.mark_success(host_id, snap_ts);
                }
                Err(e) => self.mark_failure(host_id, &format!("decode: {e}")),
            },
            Ok(resp) => self.mark_failure(host_id, &format!("HTTP {}", resp.status())),
            Err(e) => self.mark_failure(host_id, &format!("connect: {e}")),
        }
    }

    /// Simpan snapshot terbaru; bila gap terdeteksi (ts - last_seen > threshold),
    /// tarik backlog agent sejak last_seen dan insert (Q2: backfill v1).
    async fn store_snapshot_and_backfill(
        &self,
        _host_id: &str,
        agent_url: &str,
        snap: Snapshot,
        since: Option<u64>,
    ) -> Result<(), String> {
        let is_gap = since.is_some_and(|last| {
            snap.timestamp_ms.saturating_sub(last) > self.backfill_threshold.as_millis() as u64
        });

        // simpan snapshot terbaru dulu
        self.store
            .insert_snapshot(&snap)
            .map_err(|e| e.to_string())?;

        if is_gap {
            let since_ms = since.unwrap_or(0);
            let url = format!("{agent_url}/backlog?since={since_ms}");
            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.agent_token))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("backlog HTTP {}", resp.status()));
            }
            let backlog: Vec<Snapshot> = resp.json().await.map_err(|e| e.to_string())?;
            for old in backlog {
                // snapshot >= snap terbaru sudah tersimpan; backlog hanya yang lebih lama
                if old.timestamp_ms < snap.timestamp_ms {
                    self.store
                        .insert_snapshot(&old)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }

    fn mark_success(&mut self, host_id: &str, ts: u64) {
        let st = self.statuses.entry(host_id.to_string()).or_default();
        st.online = true;
        st.consecutive_failures = 0;
        st.last_ok_ms = Some(ts);
        st.last_error = None;
        self.last_seen.insert(host_id.to_string(), ts);
    }

    fn mark_failure(&mut self, host_id: &str, err: &str) {
        let was_online = self
            .statuses
            .get(host_id)
            .map(|s| s.online)
            .unwrap_or(false);
        let st = self.statuses.entry(host_id.to_string()).or_default();
        st.online = false;
        st.consecutive_failures += 1;
        st.last_error = Some(err.to_string());

        // event AgentDown HANYA sekali saat transisi online→offline (PDD §8)
        if was_online || st.consecutive_failures == 1 {
            let _ = self.store.insert_event(
                host_id,
                "agent_down",
                "warning",
                host_id,
                &serde_json::json!({"error": err}),
            );
        }
    }
}
