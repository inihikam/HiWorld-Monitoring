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
    /// Spike detector (SD6) — None = deteksi nonaktif.
    /// Detector stateless; store aktif & baseline diberikan per evaluasi.
    /// Dua koneksi DB: main store (mut) + detector store (read baseline).
    detector: Option<crate::detector::Detector<PollerClock>>,
    detector_store: Option<Store>,
    /// Broadcast realtime (WS1) — None = tanpa broadcast (kompatibilitas).
    hub: Option<crate::hub::BroadcastHub>,
    /// Telegram alert (TA5) — None = tanpa notifikasi (zero-config).
    /// Dipanggil setelah insert_event; blocking aman (poller di spawn_blocking).
    telegram: Option<crate::telegram::bridge::AsyncAlertBridge>,
    /// Turbo mode (TM4) — None = sampling statis (zero-config).
    turbo: Option<crate::turbo::TurboHolder>,
}

impl Poller {
    /// Poller tanpa detector (agent standalone / kompatibilitas).
    /// Produksi: pakai `with_detector` dengan koneksi store kedua.
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
            detector: None,
            detector_store: None,
            hub: None,
            telegram: None,
            turbo: None,
        }
    }

    /// Poller PRODUKSI lengkap: detector + hub + telegram + turbo.
    #[allow(clippy::too_many_arguments)]
    pub fn with_everything(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        detector_store: Store,
        hub: crate::hub::BroadcastHub,
        telegram: Option<crate::telegram::bridge::AsyncAlertBridge>,
        turbo: Option<crate::turbo::TurboHolder>,
    ) -> Self {
        let mut p = Self::with_detector_hub(store, agent_token, interval, cfg, detector_store, Some(hub));
        p.telegram = telegram;
        p.turbo = turbo;
        p
    }

    /// Poller dengan detector aktif (SD6) — tanpa broadcast.
    pub fn with_detector(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        detector_store: Store,
    ) -> Self {
        Self::with_detector_hub(store, agent_token, interval, cfg, detector_store, None)
    }

    /// Poller + detector + turbo (TM4).
    pub fn with_turbo(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        detector_store: Store,
        turbo: Option<crate::turbo::TurboHolder>,
    ) -> Self {
        let mut p =
            Self::with_detector_hub(store, agent_token, interval, cfg, detector_store, None);
        if let Some(t) = &mut p.turbo {
            t.set_normal_interval(interval.as_millis() as u64);
        }
        p.turbo = turbo;
        p
    }

    /// Poller + detector + Telegram bridge (TA5).
    pub fn with_detector_and_bridge(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        detector_store: Store,
        telegram: Option<crate::telegram::bridge::AsyncAlertBridge>,
    ) -> Self {
        let mut p =
            Self::with_detector_hub(store, agent_token, interval, cfg, detector_store, None);
        p.telegram = telegram;
        p
    }

    /// Poller lengkap: detector + broadcast hub (WS2).
    pub fn with_hub(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        detector_store: Store,
        hub: crate::hub::BroadcastHub,
    ) -> Self {
        Self::with_detector_hub(store, agent_token, interval, cfg, detector_store, Some(hub))
    }

    /// Konstruktor internal bersama.
    pub fn with_detector_hub(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        detector_store: Store,
        hub: Option<crate::hub::BroadcastHub>,
    ) -> Self {
        let baseline = crate::poller::StoreBaselineRef {
            store: &detector_store,
            window_min: cfg.baseline_window_min,
        };
        let detector = crate::detector::Detector::new(PollerClock, cfg);
        let _ = baseline; // baseline dibuat per-evaluasi di evaluate (borrow)
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
            detector: Some(detector),
            detector_store: Some(detector_store),
            hub,
            telegram: None,
            turbo: None,
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

    /// Poll N putaran dalam spawn_blocking — dipakai test & alat ukur.
    /// Blocking HTTP (Telegram) wajib context blocking (ADR TA-2).
    pub async fn poll_blocking_times(mut self, n: usize) -> Result<(), String> {
        tokio::task::spawn_blocking(move || {
            crate::telegram::client::init_shared_http();
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                for _ in 0..n {
                    self.poll_once_all().await?;
                }
                Ok::<(), String>(())
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Loop produksi: poll tiap interval selamanya.
    /// Poller berisi Store (rusqlite, !Send) → seluruh struct dipindah ke
    /// thread blocking khusus (bukan tokio::spawn).
    pub async fn run_forever(mut self) {
        let interval = self.interval;
        tokio::task::spawn_blocking(move || {
            // TA: reqwest::blocking client harus dibuat di thread blocking
            crate::telegram::client::init_shared_http();
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let mut ticker = tokio::time::interval(interval);
                loop {
                    ticker.tick().await;
                    if let Err(e) = self.poll_once_all().await {
                        tracing::error!("poll loop error: {e}");
                    }
                }
            });
        });
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
            Ok(resp) => {
                let resp = match resp.error_for_status() {
                    Ok(r) => r,
                    Err(e) => {
                        self.mark_failure(host_id, &format!("http: {e}"));
                        return;
                    }
                };
                match resp.json::<Snapshot>().await {
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
                }
            }
            Err(e) => {
                self.mark_failure(host_id, &format!("connect: {e}"));
            }
        }
    }

    /// Simpan snapshot terbaru; bila gap terdeteksi (ts - last_seen > threshold),
    /// tarik backlog agent sejak last_seen dan insert (Q2: backfill v1).
    async fn store_snapshot_and_backfill(
        &mut self,
        host_id: &str,
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

        // WS2: broadcast snapshot ke semua klien dashboard
        if let Some(hub) = &self.hub {
            hub.send(crate::hub::BroadcastMessage::Snapshot(snap.clone()));
        }

        // SD6: evaluasi spike; store & baseline diberikan per-evaluasi
        if let (Some(detector), Some(detector_store)) =
            (self.detector.as_mut(), self.detector_store.as_ref())
        {
            let baseline = StoreBaselineRef {
                store: detector_store,
                window_min: detector.mem_window_min(),
            };
            let evs = detector.evaluate(&snap, &mut self.store, &baseline);
            for ev in evs {
                self.store
                    .insert_event(&ev.host_id, &ev.kind, &ev.severity, &ev.subject, &ev.detail)
                    .map_err(|e| e.to_string())?;
                // WS2: broadcast event baru
                if let Some(hub) = &self.hub {
                    hub.send(crate::hub::BroadcastMessage::Event {
                        host_id: ev.host_id.clone(),
                        kind: ev.kind.clone(),
                        severity: ev.severity.clone(),
                        subject: ev.subject.clone(),
                        detail: ev.detail.clone(),
                    });
                }
                // TA5: Telegram alert (best-effort — hasil diabaikan, TA-3)
                if let Some(bridge) = &mut self.telegram {
                    let sev = match ev.severity.as_str() {
                        "critical" => crate::telegram::Severity::Critical,
                        "info" => crate::telegram::Severity::Info,
                        _ => crate::telegram::Severity::Warning,
                    };
                    let time_str = {
                        let secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let h = (secs / 3600) % 24;
                        let m = (secs / 60) % 60;
                        let s2 = secs % 60;
                        format!("{h:02}:{m:02}:{s2:02}")
                    };
                    let text = crate::telegram::format::render_alert(
                        sev,
                        &ev.kind,
                        &ev.host_id,
                        &ev.subject,
                        &ev.detail,
                        &time_str,
                    );
                    bridge
                        .handle_async(sev, &ev.kind, &ev.host_id, &ev.subject, &text)
                        .await;
                }
                // TM4: turbo push (spike_cpu/mem → interval turbo)
                if let Some(turbo) = &mut self.turbo {
                    turbo.on_event(&ev.kind, &ev.host_id).await;
                }
            }
        }
        // TM4: snapshot tanpa event → sinyal normal utk turbo pop
        if let Some(turbo) = &mut self.turbo {
            turbo.on_snapshot(host_id).await;
        }

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
        let was_offline = {
            let st = self.statuses.entry(host_id.to_string()).or_default();
            let was_offline = !st.online && st.consecutive_failures > 0;
            st.online = true;
            st.consecutive_failures = 0;
            st.last_ok_ms = Some(ts);
            st.last_error = None;
            was_offline
        };
        self.last_seen.insert(host_id.to_string(), ts);

        // Q-SD1: transisi down→up → event agent_up (info) — transparan
        if was_offline {
            let _ = self.store.insert_event(
                host_id,
                "agent_up",
                "info",
                host_id,
                &serde_json::json!({
                    "timestamp_ms": ts,
                }),
            );
        }
        // WS2: broadcast transisi status
        if let Some(hub) = &self.hub {
            hub.send(crate::hub::BroadcastMessage::HostStatus {
                host_id: host_id.to_string(),
                online: true,
            });
        }
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
        // WS2: broadcast transisi status
        if let Some(hub) = &self.hub {
            hub.send(crate::hub::BroadcastMessage::HostStatus {
                host_id: host_id.to_string(),
                online: false,
            });
        }
    }
}

/// Clock produksi untuk detector (waktu nyata).
pub struct PollerClock;

impl crate::detector::DetectorClock for PollerClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// BaselineFetcher yang MEMINJAM Store (parameter evaluasi, bukan dimiliki).
pub struct StoreBaselineRef<'a> {
    pub store: &'a Store,
    pub window_min: u32,
}

impl crate::detector::BaselineFetcher for StoreBaselineRef<'_> {
    fn avg_rss(&self, host_id: &str, pid: i32) -> Option<(u64, u32)> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let from = now.saturating_sub(self.window_min as u64 * 60_000);
        self.store
            .avg_rss_baseline(host_id, pid, from, now)
            .ok()
            .flatten()
    }
}
