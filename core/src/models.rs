//! Data models untuk hiworld-monitoring.
//!
//! Contract JSON dikunci oleh snapshot test (ARCH-AC-030): field snake_case,
//! perubahan format = test gagal = perubahan contract harus disadari.

use serde::{Deserialize, Serialize};

/// Satu hasil sampling lengkap dari sebuah host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Hostname unik per agent.
    pub host_id: String,
    /// Epoch milliseconds saat sampling.
    pub timestamp_ms: u64,
    /// Interval sampling aktif saat snapshot ini diambil (ms).
    pub interval_ms: u32,
    pub system: SystemMetrics,
    /// Top-N proses (sort by RSS; lihat ADR-3).
    pub processes: Vec<ProcessInfo>,
}

/// Metrik system-level per snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Agregat semua core, 0.0..=100.0. `None` pada sample pertama
    /// (belum ada delta jiffies — ADR-1).
    pub cpu_percent: Option<f64>,
    /// Per-core, urut index core. Panjang = jumlah core terdeteksi.
    pub cpu_per_core: Vec<Option<f64>>,
    /// Load average 1/5/15 menit dari /proc/loadavg.
    pub load_avg: [f64; 3],
    pub mem_total_bytes: u64,
    /// MemTotal − MemAvailable (ADR-2).
    pub mem_used_bytes: u64,
    pub mem_percent: f64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub disks: Vec<DiskMetrics>,
    pub net: Vec<NetMetrics>,
}

/// Metrik disk per mountpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskMetrics {
    /// Mountpoint, mis. "/", "/home".
    pub mount: String,
    /// Device source, mis. "/dev/sda1".
    pub device: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub percent: f64,
    /// Bytes read/written kumulatif (dari /proc/diskstats).
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// Metrik network per interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetMetrics {
    pub interface: String,
    /// Counter kumulatif rx/tx (dari /proc/net/dev).
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Info satu proses (anggota top-N).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: i32,
    /// Nama proses pendek dari /proc/<pid>/stat (comm).
    pub comm: String,
    /// Full command line; fallback ke comm bila cmdline kosong.
    pub cmdline: String,
    /// Username (resolve uid via /etc/passwd, di-cache).
    pub user: String,
    /// systemd unit dari /proc/<pid>/cgroup; None bila di luar systemd (ADR-4).
    pub systemd_unit: Option<String>,
    /// 0.0..=100.0; None pada sample pertama proses ini (ADR-1).
    pub cpu_percent: Option<f64>,
    pub mem_rss_bytes: u64,
    /// PSS opsional — hanya bila `collect_pss = true` (ADR-3, mahal).
    pub mem_pss_bytes: Option<u64>,
}

/// Satu event anomali yang dihasilkan detector collector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorEvent {
    pub host_id: String,
    pub timestamp_ms: u64,
    pub kind: EventKind,
    pub severity: Severity,
    /// Subjek event: pid/cmdline/unit/interface/mountpoint.
    pub subject: String,
    /// Detail tambahan (bebas struktur, disimpan sebagai JSON).
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SpikeCpu,
    SpikeMem,
    DiskAlmostFull,
    AgentDown,
    AgentUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Snapshot fixture minimal untuk round-trip & snapshot test JSON.
    fn fixture_snapshot() -> Snapshot {
        Snapshot {
            host_id: "web-01".into(),
            timestamp_ms: 1_700_000_000_123,
            interval_ms: 10_000,
            system: SystemMetrics {
                cpu_percent: Some(42.5),
                cpu_per_core: vec![Some(10.0), None],
                load_avg: [0.5, 0.25, 0.125],
                mem_total_bytes: 8_000_000_000,
                mem_used_bytes: 3_000_000_000,
                mem_percent: 37.5,
                swap_total_bytes: 1_000_000_000,
                swap_used_bytes: 0,
                disks: vec![DiskMetrics {
                    mount: "/".into(),
                    device: "/dev/sda1".into(),
                    total_bytes: 100_000_000_000,
                    used_bytes: 25_000_000_000,
                    percent: 25.0,
                    read_bytes: 1_000,
                    write_bytes: 2_000,
                }],
                net: vec![NetMetrics {
                    interface: "eth0".into(),
                    rx_bytes: 100,
                    tx_bytes: 200,
                }],
            },
            processes: vec![ProcessInfo {
                pid: 1234,
                comm: "nginx".into(),
                cmdline: "nginx: worker process".into(),
                user: "www-data".into(),
                systemd_unit: Some("nginx.service".into()),
                cpu_percent: Some(7.25),
                mem_rss_bytes: 50_000_000,
                mem_pss_bytes: None,
            }],
        }
    }

    #[test]
    fn serde_roundtrip_snapshot_is_identical() {
        let s = fixture_snapshot();
        let json = serde_json::to_string(&s).unwrap();
        let back: Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn event_roundtrip_is_identical() {
        let e = MonitorEvent {
            host_id: "db-01".into(),
            timestamp_ms: 1,
            kind: EventKind::SpikeCpu,
            severity: Severity::Warning,
            subject: "nginx.service".into(),
            detail: serde_json::json!({"cpu_percent": 95.5}),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: MonitorEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
    }

    /// Kunci format JSON: field snake_case, enum snake_case.
    /// Perubahan apapun pada format ini harus menyadari reviewer (ARCH-AC-030).
    #[test]
    fn json_format_is_locked() {
        let s = fixture_snapshot();
        let v: serde_json::Value = serde_json::to_value(&s).unwrap();

        // field top-level
        let top = v.as_object().unwrap();
        for key in [
            "host_id",
            "timestamp_ms",
            "interval_ms",
            "system",
            "processes",
        ] {
            assert!(top.contains_key(key), "field `{key}` hilang dari JSON");
        }

        // system metrics
        let sys = v["system"].as_object().unwrap();
        for key in [
            "cpu_percent",
            "cpu_per_core",
            "load_avg",
            "mem_total_bytes",
            "mem_used_bytes",
            "mem_percent",
            "swap_total_bytes",
            "swap_used_bytes",
            "disks",
            "net",
        ] {
            assert!(sys.contains_key(key), "system.`{key}` hilang dari JSON");
        }

        // proses & disk
        let p = &v["processes"][0];
        for key in [
            "pid",
            "comm",
            "cmdline",
            "user",
            "systemd_unit",
            "cpu_percent",
            "mem_rss_bytes",
            "mem_pss_bytes",
        ] {
            assert!(
                p.as_object().unwrap().contains_key(key),
                "process.`{key}` hilang"
            );
        }
        let d = &v["system"]["disks"][0];
        for key in [
            "mount",
            "device",
            "total_bytes",
            "used_bytes",
            "percent",
            "read_bytes",
            "write_bytes",
        ] {
            assert!(
                d.as_object().unwrap().contains_key(key),
                "disk.`{key}` hilang"
            );
        }

        // enum event snake_case
        let e = MonitorEvent {
            host_id: "h".into(),
            timestamp_ms: 1,
            kind: EventKind::DiskAlmostFull,
            severity: Severity::Critical,
            subject: "/".into(),
            detail: serde_json::json!({}),
        };
        let ev: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert_eq!(ev["kind"], "disk_almost_full");
        assert_eq!(ev["severity"], "critical");
    }
}
