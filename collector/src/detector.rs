//! Spike detector (Task SD2) — MURNI, semua dependensi trait-injectable
//! (docs/specs/spike-detection.md §3, ADR SD-1/SD-2/SD-7).
//!
//! Evaluate satu snapshot → Vec<EventCandidate>. Dedup via ActiveEventStore
//! (persisten — Opsi B): spike aktif tidak diterbitkan ulang; kembali normal
//! lalu spike lagi = event baru. GC: key stale > 24 jam dibuang.

use std::collections::HashMap;

use crate::api::DetectorConfig;
use hiworld_core::models::Snapshot;

// ---------- trait dependensi ----------

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveEvent {
    pub key: String,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
}

/// State dedup persisten (Opsi B — impl produksi: SQLite, SD5).
pub trait ActiveEventStore {
    fn get(&self, key: &str) -> Option<ActiveEvent>;
    fn insert(&self, key: &str, now_ms: u64);
    fn touch(&self, key: &str, now_ms: u64);
    fn delete(&self, key: &str);
    /// Buang key dengan last_seen_ms < older_than_ms (SD-7).
    fn gc(&self, older_than_ms: u64);
}

pub trait DetectorClock {
    fn now_ms(&self) -> u64;
}

/// Baseline memori: avg RSS per (host, pid) dalam window + jumlah sample.
/// None = baseline tidak valid (proses terlalu baru — min samples, SD-AC-012).
pub trait BaselineFetcher {
    fn avg_rss(&self, host_id: &str, pid: i32) -> Option<(u64, u32)>;
}

// ---------- event candidate ----------

#[derive(Debug, Clone, PartialEq)]
pub struct EventCandidate {
    pub host_id: String,
    pub kind: String,
    pub severity: String,
    pub subject: String,
    pub detail: serde_json::Value,
}

// ---------- detector ----------

const GC_AFTER_MS: u64 = 24 * 3_600_000; // 24 jam (SD-7)
const CRITICAL_CPU_PERCENT: f64 = 95.0;

pub struct Detector<S: ActiveEventStore, C: DetectorClock, B: BaselineFetcher> {
    store: S,
    clock: C,
    cfg: DetectorConfig,
    baseline: B,
}

impl<S: ActiveEventStore, C: DetectorClock, B: BaselineFetcher> Detector<S, C, B> {
    pub fn new(store: S, clock: C, cfg: DetectorConfig, baseline: B) -> Self {
        Self {
            store,
            clock,
            cfg,
            baseline,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Akses clock (hanya untuk test — introspeksi & manipulasi waktu).
    pub fn clock_mut(&mut self) -> &mut C {
        &mut self.clock
    }

    /// Evaluasi satu snapshot. Idempotent terhadap dedup: spike aktif
    /// tidak menghasilkan event kedua.
    pub fn evaluate(&mut self, snap: &Snapshot) -> Vec<EventCandidate> {
        let now = self.clock.now_ms();
        let mut events = Vec::new();

        // GC stale (SD-7) — murah, jalan tiap evaluasi
        self.store.gc(now.saturating_sub(GC_AFTER_MS));

        self.detect_cpu_spikes(snap, now, &mut events);
        self.detect_disk_full(snap, now, &mut events);
        self.detect_mem_spikes(snap, now, &mut events);

        events
    }

    fn detect_cpu_spikes(&mut self, snap: &Snapshot, now: u64, events: &mut Vec<EventCandidate>) {
        for p in &snap.processes {
            let Some(cpu) = p.cpu_percent else {
                continue; // sample pertama / tanpa delta (SD-5)
            };
            if cpu < self.cfg.cpu_spike_threshold_percent {
                // normal → bersihkan state aktif bila ada (siklus Opsi B langkah 3)
                self.store.delete(&self.cpu_key(snap, p.pid));
                continue;
            }

            let key = self.cpu_key(snap, p.pid);
            match self.store.get(&key) {
                Some(active) => {
                    // masih spike → cukup touch last_seen (langkah 2)
                    self.store.touch(&key, now);
                    let _ = active;
                }
                None => {
                    // spike baru → INSERT + terbitkan event (langkah 1)
                    self.store.insert(&key, now);
                    let severity = if cpu >= CRITICAL_CPU_PERCENT {
                        "critical"
                    } else {
                        "warning"
                    };
                    events.push(EventCandidate {
                        host_id: snap.host_id.clone(),
                        kind: "spike_cpu".into(),
                        severity: severity.into(),
                        subject: format!("{} (pid {}, {})", p.comm, p.pid, unit_label(p)),
                        detail: serde_json::json!({
                            "cpu_percent": cpu,
                            "mem_rss_bytes": p.mem_rss_bytes,
                            "systemd_unit": p.systemd_unit,
                            "user": p.user,
                            "threshold": self.cfg.cpu_spike_threshold_percent,
                        }),
                    });
                }
            }
        }
    }

    fn cpu_key(&self, snap: &Snapshot, pid: i32) -> String {
        format!("spike_cpu:{}:{}", snap.host_id, pid)
    }

    /// Memory spike: kenaikan RSS vs baseline window (menangkap LEAK,
    /// bukan proses yang memang besar — SD-AC-011).
    fn detect_mem_spikes(&mut self, snap: &Snapshot, now: u64, events: &mut Vec<EventCandidate>) {
        for p in &snap.processes {
            let key = format!("spike_mem:{}:{}", snap.host_id, p.pid);
            // baseline valid?
            let Some((base_rss, sample_count)) = self.baseline.avg_rss(&snap.host_id, p.pid) else {
                continue; // SD-AC-012: tanpa baseline valid → skip
            };
            if sample_count < self.cfg.min_samples_for_baseline {
                continue; // proses terlalu baru → baseline tidak valid
            }
            if base_rss == 0 {
                continue; // hindari div by zero
            }
            let growth_percent =
                (p.mem_rss_bytes.saturating_sub(base_rss)) as f64 / base_rss as f64 * 100.0;
            if growth_percent < self.cfg.mem_spike_threshold_percent {
                self.store.delete(&key); // kembali normal → clear state
                continue;
            }
            match self.store.get(&key) {
                Some(_) => self.store.touch(&key, now),
                None => {
                    self.store.insert(&key, now);
                    // severity: kenaikan >= 2× threshold → critical (SD-4)
                    let severity = if growth_percent >= self.cfg.mem_spike_threshold_percent * 2.0 {
                        "critical"
                    } else {
                        "warning"
                    };
                    events.push(EventCandidate {
                        host_id: snap.host_id.clone(),
                        kind: "spike_mem".into(),
                        severity: severity.into(),
                        subject: format!("{} (pid {}, {})", p.comm, p.pid, unit_label(p)),
                        detail: serde_json::json!({
                            "baseline_rss_bytes": base_rss,
                            "current_rss_bytes": p.mem_rss_bytes,
                            "growth_percent": growth_percent,
                            "threshold_percent": self.cfg.mem_spike_threshold_percent,
                            "systemd_unit": p.systemd_unit,
                            "user": p.user,
                        }),
                    });
                }
            }
        }
    }

    fn detect_disk_full(&mut self, snap: &Snapshot, now: u64, events: &mut Vec<EventCandidate>) {
        for d in &snap.system.disks {
            let key = format!("disk_full:{}:{}", snap.host_id, d.mount);
            if d.percent < self.cfg.disk_full_threshold_percent {
                self.store.delete(&key);
                continue;
            }
            match self.store.get(&key) {
                Some(_) => self.store.touch(&key, now),
                None => {
                    self.store.insert(&key, now);
                    let severity = if d.percent >= 95.0 {
                        "critical"
                    } else {
                        "warning"
                    };
                    events.push(EventCandidate {
                        host_id: snap.host_id.clone(),
                        kind: "disk_almost_full".into(),
                        severity: severity.into(),
                        subject: format!("{} ({})", d.mount, d.device),
                        detail: serde_json::json!({
                            "percent": d.percent,
                            "used_bytes": d.used_bytes,
                            "total_bytes": d.total_bytes,
                            "threshold": self.cfg.disk_full_threshold_percent,
                        }),
                    });
                }
            }
        }
    }
}

fn unit_label(p: &hiworld_core::models::ProcessInfo) -> String {
    p.systemd_unit
        .clone()
        .unwrap_or_else(|| "non-systemd".into())
}

/// Map sederhana untuk GC bila implementasi store butuh iterasi lokal
/// (dipakai di test fake store; produksi pakai SQL).
#[allow(dead_code)]
fn collect_stale(events: &HashMap<String, (u64, u64)>, older_than_ms: u64) -> Vec<String> {
    events
        .iter()
        .filter(|(_, (_, last))| *last < older_than_ms)
        .map(|(k, _)| k.clone())
        .collect()
}
