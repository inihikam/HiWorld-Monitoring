//! TDD: sampler (Task C1). Sampler menerima ProcSource + Clock abstrak
//! sehingga testable tanpa /proc asli & tanpa tidur nyata.
//!
//! FakeSource memakai Rc<RefCell> agar data bisa diubah ANTARA sampling
//! (source sudah dipindah ke Sampler).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use hiworld_agent::sampler::{Clock, ProcSource, Sampler};
use hiworld_core::proc_parser::{CpuStat, CpuTime, MemInfo, PidStat, ProcError, ProcFs};

#[derive(Clone)]
struct FakeState {
    cpu: CpuTime,
    mem: MemInfo,
    load: [f64; 3],
    procs: Vec<PidStat>,
    units: Vec<Option<String>>,
    users: Vec<String>,
    t: u64,
}

impl FakeState {
    fn new() -> Self {
        Self {
            cpu: CpuTime {
                user: 0,
                nice: 0,
                system: 0,
                idle: 1_000_000,
                iowait: 0,
                irq: 0,
                softirq: 0,
                steal: 0,
            },
            mem: MemInfo {
                mem_total_bytes: 16_000_000_000,
                mem_available_bytes: 8_000_000_000,
                swap_total_bytes: 2_000_000_000,
                swap_free_bytes: 2_000_000_000,
            },
            load: [0.0, 0.0, 0.0],
            procs: vec![],
            units: vec![],
            users: vec![],
            t: 1_000_000,
        }
    }

    /// Geser jiffies: busy naik `busy_delta`, idle naik 3x (≈25% busy).
    fn burn_cpu(&mut self, busy_delta: u64) {
        self.cpu.user += busy_delta;
        self.cpu.idle += busy_delta * 3;
    }

    fn tick(&mut self, ms: u64) {
        self.t += ms;
    }
}

struct FakeSource {
    st: Rc<RefCell<FakeState>>,
}

impl ProcSource for FakeSource {
    fn stat(&self) -> Result<CpuStat, ProcError> {
        let st = self.st.borrow();
        Ok(CpuStat {
            cpu_total: st.cpu.clone(),
            cpus: vec![],
        })
    }
    fn meminfo(&self) -> Result<MemInfo, ProcError> {
        Ok(self.st.borrow().mem.clone())
    }
    fn loadavg(&self) -> Result<[f64; 3], ProcError> {
        Ok(self.st.borrow().load)
    }
    fn uptime(&self) -> Result<f64, ProcError> {
        Ok(self.st.borrow().t as f64 / 1000.0)
    }
    fn now_ms(&self) -> u64 {
        self.st.borrow().t
    }
    fn top_processes(
        &self,
        _n: usize,
    ) -> Result<Vec<(PidStat, Option<String>, String)>, ProcError> {
        let st = self.st.borrow();
        Ok(st
            .procs
            .iter()
            .zip(&st.units)
            .zip(&st.users)
            .map(|((p, u), name)| (p.clone(), u.clone(), name.clone()))
            .collect())
    }
}

#[derive(Default)]
struct FakeClock(u64);

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
    fn sleep(&mut self, d: Duration) {
        self.0 += d.as_millis() as u64;
    }
}

#[test]
fn first_sample_has_no_cpu_percent() {
    let st = Rc::new(RefCell::new(FakeState::new()));
    let src = FakeSource { st: st.clone() };
    let mut clock = FakeClock::default();
    let mut s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(10_000),
        5,
        false,
    );

    let snap = s.sample_once().unwrap();
    assert!(
        snap.system.cpu_percent.is_none(),
        "sample pertama tanpa delta"
    );
    assert_eq!(
        snap.system.mem_used_bytes, 8_000_000_000,
        "total - available (ADR-2)"
    );
    assert_eq!(snap.system.mem_percent, 50.0);
}

#[test]
fn second_sample_has_cpu_percent() {
    let st = Rc::new(RefCell::new(FakeState::new()));
    st.borrow_mut().burn_cpu(1_000);
    st.borrow_mut().tick(10_000);

    let src = FakeSource { st: st.clone() };
    let mut clock = FakeClock(10_000);
    let mut s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(10_000),
        5,
        false,
    );

    let _first = s.sample_once().unwrap();

    // antara sampling: burn CPU lagi + waktu maju
    st.borrow_mut().burn_cpu(1_000);
    st.borrow_mut().tick(10_000);

    let snap = s.sample_once().unwrap();
    let pct = snap.system.cpu_percent.unwrap();
    assert!((0.0..=100.0).contains(&pct), "pct {pct} harus 0..=100");
    // busy delta 1000, total delta ~4000 → ~25%
    assert!((pct - 25.0).abs() < 2.0, "dapat {pct}, harap ~25");
}

#[test]
fn process_listing_with_unit_and_user() {
    let st = Rc::new(RefCell::new(FakeState::new()));
    {
        let mut b = st.borrow_mut();
        b.procs = vec![PidStat {
            pid: 1234,
            comm: "nginx".into(),
            utime: 10,
            stime: 5,
            rss_kb: 48_828,
        }];
        b.units = vec![Some("nginx.service".into())];
        b.users = vec!["www-data".into()];
    }

    let src = FakeSource { st };
    let mut clock = FakeClock::default();
    let mut s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(10_000),
        5,
        false,
    );
    let snap = s.sample_once().unwrap();

    assert_eq!(snap.processes.len(), 1);
    let p = &snap.processes[0];
    assert_eq!(p.systemd_unit.as_deref(), Some("nginx.service"));
    assert_eq!(p.user, "www-data");
    assert!(p.cpu_percent.is_none(), "proses pertama tanpa delta");
    assert_eq!(p.mem_rss_bytes, 48_828 * 1024);
}

#[test]
fn top_n_processes_by_rss() {
    let st = Rc::new(RefCell::new(FakeState::new()));
    {
        let mut b = st.borrow_mut();
        // 3 proses dengan RSS berbeda; top_n = 2 → yang terbesar bertahan
        for (i, kb) in [100u64, 500, 900].iter().enumerate() {
            b.procs.push(PidStat {
                pid: 100 + i as i32,
                comm: format!("p{i}"),
                utime: 0,
                stime: 0,
                rss_kb: *kb,
            });
            b.units.push(None);
            b.users.push("root".into());
        }
    }
    let src = FakeSource { st };
    let mut clock = FakeClock::default();
    let mut s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(10_000),
        2,
        false,
    );
    let snap = s.sample_once().unwrap();

    assert_eq!(snap.processes.len(), 2, "top_n=2");
    assert_eq!(snap.processes[0].comm, "p2", "RSS terbesar duluan");
    assert_eq!(snap.processes[1].comm, "p1");
}

#[test]
fn interval_clamped_in_sampler() {
    let st = Rc::new(RefCell::new(FakeState::new()));
    let src = FakeSource { st };
    let mut clock = FakeClock::default();
    // interval 100ms → di-clamp ke 1000ms (PDD §7.1)
    let s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(100),
        5,
        false,
    );
    assert_eq!(s.interval(), Duration::from_millis(1000));
}

#[test]
fn timestamp_and_interval_in_snapshot() {
    let st = Rc::new(RefCell::new(FakeState::new()));
    st.borrow_mut().t = 1_700_000_123_456;
    let src = FakeSource { st };
    let mut clock = FakeClock::default();
    let mut s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(10_000),
        5,
        false,
    );
    let snap = s.sample_once().unwrap();
    assert_eq!(snap.timestamp_ms, 1_700_000_123_456);
    assert_eq!(snap.interval_ms, 10_000);
    assert!(!snap.host_id.is_empty(), "host_id terisi (hostname)");
}

// referensi agar ProcFs tetap dipakai (dipakai sampler produksi)
#[allow(dead_code)]
fn _fs_type_check() {
    let _ = ProcFs::system();
}
