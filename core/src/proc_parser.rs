//! Parser /proc — murni, testable terhadap fixture (ARCH-AC-010/011).
//!
//! Semua fungsi membaca lewat [`ProcFs`] yang root-nya bisa di-inject.
//! Di produksi root = "/" ; di test root = direktori fixture.

use std::fs;
use std::path::PathBuf;

/// Sumber data /proc. Root dapat diarahkan ke fixture untuk testing.
#[derive(Debug, Clone)]
pub struct ProcFs {
    root: PathBuf,
}

impl ProcFs {
    pub fn at_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// /proc asli (produksi). /proc di-mount di /proc, bukan /.
    pub fn system() -> Self {
        Self::at_root("/proc")
    }

    fn read(&self, rel: &[&str]) -> Result<String, ParseError> {
        let mut path = self.root.clone();
        for part in rel {
            path.push(part);
        }
        fs::read_to_string(&path).map_err(|source| ParseError::Read {
            path: path.display().to_string(),
            source,
        })
    }

    pub fn stat(&self) -> Result<CpuStat, ParseError> {
        let raw = self.read(&["stat"])?;
        parse_stat(&raw).ok_or(ParseError::Format("stat: baris cpu tidak ditemukan".into()))
    }

    pub fn meminfo(&self) -> Result<MemInfo, ParseError> {
        let raw = self.read(&["meminfo"])?;
        parse_meminfo(&raw).ok_or(ParseError::Format("meminfo: field kunci hilang".into()))
    }

    pub fn loadavg(&self) -> Result<[f64; 3], ParseError> {
        let raw = self.read(&["loadavg"])?;
        parse_loadavg(&raw).ok_or(ParseError::Format("loadavg: format tidak sesuai".into()))
    }

    pub fn mounts(&self) -> Result<Vec<MountEntry>, ParseError> {
        let raw = self.read(&["mounts"])?;
        Ok(parse_mounts(&raw))
    }

    pub fn diskstats(&self) -> Result<Vec<DiskStat>, ParseError> {
        let raw = self.read(&["diskstats"])?;
        Ok(parse_diskstats(&raw))
    }

    pub fn net_dev(&self) -> Result<Vec<NetDev>, ParseError> {
        let raw = self.read(&["net", "dev"])?;
        parse_net_dev(&raw).ok_or(ParseError::Format("net/dev: header tidak ditemukan".into()))
    }

    pub fn uptime(&self) -> Result<f64, ParseError> {
        let raw = self.read(&["uptime"])?;
        raw.split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or(ParseError::Format("uptime: tidak bisa diparse".into()))
    }

    /// /proc/<pid>/stat — perlu parsing hati-hati: comm bisa mengandung
    /// spasi & kurung (mis. "tmux: server").
    pub fn pid_stat(&self, pid: i32) -> Result<PidStat, ParseError> {
        let raw = self.read(&[&pid.to_string(), "stat"])?;
        parse_pid_stat(&raw).ok_or(ParseError::Format(format!(
            "stat[{pid}]: format tidak sesuai"
        )))
    }

    /// /proc/<pid>/cmdline — argumen dipisah NUL. String kosong = cmdline kosong.
    pub fn pid_cmdline(&self, pid: i32) -> Result<String, ParseError> {
        let raw = self.read(&[&pid.to_string(), "cmdline"])?;
        Ok(raw
            .split('\0')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "))
    }

    /// /proc/<pid>/cgroup → systemd unit bila berada di system.slice (ADR-4).
    pub fn pid_systemd_unit(&self, pid: i32) -> Result<Option<String>, ParseError> {
        let raw = self.read(&[&pid.to_string(), "cgroup"])?;
        Ok(parse_systemd_unit(&raw))
    }

    /// /proc/<pid>/status → Uid efektif (field ke-2).
    pub fn pid_uid(&self, pid: i32) -> Result<Option<u32>, ParseError> {
        let raw = self.read(&[&pid.to_string(), "status"])?;
        Ok(parse_uid(&raw))
    }

    /// /proc/<pid>/smaps_rollup → PSS dalam bytes (opsional, mahal — ADR-3).
    pub fn pid_pss_bytes(&self, pid: i32) -> Result<Option<u64>, ParseError> {
        match self.read(&[&pid.to_string(), "smaps_rollup"]) {
            Ok(raw) => Ok(parse_pss_bytes(&raw)),
            // Kernel tanpa smaps_rollup / proses mati → bukan error fatal.
            Err(ParseError::Read { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Daftar PID numerik di root proc.
    pub fn list_pids(&self) -> Result<Vec<i32>, ParseError> {
        let entries = fs::read_dir(&self.root).map_err(|source| ParseError::Read {
            path: self.root.display().to_string(),
            source,
        })?;
        let mut pids = Vec::new();
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if let Ok(pid) = name.parse::<i32>() {
                    pids.push(pid);
                }
            }
        }
        pids.sort_unstable();
        Ok(pids)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("gagal membaca {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Format(String),
}

/// Alias publik agar consumer (agent/collector) tidak menyebut "ParseError"
/// untuk error yang juga mencakup I/O.
pub type ProcError = ParseError;

// ---------- struct hasil parse ----------

#[derive(Debug, Clone, PartialEq)]
pub struct CpuTime {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuTime {
    /// Total semua jiffies (denominator CPU% — ADR-1).
    pub fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    /// Jiffies non-idle (numerator CPU%): idle & iowait dikecualikan.
    /// steal tetap dihitung sebagai "terpakai" (waktu dicuri hypervisor).
    pub fn busy(&self) -> u64 {
        self.user + self.nice + self.system + self.irq + self.softirq + self.steal
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuStat {
    pub cpu_total: CpuTime,
    pub cpus: Vec<CpuTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemInfo {
    pub mem_total_bytes: u64,
    pub mem_available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MountEntry {
    pub device: String,
    pub mount: String,
    pub fstype: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskStat {
    pub name: String,
    pub sectors_read: u64,
    pub sectors_written: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetDev {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PidStat {
    pub pid: i32,
    pub comm: String,
    /// utime + stime (jiffies) — ADR-1.
    pub utime: u64,
    pub stime: u64,
    /// RSS dalam KILOBYTE (field rss dari stat, satuan halaman dikonversi di sini
    /// memakai asumsi halaman 4KB; halaman non-4KB diabaikan v1 — terdokumentasi).
    pub rss_kb: u64,
}

// ---------- fungsi parse murni (dapat diuji tanpa file) ----------

fn parse_stat(raw: &str) -> Option<CpuStat> {
    let mut cpus = Vec::new();
    let mut cpu_total = None;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            cpu_total = Some(parse_cpu_time(rest)?);
        } else if let Some(rest) = line.strip_prefix("cpu") {
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                cpus.push(parse_cpu_time(rest)?);
            }
        }
    }
    Some(CpuStat {
        cpu_total: cpu_total?,
        cpus,
    })
}

fn parse_cpu_time(rest: &str) -> Option<CpuTime> {
    let vals: Vec<u64> = rest
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    if vals.len() < 8 {
        return None;
    }
    Some(CpuTime {
        user: vals[0],
        nice: vals[1],
        system: vals[2],
        idle: vals[3],
        iowait: vals[4],
        irq: vals[5],
        softirq: vals[6],
        steal: vals[7],
    })
}

fn parse_meminfo(raw: &str) -> Option<MemInfo> {
    let get_kb = |key: &str| -> Option<u64> {
        raw.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == key).then(|| v.trim().trim_end_matches(" kB").parse::<u64>().ok())?
        })
    };
    Some(MemInfo {
        mem_total_bytes: get_kb("MemTotal")? * 1024,
        mem_available_bytes: get_kb("MemAvailable")? * 1024,
        swap_total_bytes: get_kb("SwapTotal")? * 1024,
        swap_free_bytes: get_kb("SwapFree")? * 1024,
    })
}

fn parse_loadavg(raw: &str) -> Option<[f64; 3]> {
    let vals: Vec<f64> = raw
        .split_whitespace()
        .take(3)
        .filter_map(|v| v.parse().ok())
        .collect();
    (vals.len() == 3).then_some([vals[0], vals[1], vals[2]])
}

/// Mount yang dipantau: hanya filesystem dengan device blok nyata
/// (prefix /dev/). tmpfs/proc/sysfs dikecualikan — metrik disk untuk
/// filesystem virtual menyesatkan.
fn parse_mounts(raw: &str) -> Vec<MountEntry> {
    raw.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let device = parts.next()?;
            let mount = parts.next()?;
            let fstype = parts.next()?;
            (device.starts_with("/dev/")).then(|| MountEntry {
                device: device.to_string(),
                mount: mount.to_string(),
                fstype: fstype.to_string(),
            })
        })
        .collect()
}

fn parse_diskstats(raw: &str) -> Vec<DiskStat> {
    // Field index (0-based): name=2, sectors_read=5, sectors_written=9
    // (kernel doc: reads_completed reads_merged sectors_read ms_reading ...)
    raw.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 10 {
                return None;
            }
            Some(DiskStat {
                name: f[2].to_string(),
                sectors_read: f[5].parse().ok()?,
                sectors_written: f[9].parse().ok()?,
            })
        })
        .collect()
}

fn parse_net_dev(raw: &str) -> Option<Vec<NetDev>> {
    let mut nets = Vec::new();
    for line in raw.lines().skip(2) {
        // "  eth0: 9876543210 ..." → interface sebelum ':'
        let (iface, rest) = line.split_once(':')?;
        let vals: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        if vals.len() < 9 {
            continue;
        }
        nets.push(NetDev {
            interface: iface.trim().to_string(),
            rx_bytes: vals[0],
            // rx_bytes packets errs drop fifo frame compressed multicast → tx bytes = index 8
            tx_bytes: vals[8],
        });
    }
    (!nets.is_empty()).then_some(nets)
}

fn parse_pid_stat(raw: &str) -> Option<PidStat> {
    // comm ditutup kurung terakhir — aman untuk comm berisi spasi/kurung.
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    let pid: i32 = raw[..open].trim().parse().ok()?;
    let comm = raw[open + 1..close].to_string();
    let fields: Vec<&str> = raw[close + 2..].split_whitespace().collect();
    // Setelah comm, index 0 = state (field 3 kernel). Urutan kernel setelah state:
    // 4=ppid 5=pgrp 6=session 7=tty 8=tpgid 9=flags 10=minflt 11=cminflt
    // 12=majflt 13=cmajflt 14=utime 15=stime ... 24=rss(halaman)
    // → utime idx=11, stime idx=12, rss idx=21 di vektor ini.
    if fields.len() < 22 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    let rss_pages: u64 = fields[21].parse().ok()?;
    Some(PidStat {
        pid,
        comm,
        utime,
        stime,
        rss_kb: rss_pages * 4, // halaman 4KB (asumsi terdokumentasi, x86_64)
    })
}

/// Cari systemd unit: baris "0::/system.slice/nginx.service" → "nginx.service".
/// Hanya system.slice yang dianggap unit systemd (ADR-4); scope/user.slice → None.
fn parse_systemd_unit(raw: &str) -> Option<String> {
    raw.lines().find_map(|line| {
        let path = line.rsplit(':').next()?.trim();
        let after_slice = path.strip_prefix("/system.slice/")?;
        // Sub-cgroup (mis. nginx.service/child.scope) → ambil segmen pertama.
        let unit = after_slice.split('/').next()?;
        (!unit.is_empty()).then(|| unit.to_string())
    })
}

fn parse_uid(raw: &str) -> Option<u32> {
    raw.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        if k.trim() != "Uid" {
            return None;
        }
        // Uid: real effective saved fs → efektif = field ke-2.
        v.split_whitespace().nth(1)?.parse().ok()
    })
}

fn parse_pss_bytes(raw: &str) -> Option<u64> {
    raw.lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == "Pss").then(|| v.trim().trim_end_matches(" kB").parse::<u64>().ok())?
        })
        .map(|kb| kb * 1024)
}

/// Ekstrak angka kedua dari baris "cpu  N N N ..." — utilitas untuk golden test.
pub fn cpu_time_at(raw: &str, line_prefix: &str) -> Option<CpuTime> {
    raw.lines()
        .find_map(|l| l.strip_prefix(line_prefix))
        .and_then(parse_cpu_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Golden case dari PDD ARCH-AC-013: total t0=1000 t1=2000, idle t0=400 t1=900
    /// → busy delta = 1100-600=500? TIDAK — hitung: busy(t)=total(t)-idle(t)-iowait(t).
    /// Di fixture ini iowait 0: busy0=600, busy1=1100, total delta=1000 → 50%.
    #[test]
    fn golden_cpu_percent_50() {
        let t0 = CpuTime {
            user: 600,
            nice: 0,
            system: 0,
            idle: 400,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        };
        let t1 = CpuTime {
            user: 1100,
            nice: 0,
            system: 0,
            idle: 900,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        };
        let dt = t1.total() - t0.total();
        let db = t1.busy() - t0.busy();
        assert_eq!(dt, 1000);
        assert_eq!(db, 500);
        let pct = db as f64 / dt as f64 * 100.0;
        assert!((pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn pid_stat_handles_comm_with_spaces_and_parens() {
        let raw = "5678 (tmux: server) S 5000 5678 5678 0 -1 4194304 500 10 0 0 100 50 0 0 20 0 2 0 1690000456 10000000 2440 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0";
        let st = parse_pid_stat(raw).unwrap();
        assert_eq!(st.pid, 5678);
        assert_eq!(st.comm, "tmux: server");
        assert_eq!(st.utime, 100);
        assert_eq!(st.stime, 50);
        assert_eq!(st.rss_kb, 2440 * 4);
    }

    #[test]
    fn systemd_unit_variants() {
        assert_eq!(
            parse_systemd_unit("0::/system.slice/nginx.service"),
            Some("nginx.service".into())
        );
        // non-systemd → None
        assert_eq!(
            parse_systemd_unit("0::/user.slice/user-1000.slice/session-3.scope"),
            None
        );
        // sub-cgroup → segmen pertama
        assert_eq!(
            parse_systemd_unit("0::/system.slice/docker.service/docker/abc123"),
            Some("docker.service".into())
        );
        assert_eq!(parse_systemd_unit(""), None);
    }

    #[test]
    fn cmdline_nul_separated_join() {
        let fs = ProcFs::at_root(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proc"));
        assert_eq!(fs.pid_cmdline(1234).unwrap(), "nginx: worker process");
        // cmdline kosong → string kosong (caller fallback ke comm)
        assert_eq!(fs.pid_cmdline(5678).unwrap(), "");
    }

    #[test]
    fn uid_effective_extracted() {
        let fs = ProcFs::at_root(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proc"));
        assert_eq!(fs.pid_uid(1234).unwrap(), Some(33)); // www-data
        assert_eq!(fs.pid_uid(5678).unwrap(), Some(1000));
    }

    #[test]
    fn pss_from_smaps_rollup() {
        let fs = ProcFs::at_root(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/proc"));
        assert_eq!(fs.pid_pss_bytes(1234).unwrap(), Some(40_000 * 1024));
    }
}
