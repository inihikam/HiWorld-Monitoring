//! TDD RED: golden + property test CPU% (ARCH-AC-012, ARCH-AC-013).
//! Modul `units` belum ada → test ini HARUS gagal dulu.

use hiworld_core::proc_parser::{CpuTime, PidStat};
use hiworld_core::units::{cpu_percent, cpu_percent_proc};

fn cpu(user: u64, idle: u64) -> CpuTime {
    CpuTime {
        user,
        nice: 0,
        system: 0,
        idle,
        iowait: 0,
        irq: 0,
        softirq: 0,
        steal: 0,
    }
}

// ---------- GOLDEN (ARCH-AC-013) ----------

#[test]
fn golden_50_percent() {
    let t0 = cpu(600, 400); // total 1000, busy 600
    let t1 = cpu(1100, 900); // total 2000, busy 1100
    let pct = cpu_percent(&t0, &t1).unwrap();
    assert!((pct - 50.0).abs() < 0.1, "dapat {pct}");
}

#[test]
fn golden_100_percent() {
    let t0 = cpu(0, 1000);
    let t1 = cpu(1000, 1000); // idle tidak bergerak, user naik 1000
    let pct = cpu_percent(&t0, &t1).unwrap();
    assert!((pct - 100.0).abs() < 0.1, "dapat {pct}");
}

#[test]
fn golden_0_percent() {
    let t0 = cpu(1000, 1000);
    let t1 = cpu(1000, 2000); // hanya idle yang naik
    let pct = cpu_percent(&t0, &t1).unwrap();
    assert!((pct - 0.0).abs() < 0.1, "dapat {pct}");
}

#[test]
fn golden_iowait_dikecualikan_dari_busy() {
    // iowait naik 500, user tetap → CPU% = 0 (iowait bukan "terpakai")
    let t0 = CpuTime {
        user: 1000,
        nice: 0,
        system: 0,
        idle: 1000,
        iowait: 0,
        irq: 0,
        softirq: 0,
        steal: 0,
    };
    let t1 = CpuTime {
        user: 1000,
        nice: 0,
        system: 0,
        idle: 1000,
        iowait: 500,
        irq: 0,
        softirq: 0,
        steal: 0,
    };
    let pct = cpu_percent(&t0, &t1).unwrap();
    assert!((pct - 0.0).abs() < 0.1, "dapat {pct}");
}

#[test]
fn golden_delta_tidak_maju_none() {
    // total jiffies sama → interval tidak valid → None (bukan 0% palsu)
    let t = cpu(1000, 1000);
    assert!(cpu_percent(&t, &t).is_none());
    // total mundur (kernel reset — praktis tak terjadi, tapi harus aman)
    let a = cpu(2000, 2000);
    let b = cpu(1000, 1000);
    assert!(cpu_percent(&a, &b).is_none());
}

#[test]
fn golden_proc_50_percent() {
    let p0 = PidStat {
        pid: 1,
        comm: "x".into(),
        utime: 100,
        stime: 100,
        rss_kb: 1000,
    };
    let p1 = PidStat {
        pid: 1,
        comm: "x".into(),
        utime: 300,
        stime: 300,
        rss_kb: 1000,
    };
    // proses: jiffies naik 400; system: total naik 800 → 50%
    let s0 = cpu(0, 800);
    let s1 = cpu(0, 1600);
    let pct = cpu_percent_proc(&p0, &p1, &s0, &s1).unwrap();
    assert!((pct - 50.0).abs() < 0.1, "dapat {pct}");
}

// ---------- PROPERTY (ARCH-AC-012) ----------

use proptest::prelude::*;

prop_compose! {
    fn arb_cpu()(v in 0u64..1_000_000) -> CpuTime {
        // komponen acak, pastikan total > 0
        let idle = v % 100_000 + 1;
        CpuTime { user: v % 50_000, nice: v % 100, system: v % 20_000, idle, iowait: v % 5_000, irq: v % 1_000, softirq: v % 2_000, steal: v % 1_000 }
    }
}

proptest! {
    #[test]
    fn cpu_percent_always_0_to_100_or_none(t0 in arb_cpu(), t1 in arb_cpu()) {
        match cpu_percent(&t0, &t1) {
            None => prop_assert!(t1.total() <= t0.total(), "None hanya bila delta <= 0"),
            Some(pct) => prop_assert!((0.0..=100.0).contains(&pct), "pct {pct} di luar 0..=100"),
        }
    }

    #[test]
    fn proc_percent_always_0_to_100_or_none(
        u0 in 0u64..500_000, s0 in 0u64..100_000,
        du in 0u64..500_000, ds in 0u64..100_000,
        sys0 in 1u64..2_000_000, dsys in 0u64..2_000_000,
    ) {
        let p0 = PidStat { pid: 1, comm: "x".into(), utime: u0, stime: s0, rss_kb: 1 };
        let p1 = PidStat { pid: 1, comm: "x".into(), utime: u0 + du, stime: s0 + ds, rss_kb: 1 };
        let a = cpu(0, sys0);
        let b = cpu(0, sys0 + dsys);
        match cpu_percent_proc(&p0, &p1, &a, &b) {
            None => prop_assert!(b.total() <= a.total()),
            Some(pct) => prop_assert!((0.0..=100.0).contains(&pct), "pct {pct}"),
        }
    }
}
