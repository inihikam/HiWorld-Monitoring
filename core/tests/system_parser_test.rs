//! Integration test parser system — membaca FIXTURE, bukan /proc asli
//! (ARCH-AC-010, ARCH-AC-011). Nilai assert dihitung manual dari fixture.

use std::path::PathBuf;

use hiworld_core::proc_parser::ProcFs;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proc")
}

#[test]
fn parses_stat_cpu_totals() {
    let fs = ProcFs::at_root(fixture_root());
    let stat = fs.stat().unwrap();

    // Fixture: cpu  84125 0 22310 105000 2400 0 1150 0 0 0
    assert_eq!(stat.cpu_total.user, 84125);
    assert_eq!(stat.cpu_total.nice, 0);
    assert_eq!(stat.cpu_total.system, 22310);
    assert_eq!(stat.cpu_total.idle, 105000);
    assert_eq!(stat.cpu_total.iowait, 2400);
    assert_eq!(stat.cpu_total.irq, 0);
    assert_eq!(stat.cpu_total.softirq, 1150);
    assert_eq!(stat.cpu_total.steal, 0);
    assert_eq!(stat.cpus.len(), 2, "fixture punya cpu0 & cpu1");
}

#[test]
fn parses_meminfo() {
    let fs = ProcFs::at_root(fixture_root());
    let mem = fs.meminfo().unwrap();

    assert_eq!(mem.mem_total_bytes, 16_384_000 * 1024); // MemTotal: 16384000 kB
    assert_eq!(mem.mem_available_bytes, 8_388_608 * 1024);
    assert_eq!(mem.swap_total_bytes, 4_194_304 * 1024);
    assert_eq!(mem.swap_free_bytes, 3_145_728 * 1024);
}

#[test]
fn parses_loadavg() {
    let fs = ProcFs::at_root(fixture_root());
    let load = fs.loadavg().unwrap();
    assert_eq!(load, [0.50, 0.25, 0.12]);
}

#[test]
fn parses_mounts_real_filesystems_only() {
    let fs = ProcFs::at_root(fixture_root());
    let mounts = fs.mounts().unwrap();

    // tmpfs/proc dikeluarkan — hanya device fisik/virtio blok yang dipantau.
    assert_eq!(mounts.len(), 2, "hanya /dev/sd* yang diambil dari fixture");
    assert_eq!(mounts[0].device, "/dev/sda1");
    assert_eq!(mounts[0].mount, "/");
    assert_eq!(mounts[1].device, "/dev/sdb1");
    assert_eq!(mounts[1].mount, "/home");
}

#[test]
fn parses_diskstats() {
    let fs = ProcFs::at_root(fixture_root());
    let disks = fs.diskstats().unwrap();

    // Fixture: sda1 (major 8 minor 1) sectors read 950000, written 850000.
    // Parser menyimpan sectors; konversi bytes (×512) di layer units (B3).
    let sda1 = disks.iter().find(|d| d.name == "sda1").unwrap();
    assert_eq!(sda1.sectors_read, 950_000);
    assert_eq!(sda1.sectors_written, 850_000);
    assert_eq!(
        disks.len(),
        4,
        "sda, sda1, sdb, sdb1 (parent+partition tetap diparse)"
    );
}

#[test]
fn parses_net_dev() {
    let fs = ProcFs::at_root(fixture_root());
    let nets = fs.net_dev().unwrap();

    let eth0 = nets.iter().find(|n| n.interface == "eth0").unwrap();
    assert_eq!(eth0.rx_bytes, 9_876_543_210);
    assert_eq!(eth0.tx_bytes, 5_555_555_555);
    // lo tetap diparse; keputusan filter dilakukan di layer sampler, bukan parser.
    assert!(nets.iter().any(|n| n.interface == "lo"));
    assert_eq!(nets.len(), 2);
}

#[test]
fn parses_uptime() {
    let fs = ProcFs::at_root(fixture_root());
    let up = fs.uptime().unwrap();
    assert!((up - 123_456.78).abs() < 0.001);
}
