//! TDD: SQLite store untuk detector (Task SD5, SD-AC-013 + Opsi B).
//! 1. avg_rss_baseline: query processes_json dalam window
//! 2. ActiveEventStore impl: dedup lintas "restart" (buka store baru)

use std::cell::RefCell;

use hiworld_collector::detector::ActiveEventStore;
use hiworld_collector::store::Store;
use hiworld_core::models::{ProcessInfo, Snapshot, SystemMetrics};

fn fixture_snapshot(host: &str, ts_ms: u64, procs: Vec<ProcessInfo>) -> Snapshot {
    Snapshot {
        host_id: host.into(),
        timestamp_ms: ts_ms,
        interval_ms: 10_000,
        system: SystemMetrics {
            cpu_percent: Some(5.0),
            cpu_per_core: vec![],
            load_avg: [0.0; 3],
            mem_total_bytes: 1,
            mem_used_bytes: 0,
            mem_percent: 0.0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            disks: vec![],
            net: vec![],
        },
        processes: procs,
    }
}

fn proc(pid: i32, rss_mb: u64) -> ProcessInfo {
    ProcessInfo {
        pid,
        comm: "app".into(),
        cmdline: "app".into(),
        user: "root".into(),
        systemd_unit: None,
        cpu_percent: Some(1.0),
        mem_rss_bytes: rss_mb * 1_000_000,
        mem_pss_bytes: None,
    }
}

// ---------- SD-AC-013: baseline dari store ----------

#[test]
fn baseline_avg_over_window() {
    let store = Store::open_in_memory().unwrap();
    // nginx pid 1234: RSS 100, 110, 90 MB dalam window → avg 100MB
    for (ts, mb) in [(1_000, 100), (11_000, 110), (21_000, 90)] {
        store
            .insert_snapshot(&fixture_snapshot("web-01", ts, vec![proc(1234, mb)]))
            .unwrap();
    }

    let (avg, count) = store
        .avg_rss_baseline("web-01", 1234, 0, 1_000_000)
        .unwrap()
        .unwrap();
    assert!(avg.abs_diff(100_000_000) < 1, "avg dapat {avg}");
    assert_eq!(count, 3);
}

#[test]
fn baseline_excludes_outside_window() {
    let store = Store::open_in_memory().unwrap();
    // dalam window (1_000..1_000_000): 100 & 110; di luar: 500 (terlalu lama)
    store
        .insert_snapshot(&fixture_snapshot("web-01", 500, vec![proc(1234, 500)]))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_000, vec![proc(1234, 100)]))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 2_000, vec![proc(1234, 110)]))
        .unwrap();

    let (avg, count) = store
        .avg_rss_baseline("web-01", 1234, 1_000, 1_000_000)
        .unwrap()
        .unwrap();
    assert!(
        avg.abs_diff(105_000_000) < 1,
        "avg hanya 2 sample dalam window: {avg}"
    );
    assert_eq!(count, 2);
}

#[test]
fn baseline_none_when_process_unknown() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_000, vec![proc(1234, 100)]))
        .unwrap();
    let r = store
        .avg_rss_baseline("web-01", 9999, 0, 1_000_000)
        .unwrap();
    assert!(r.is_none(), "pid tak dikenal → None");
}

#[test]
fn baseline_zero_rss_samples_excluded_to_avoid_div_zero() {
    // sample dengan rss 0 (anomali parse) tidak boleh merusak avg
    let store = Store::open_in_memory().unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_000, vec![proc(1234, 0)]))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 2_000, vec![proc(1234, 100)]))
        .unwrap();

    let (avg, count) = store
        .avg_rss_baseline("web-01", 1234, 0, 1_000_000)
        .unwrap()
        .unwrap();
    assert!(avg.abs_diff(100_000_000) < 1, "rss 0 dikecualikan: {avg}");
    assert_eq!(count, 1);
}

// ---------- Opsi B: ActiveEventStore persisten ----------

#[test]
fn active_event_persists_and_dedups_across_restart() {
    let path = std::env::temp_dir().join(format!("hiworld-sd5-{}.db", std::process::id()));

    // "instance pertama": register spike
    {
        let store = Store::open(&path).unwrap();
        store
            .insert_active_event("spike_cpu:web-01:1234", 1_000)
            .unwrap();
    }

    // "instance kedua" (restart): key masih ada → dedup bekerja
    {
        let store = Store::open(&path).unwrap();
        assert!(
            store
                .get_active_event("spike_cpu:web-01:1234")
                .unwrap()
                .is_some(),
            "Opsi B: state aktif selamat dari restart"
        );

        // touch lalu cek last_seen berubah
        store
            .touch_active_event("spike_cpu:web-01:1234", 2_000)
            .unwrap();
        let ev = store
            .get_active_event("spike_cpu:web-01:1234")
            .unwrap()
            .unwrap();
        assert_eq!(ev.first_seen_ms, 1_000, "first_seen tidak berubah");
        assert_eq!(ev.last_seen_ms, 2_000);

        // delete saat pulih
        store.delete_active_event("spike_cpu:web-01:1234").unwrap();
        assert!(store
            .get_active_event("spike_cpu:web-01:1234")
            .unwrap()
            .is_none());
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn active_event_gc_removes_stale() {
    let store = Store::open_in_memory().unwrap();
    store.insert_active_event("spike_cpu:old:1", 1_000).unwrap();
    store
        .insert_active_event("spike_cpu:new:2", 100_000)
        .unwrap();

    // GC: buang yang last_seen < 50_000
    store.gc_active_events(50_000).unwrap();

    assert!(
        store.get_active_event("spike_cpu:old:1").unwrap().is_none(),
        "stale dibuang"
    );
    assert!(
        store.get_active_event("spike_cpu:new:2").unwrap().is_some(),
        "baru tetap ada"
    );
}

// ---------- detector memakai store produksi (wiring check) ----------

#[test]
fn detector_with_sqlite_store_persists() {
    let mut store = Store::open_in_memory().unwrap();
    let mut d = hiworld_collector::detector::Detector::new(TestClock(1_000), Default::default());

    // spike → event + state tersimpan di SQLite asli
    let mut snap = fixture_snapshot("web-01", 2_000, vec![proc(1234, 100)]);
    snap.processes[0].cpu_percent = Some(95.0);
    let events = d.evaluate(&snap, &mut store, &NoBaseline);
    assert_eq!(events.len(), 1, "cpu spike");

    assert!(
        store
            .get_active_event("spike_cpu:web-01:1234")
            .unwrap()
            .is_some(),
        "state tersimpan di SQLite produksi"
    );

    // evaluasi ulang → dedup (tidak ada event baru)
    snap.timestamp_ms = 3_000;
    let events = d.evaluate(&snap, &mut store, &NoBaseline);
    assert!(events.is_empty(), "dedup via SQLite");
}

struct TestClock(u64);
impl hiworld_collector::detector::DetectorClock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

struct NoBaseline;

impl hiworld_collector::detector::BaselineFetcher for NoBaseline {
    fn avg_rss(&self, _host: &str, _pid: i32) -> Option<(u64, u32)> {
        None
    }
}

// pemakaian RefCell supaya import tidak unused (clone_for_detector butuh Rc di lib)
#[allow(dead_code)]
fn _witness() {
    let _ = RefCell::new(0);
}
