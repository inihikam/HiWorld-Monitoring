//! TDD: disk almost full (Task SD3, SD-AC-020, SD-AC-021).

use std::cell::RefCell;
use std::rc::Rc;

use hiworld_collector::detector::{
    ActiveEvent, ActiveEventStore, BaselineFetcher, Detector, DetectorClock,
};
use hiworld_core::models::{DiskMetrics, Snapshot, SystemMetrics};

// reuse fake store & clock dari detector_test (definisi duplikat ringan
// karena integration test terpisah binary)

#[derive(Default, Clone)]
struct FakeStore {
    events: Rc<RefCell<std::collections::HashMap<String, (u64, u64)>>>,
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

/// Baseline kosong (CPU/disk test tidak menyentuh mem spike).
struct NoBaseline;

impl BaselineFetcher for NoBaseline {
    fn avg_rss(&self, _host_id: &str, _pid: i32) -> Option<(u64, u32)> {
        None
    }
}

#[derive(Default)]
struct FakeClock(u64);

impl DetectorClock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

fn disk(mount: &str, percent: f64) -> DiskMetrics {
    DiskMetrics {
        mount: mount.into(),
        device: format!("/dev/sda1"),
        total_bytes: 100_000_000_000,
        used_bytes: (percent * 1_000_000_000.0) as u64,
        percent,
        read_bytes: 0,
        write_bytes: 0,
    }
}

fn snapshot_with_disks(disks: Vec<DiskMetrics>) -> Snapshot {
    Snapshot {
        host_id: "web-01".into(),
        timestamp_ms: 1_000,
        interval_ms: 10_000,
        system: SystemMetrics {
            cpu_percent: None,
            cpu_per_core: vec![],
            load_avg: [0.0; 3],
            mem_total_bytes: 1,
            mem_used_bytes: 0,
            mem_percent: 0.0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            disks,
            net: vec![],
        },
        processes: vec![],
    }
}

// ---------- SD-AC-020 ----------

#[test]
fn disk_92_percent_warning() {
    let mut d = Detector::new(
        FakeStore::default(),
        FakeClock::default(),
        Default::default(),
        NoBaseline,
    );
    let events = d.evaluate(&snapshot_with_disks(vec![disk("/", 92.0)]));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "disk_almost_full");
    assert_eq!(events[0].severity, "warning");
    assert!(
        events[0].subject.contains("/"),
        "subject menyebut mountpoint"
    );
    assert_eq!(events[0].detail["percent"], 92.0);
}

#[test]
fn disk_96_percent_critical() {
    let mut d = Detector::new(
        FakeStore::default(),
        FakeClock::default(),
        Default::default(),
        NoBaseline,
    );
    let events = d.evaluate(&snapshot_with_disks(vec![disk("/", 96.0)]));
    assert_eq!(events[0].severity, "critical", ">= 95% → critical");
}

#[test]
fn disk_normal_no_event() {
    let mut d = Detector::new(
        FakeStore::default(),
        FakeClock::default(),
        Default::default(),
        NoBaseline,
    );
    let events = d.evaluate(&snapshot_with_disks(vec![disk("/", 50.0)]));
    assert!(events.is_empty());
}

// ---------- SD-AC-021: dedup per mountpoint ----------

#[test]
fn disk_dedup_cycle() {
    let mut d = Detector::new(
        FakeStore::default(),
        FakeClock::default(),
        Default::default(),
        NoBaseline,
    );

    // 1: penuh → event
    let e1 = d.evaluate(&snapshot_with_disks(vec![disk("/", 93.0)]));
    assert_eq!(e1.len(), 1);

    // 2: masih penuh → dedup
    let e2 = d.evaluate(&snapshot_with_disks(vec![disk("/", 94.0)]));
    assert!(e2.is_empty(), "masih aktif → tidak spam");

    // 3: dibersihkan (cleanup berhasil) → tidak ada event, state deleted
    let e3 = d.evaluate(&snapshot_with_disks(vec![disk("/", 40.0)]));
    assert!(e3.is_empty());

    // 4: penuh lagi → event baru
    let e4 = d.evaluate(&snapshot_with_disks(vec![disk("/", 95.0)]));
    assert_eq!(e4.len(), 1, "penuh setelah normal → event baru");
    assert_eq!(e4[0].severity, "critical");
}

#[test]
fn disk_two_mountpoints_independent() {
    let mut d = Detector::new(
        FakeStore::default(),
        FakeClock::default(),
        Default::default(),
        NoBaseline,
    );

    // "/" dan "/home" sama-sama penuh → dua event terpisah
    let e1 = d.evaluate(&snapshot_with_disks(vec![
        disk("/", 91.0),
        disk("/home", 92.0),
    ]));
    assert_eq!(e1.len(), 2, "mountpoint beda → event terpisah");

    // "/" pulih, "/home" masih penuh → tidak ada event baru untuk /home
    let e2 = d.evaluate(&snapshot_with_disks(vec![
        disk("/", 30.0),
        disk("/home", 93.0),
    ]));
    assert!(e2.is_empty(), "dedup per mountpoint");

    // "/home" pulih lalu penuh lagi → event baru hanya /home
    let e3 = d.evaluate(&snapshot_with_disks(vec![disk("/home", 40.0)]));
    assert!(e3.is_empty());
    let e4 = d.evaluate(&snapshot_with_disks(vec![disk("/home", 96.0)]));
    assert_eq!(e4.len(), 1);
    assert!(e4[0].subject.contains("/home"));
}
