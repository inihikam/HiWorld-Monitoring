//! TDD: memory spike via baseline (Task SD4, SD-AC-010..012).
//! Kasus inti: redis-besar-tetap-aman vs memory-leak-terdeteksi.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use hiworld_collector::detector::{
    ActiveEvent, ActiveEventStore, BaselineFetcher, Detector, DetectorClock,
};
use hiworld_core::models::{ProcessInfo, Snapshot, SystemMetrics};

// ---------- fakes (pola sama dengan detector_test) ----------

#[derive(Default, Clone)]
struct FakeStore {
    events: Rc<RefCell<HashMap<String, (u64, u64)>>>,
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
    }
    fn gc(&self, _older_than_ms: u64) {}
}

#[derive(Default)]
struct FakeClock(u64);

impl DetectorClock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

/// Baseline fetcher palsu: (host, pid) -> (avg_rss, sample_count)
#[derive(Clone, Default)]
struct FakeBaseline {
    data: Rc<RefCell<HashMap<(String, i32), (u64, u32)>>>,
}

impl BaselineFetcher for FakeBaseline {
    fn avg_rss(&self, host_id: &str, pid: i32) -> Option<(u64, u32)> {
        self.data.borrow().get(&(host_id.to_string(), pid)).copied()
    }
}

// ---------- snapshot builder ----------

fn proc(pid: i32, comm: &str, rss: u64) -> ProcessInfo {
    ProcessInfo {
        pid,
        comm: comm.into(),
        cmdline: comm.into(),
        user: "redis".into(),
        systemd_unit: Some(format!("{comm}.service")),
        cpu_percent: Some(1.0),
        mem_rss_bytes: rss,
        mem_pss_bytes: None,
    }
}

fn snapshot(ts: u64, procs: Vec<ProcessInfo>) -> Snapshot {
    Snapshot {
        host_id: "db-01".into(),
        timestamp_ms: ts,
        interval_ms: 10_000,
        system: SystemMetrics {
            cpu_percent: Some(5.0),
            cpu_per_core: vec![],
            load_avg: [0.0; 3],
            mem_total_bytes: 8_000_000_000,
            mem_used_bytes: 4_000_000_000,
            mem_percent: 50.0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            disks: vec![],
            net: vec![],
        },
        processes: procs,
    }
}

// ---------- SD-AC-010: kenaikan vs baseline terdeteksi ----------

#[test]
fn mem_leak_detected_vs_baseline() {
    let baseline = FakeBaseline::default();
    // nginx stabil 100MB dengan 10 sample dalam window
    baseline
        .data
        .borrow_mut()
        .insert(("db-01".into(), 1234), (100_000_000, 10));

    let mut d = Detector::new(FakeClock::default(), Default::default());

    let mut store = FakeStore::default();

    // sekarang naik ke 130MB = +30% ≥ threshold 10% → event spike_mem
    let events = d.evaluate(
        &snapshot(1_000, vec![proc(1234, "nginx", 130_000_000)]),
        &mut store,
        &baseline,
    );
    assert_eq!(events.len(), 1);
    let e = &events[0];
    assert_eq!(e.kind, "spike_mem");
    assert_eq!(
        e.severity, "critical",
        "+30% >= 2×threshold (20%) → critical (SD-4)"
    );
    assert_eq!(e.detail["baseline_rss_bytes"], 100_000_000);
    assert_eq!(e.detail["current_rss_bytes"], 130_000_000);
    assert_eq!(e.detail["growth_percent"], 30.0);
    assert_eq!(e.detail["threshold_percent"], 10.0);
}

#[test]
fn mem_growth_20_percent_is_critical() {
    let baseline = FakeBaseline::default();
    baseline
        .data
        .borrow_mut()
        .insert(("db-01".into(), 1234), (100_000_000, 10));

    let mut d = Detector::new(FakeClock::default(), Default::default());
    let mut store = FakeStore::default();

    // +25% >= 2×10% = 20% → critical (SD-4 dua tingkat)
    let events = d.evaluate(
        &snapshot(1_000, vec![proc(1234, "nginx", 125_000_000)]),
        &mut store,
        &baseline,
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].severity, "critical",
        "kenaikan 25% >= 2×threshold (20%) → critical"
    );
}

// ---------- SD-AC-011: redis besar tetap aman ----------

#[test]
fn big_but_stable_process_no_event() {
    let baseline = FakeBaseline::default();
    // redis 500MB stabil (baseline = 500MB, 20 sample)
    baseline
        .data
        .borrow_mut()
        .insert(("db-01".into(), 999), (500_000_000, 20));

    let mut d = Detector::new(FakeClock::default(), Default::default());
    let mut store = FakeStore::default();

    // sekarang 502MB = +0.4% → di bawah threshold
    let events = d.evaluate(
        &snapshot(1_000, vec![proc(999, "redis", 502_000_000)]),
        &mut store,
        &baseline,
    );
    assert!(
        events.is_empty(),
        "proses memang besar & stabil → BUKAN spike; dapat {events:?}"
    );
}

// ---------- SD-AC-012: baseline butuh min samples ----------

#[test]
fn too_few_samples_no_baseline_no_event() {
    let baseline = FakeBaseline::default();
    // proses baru: hanya 2 sample (< min 3) → baseline TIDAK valid
    baseline
        .data
        .borrow_mut()
        .insert(("db-01".into(), 777), (10_000_000, 2));

    let mut d = Detector::new(FakeClock::default(), Default::default());
    let mut store = FakeStore::default();

    // RSS naik besar → tetap TIDAK ada event karena baseline belum valid
    let events = d.evaluate(
        &snapshot(1_000, vec![proc(777, "newapp", 50_000_000)]),
        &mut store,
        &baseline,
    );
    assert!(
        events.is_empty(),
        "proses tanpa baseline valid → skip (false positive guard)"
    );
}

#[test]
fn no_baseline_entry_at_all_skipped() {
    let baseline = FakeBaseline::default();
    let mut d = Detector::new(FakeClock::default(), Default::default());
    let mut store = FakeStore::default();
    let events = d.evaluate(
        &snapshot(1_000, vec![proc(555, "unknown-proc", 900_000_000)]),
        &mut store,
        &baseline,
    );
    assert!(events.is_empty(), "proses tak dikenal → skip");
}

// ---------- dedup mem spike (sama pola CPU) ----------

#[test]
fn mem_spike_dedup_cycle() {
    let baseline = FakeBaseline::default();
    baseline
        .data
        .borrow_mut()
        .insert(("db-01".into(), 1234), (100_000_000, 10));

    let mut d = Detector::new(FakeClock::default(), Default::default());

    let mut store = FakeStore::default();

    // 1: leak mulai → event
    let e1 = d.evaluate(
        &snapshot(1_000, vec![proc(1234, "nginx", 130_000_000)]),
        &mut store,
        &baseline,
    );
    assert_eq!(e1.len(), 1);

    // 2: masih leak → dedup
    let e2 = d.evaluate(
        &snapshot(11_000, vec![proc(1234, "nginx", 140_000_000)]),
        &mut store,
        &baseline,
    );
    assert!(e2.is_empty(), "spike mem masih aktif → tidak spam");

    // 3: kembali normal (restart proses) → state cleared
    let e3 = d.evaluate(
        &snapshot(21_000, vec![proc(1234, "nginx", 100_000_000)]),
        &mut store,
        &baseline,
    );
    assert!(e3.is_empty());

    // 4: leak lagi → event baru
    let e4 = d.evaluate(
        &snapshot(31_000, vec![proc(1234, "nginx", 135_000_000)]),
        &mut store,
        &baseline,
    );
    assert_eq!(e4.len(), 1);
}

// ---------- SD-AC-010 lanjutan: memory spike by absolute growth fallback ----------

#[test]
fn mem_spike_threshold_respects_config() {
    // threshold custom 25%: kenaikan 30% → tetap event; kenaikan 15% → tidak
    let baseline = FakeBaseline::default();
    baseline
        .data
        .borrow_mut()
        .insert(("db-01".into(), 1234), (100_000_000, 10));

    let cfg = hiworld_collector::api::DetectorConfig {
        mem_spike_threshold_percent: 25.0,
        ..Default::default()
    };
    let mut d = Detector::new(FakeClock::default(), cfg);

    let mut store = FakeStore::default();

    // +15% → di bawah 25% → tidak ada event
    let e1 = d.evaluate(
        &snapshot(1_000, vec![proc(1234, "nginx", 115_000_000)]),
        &mut store,
        &baseline,
    );
    assert!(e1.is_empty(), "di bawah threshold custom");

    // +30% → di atas 25% → event
    let e2 = d.evaluate(
        &snapshot(2_000, vec![proc(1234, "nginx", 130_000_000)]),
        &mut store,
        &baseline,
    );
    assert_eq!(e2.len(), 1, "di atas threshold custom");
}
