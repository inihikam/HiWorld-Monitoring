//! TDD: Detector murni — CPU spike + active_events (Task SD2).
//! Semua dependensi (store & clock) di-inject via trait — tanpa DB/jam nyata.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use hiworld_collector::detector::{
    ActiveEvent, ActiveEventStore, BaselineFetcher, Detector, DetectorClock,
};

// ---------- fake store ----------

#[derive(Default, Clone)]
struct FakeStore {
    /// key -> (first_seen, last_seen)
    events: Rc<RefCell<std::collections::HashMap<String, (u64, u64)>>>,
    deleted: Rc<RefCell<Vec<String>>>,
}

impl ActiveEventStore for FakeStore {
    fn get(&self, key: &str) -> Option<ActiveEvent> {
        self.events.borrow().get(key).map(|(f, l)| ActiveEvent {
            key: key.to_string(),
            first_seen_ms: *f,
            last_seen_ms: *l,
        })
    }
    fn insert(&self, key: &str, now_ms: u64) {
        self.events
            .borrow_mut()
            .insert(key.to_string(), (now_ms, now_ms));
    }
    fn touch(&self, key: &str, now_ms: u64) {
        if let Some(e) = self.events.borrow_mut().get_mut(key) {
            e.1 = now_ms;
        }
    }
    fn delete(&self, key: &str) {
        self.events.borrow_mut().remove(key);
        self.deleted.borrow_mut().push(key.to_string());
    }
    fn gc(&self, older_than_ms: u64) {
        let stale: Vec<String> = self
            .events
            .borrow()
            .iter()
            .filter(|(_, (_, last))| *last < older_than_ms)
            .map(|(k, _)| k.clone())
            .collect();
        for k in stale {
            self.delete(&k);
        }
    }
}

#[derive(Default)]
struct FakeClock(u64);

impl DetectorClock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

// ---------- snapshot builder ----------

use hiworld_core::models::{ProcessInfo, Snapshot, SystemMetrics};

fn proc(pid: i32, comm: &str, cpu: Option<f64>, rss: u64) -> ProcessInfo {
    ProcessInfo {
        pid,
        comm: comm.into(),
        cmdline: comm.into(),
        user: "www-data".into(),
        systemd_unit: Some(format!("{comm}.service")),
        cpu_percent: cpu,
        mem_rss_bytes: rss,
        mem_pss_bytes: None,
    }
}

fn snapshot(ts: u64, procs: Vec<ProcessInfo>) -> Snapshot {
    Snapshot {
        host_id: "web-01".into(),
        timestamp_ms: ts,
        interval_ms: 10_000,
        system: SystemMetrics {
            cpu_percent: Some(50.0),
            cpu_per_core: vec![],
            load_avg: [0.0; 3],
            mem_total_bytes: 16_000_000_000,
            mem_used_bytes: 8_000_000_000,
            mem_percent: 50.0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            disks: vec![], // disk di SD3
            net: vec![],
        },
        processes: procs,
    }
}

fn detector() -> Detector<FakeStore, FakeClock, NoBaseline> {
    Detector::new(
        FakeStore::default(),
        FakeClock::default(),
        hiworld_collector::api::DetectorConfig::default(),
        NoBaseline,
    )
}

/// Baseline kosong: semua proses dianggap tanpa baseline (CPU/disk test fokus).
struct NoBaseline;

impl BaselineFetcher for NoBaseline {
    fn avg_rss(&self, _host_id: &str, _pid: i32) -> Option<(u64, u32)> {
        None
    }
}

// ---------- SR-AC-001: spike terdeteksi dengan detail lengkap ----------

#[test]
fn cpu_spike_emits_event_with_full_detail() {
    let mut d = detector();
    let snap = snapshot(1_000, vec![proc(1234, "nginx", Some(91.2), 52_428_800)]);
    let events = d.evaluate(&snap);

    assert_eq!(events.len(), 1, "satu spike → satu event");
    let e = &events[0];
    assert_eq!(e.kind, "spike_cpu");
    assert_eq!(e.severity, "warning");
    assert_eq!(e.host_id, "web-01");
    // subject readable
    assert!(
        e.subject.contains("nginx") && e.subject.contains("1234"),
        "subject harus manusiawi: {}",
        e.subject
    );
    // detail lengkap
    let detail = &e.detail;
    assert_eq!(detail["cpu_percent"], 91.2);
    assert_eq!(detail["mem_rss_bytes"], 52_428_800);
    assert_eq!(detail["systemd_unit"], "nginx.service");
    assert_eq!(detail["user"], "www-data");
    assert_eq!(detail["threshold"], 85.0);
}

// ---------- SD-AC-002: critical threshold ----------

#[test]
fn cpu_96_percent_is_critical() {
    let mut d = detector();
    let snap = snapshot(1_000, vec![proc(1, "stress", Some(96.0), 1_000)]);
    let events = d.evaluate(&snap);
    assert_eq!(events[0].severity, "critical", ">= 95% → critical");
}

#[test]
fn cpu_94_percent_is_warning() {
    let mut d = detector();
    let snap = snapshot(1_000, vec![proc(1, "stress", Some(94.0), 1_000)]);
    let events = d.evaluate(&snap);
    assert_eq!(events[0].severity, "warning");
}

// ---------- SD-AC-003: normal → kosong ----------

#[test]
fn normal_cpu_no_event() {
    let mut d = detector();
    let snap = snapshot(1_000, vec![proc(1, "nginx", Some(40.0), 1_000)]);
    assert!(d.evaluate(&snap).is_empty());
}

// ---------- SD-AC-004: dedup 4-snapshot cycle ----------

#[test]
fn dedup_cycle_spike_spike_normal_spike() {
    let mut d = detector();

    // snapshot 1: spike → event
    let e1 = d.evaluate(&snapshot(
        1_000,
        vec![proc(1234, "nginx", Some(90.0), 1_000)],
    ));
    assert_eq!(e1.len(), 1, "spike pertama → event");

    // snapshot 2: masih spike pid sama → TIDAK ada event baru (dedup)
    let e2 = d.evaluate(&snapshot(
        11_000,
        vec![proc(1234, "nginx", Some(92.0), 1_000)],
    ));
    assert!(e2.is_empty(), "masih aktif → tidak spam");

    // snapshot 3: normal → tidak ada event, tapi state dibersihkan
    let e3 = d.evaluate(&snapshot(
        21_000,
        vec![proc(1234, "nginx", Some(10.0), 1_000)],
    ));
    assert!(e3.is_empty());

    // snapshot 4: spike lagi → event BARU
    let e4 = d.evaluate(&snapshot(
        31_000,
        vec![proc(1234, "nginx", Some(90.0), 1_000)],
    ));
    assert_eq!(e4.len(), 1, "spike setelah normal → event baru");
}

// ---------- SD-AC-005: dua pid spike → dua event ----------

#[test]
fn two_pids_spiking_two_events() {
    let mut d = detector();
    let snap = snapshot(
        1_000,
        vec![
            proc(1, "nginx", Some(90.0), 1_000),
            proc(2, "build", Some(88.0), 1_000),
        ],
    );
    let events = d.evaluate(&snap);
    assert_eq!(events.len(), 2, "pid beda → event terpisah");
}

// ---------- SD-AC-006: cpu None dilewati ----------

#[test]
fn cpu_none_skipped_no_panic() {
    let mut d = detector();
    let snap = snapshot(1_000, vec![proc(1, "first-sample", None, 1_000)]);
    assert!(d.evaluate(&snap).is_empty(), "None → skip (ADR-1/SD-5)");
}

// ---------- Opsi B: persistence via store ----------

#[test]
fn active_state_persisted_in_store() {
    let mut d = detector();
    let _ = d.evaluate(&snapshot(
        1_000,
        vec![proc(1234, "nginx", Some(90.0), 1_000)],
    ));

    // state tersimpan di store (bukan cuma in-memory struct)
    let store = d.store();
    assert!(
        store.get("spike_cpu:web-01:1234").is_some(),
        "key aktif harus ada di store (Opsi B)"
    );
}

#[derive(Clone, Default)]
struct FakeBaseline {
    data: Rc<RefCell<HashMap<(String, i32), (u64, u32)>>>,
}

impl BaselineFetcher for FakeBaseline {
    fn avg_rss(&self, host_id: &str, pid: i32) -> Option<(u64, u32)> {
        self.data.borrow().get(&(host_id.to_string(), pid)).copied()
    }
}

#[test]
fn persistence_across_restart() {
    // "restart" = detector baru dengan STORE YANG SAMA
    let store = FakeStore::default();
    let cfg = hiworld_collector::api::DetectorConfig::default();

    let baseline = FakeBaseline::default();
    baseline
        .data
        .borrow_mut()
        .insert(("web-01".into(), 1234), (1_000, 10));
    let mut d1 = Detector::new(
        store.clone(),
        FakeClock::default(),
        cfg.clone(),
        baseline.clone(),
    );
    let _ = d1.evaluate(&snapshot(
        1_000,
        vec![proc(1234, "nginx", Some(90.0), 130_000_000)],
    ));

    // detector baru (simulasi restart) — state tetap dari store
    let mut d2 = Detector::new(store.clone(), FakeClock::default(), cfg, baseline);
    let e = d2.evaluate(&snapshot(
        11_000,
        vec![proc(1234, "nginx", Some(92.0), 140_000_000)],
    ));
    assert!(
        e.is_empty(),
        "Opsi B: setelah restart, spike aktif TIDAK diterbitkan ulang"
    );
}

// ---------- GC (SD-7) ----------

#[test]
fn gc_removes_stale_entries() {
    let mut d = detector();
    let _ = d.evaluate(&snapshot(
        1_000,
        vec![proc(1234, "nginx", Some(90.0), 1_000)],
    ));

    // maju waktu > 24 jam tanpa touch → GC harus membersihkan
    d.clock_mut().0 = 1_000 + 25 * 3_600_000;
    let _ = d.evaluate(&snapshot(1_000 + 25 * 3_600_000, vec![]));

    let store = d.store();
    assert!(
        store.get("spike_cpu:web-01:1234").is_none(),
        "stale key (> 24 jam) dibuang oleh GC"
    );
}
