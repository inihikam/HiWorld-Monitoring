//! Sampler loop agent (Task C1): baca /proc berkala → Snapshot.
//!
//! Trait `ProcSource` + `Clock` memisahkan I/O & waktu dari logika —
//! testable dengan fake (sampler_test.rs). Di produksi:
//! source = ProcFs::system(), clock = SystemClock.

use std::time::Duration;

use hiworld_core::models::{DiskMetrics, NetMetrics, ProcessInfo, Snapshot, SystemMetrics};
use hiworld_core::proc_parser::{CpuStat, CpuTime, MemInfo, PidStat, ProcError, ProcFs};
use hiworld_core::units::{cpu_percent, cpu_percent_proc, sectors_to_bytes};

/// Sumber data /proc yang dapat di-mock.
pub trait ProcSource {
    fn stat(&self) -> Result<CpuStat, ProcError>;
    fn meminfo(&self) -> Result<MemInfo, ProcError>;
    fn loadavg(&self) -> Result<[f64; 3], ProcError>;
    fn uptime(&self) -> Result<f64, ProcError>;
    fn now_ms(&self) -> u64;
    /// Top proses: (PidStat, systemd_unit, username) — sort dilakukan sampler.
    fn top_processes(&self, n: usize) -> Result<Vec<(PidStat, Option<String>, String)>, ProcError>;
}

/// Sumber produksi: /proc asli.
pub struct SystemSource {
    fs: ProcFs,
}

impl SystemSource {
    pub fn new() -> Self {
        Self {
            fs: ProcFs::system(),
        }
    }
}

impl Default for SystemSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcSource for SystemSource {
    fn stat(&self) -> Result<CpuStat, ProcError> {
        self.fs.stat()
    }
    fn meminfo(&self) -> Result<MemInfo, ProcError> {
        self.fs.meminfo()
    }
    fn loadavg(&self) -> Result<[f64; 3], ProcError> {
        self.fs.loadavg()
    }
    fn uptime(&self) -> Result<f64, ProcError> {
        self.fs.uptime()
    }
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    fn top_processes(
        &self,
        _n: usize,
    ) -> Result<Vec<(PidStat, Option<String>, String)>, ProcError> {
        // Produksi: enumerate /proc, ambil N besar by RSS.
        // (enumerate penuh ada di collect_all_processes; v1 sampler cukup
        //  ambil semua lalu sort — N yang dipilih di sampler.)
        let pids = self.fs.list_pids()?;
        let mut out = Vec::new();
        for pid in pids {
            // proses mati saat dibaca → skip (PDD §8)
            let Ok(stat) = self.fs.pid_stat(pid) else {
                continue;
            };
            let unit = self.fs.pid_systemd_unit(pid).unwrap_or(None);
            let uid = self.fs.pid_uid(pid).ok().flatten().unwrap_or(0);
            // resolve_user butuh &mut — v1: duplikasi kecil via cache statis
            // diabaikan; uid→name di-resolve tanpa cache di jalur ini.
            let user = resolve_user_static(uid);
            out.push((stat, unit, user));
        }
        Ok(out)
    }
}

fn resolve_user_static(uid: u32) -> String {
    static CACHE: std::sync::OnceLock<std::collections::HashMap<u32, String>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        std::fs::read_to_string("/etc/passwd")
            .map(|passwd| {
                passwd
                    .lines()
                    .filter_map(|l| {
                        let f: Vec<&str> = l.split(':').collect();
                        let uid = f.get(2)?.parse::<u32>().ok()?;
                        Some((uid, f[0].to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    cache
        .get(&uid)
        .cloned()
        .unwrap_or_else(|| format!("uid:{uid}"))
}

/// Clock yang dapat di-mock (sleep tanpa tidur nyata di test).
pub trait Clock {
    fn now_ms(&self) -> u64;
    fn sleep(&mut self, d: Duration);
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Sampler stateful: menyimpan pengukuran sebelumnya untuk delta.
pub struct Sampler<'a> {
    source: Box<dyn ProcSource + 'a>,
    #[allow(dead_code)] // dipakai di loop produksi (C2); test memanggil sample_once langsung
    clock: &'a mut dyn Clock,
    interval: Duration,
    top_n: usize,
    collect_pss: bool,
    prev_cpu: Option<CpuStat>,
    prev_procs: std::collections::HashMap<i32, PidStat>,
}

impl<'a> Sampler<'a> {
    pub fn new(
        source: Box<dyn ProcSource + 'a>,
        clock: &'a mut dyn Clock,
        interval: Duration,
        top_n: usize,
        collect_pss: bool,
    ) -> Self {
        // clamp 1000..=60000 ms (PDD §7.1)
        let ms = interval.as_millis() as u64;
        let ms = ms.clamp(1000, 60_000);
        Self {
            source,
            clock,
            interval: Duration::from_millis(ms),
            top_n: top_n.max(1),
            collect_pss,
            prev_cpu: None,
            prev_procs: std::collections::HashMap::new(),
        }
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Ambil satu Snapshot. Sample pertama: cpu_percent = None (ADR-1).
    pub fn sample_once(&mut self) -> Result<Snapshot, ProcError> {
        let now_ms = self.source.now_ms();

        let cpu_now = self.source.stat()?;
        let mem = self.source.meminfo()?;
        let load = self.source.loadavg()?;

        // sample pertama: None (ADR-1); berikutnya: delta antar pengukuran
        let cpu_percent_val = self
            .prev_cpu
            .as_ref()
            .map(|cs| cs.cpu_total.clone())
            .and_then(|prev| cpu_percent(&prev, &cpu_now.cpu_total));

        // per-core
        let mut per_core = Vec::with_capacity(cpu_now.cpus.len());
        if let Some(prev) = &self.prev_cpu {
            for (i, cur) in cpu_now.cpus.iter().enumerate() {
                if let Some(p) = prev.cpus.get(i) {
                    per_core.push(cpu_percent(p, cur));
                }
            }
        } else {
            per_core = cpu_now.cpus.iter().map(|_| None).collect();
        }

        // proses: hitung delta + sort by RSS desc, ambil top_n
        let all = self.source.top_processes(usize::MAX)?;
        let mut proc_infos: Vec<ProcessInfo> = Vec::with_capacity(all.len());
        for (stat, unit, user) in &all {
            let prev = self.prev_procs.get(&stat.pid);
            let pct = prev.and_then(|p| {
                // delta proses hanya valid bila system total juga maju
                self.prev_cpu
                    .as_ref()
                    .map(|cs| cs.cpu_total.clone())
                    .and_then(|sp| cpu_percent_proc(p, stat, &sp, &cpu_now.cpu_total))
            });
            let pss = if self.collect_pss {
                // source tidak expose PSS di trait v1 — skip (fitur flag menyusul)
                None
            } else {
                None
            };
            proc_infos.push(ProcessInfo {
                pid: stat.pid,
                comm: stat.comm.clone(),
                cmdline: stat.comm.clone(), // v1: cmdline diambil di C2 via source penuh
                user: user.clone(),
                systemd_unit: unit.clone(),
                cpu_percent: pct,
                mem_rss_bytes: stat.rss_kb * 1024,
                mem_pss_bytes: pss,
            });
        }
        // sort by RSS desc, ambil top_n
        proc_infos.sort_by_key(|p| std::cmp::Reverse(p.mem_rss_bytes));
        proc_infos.truncate(self.top_n);

        // simpan state untuk sampling berikutnya
        self.prev_cpu = Some(cpu_now.clone());
        self.prev_procs.clear();
        for (stat, _, _) in &all {
            self.prev_procs.insert(stat.pid, stat.clone());
        }

        let mem_used = mem.mem_total_bytes.saturating_sub(mem.mem_available_bytes);
        let mem_percent = if mem.mem_total_bytes > 0 {
            mem_used as f64 / mem.mem_total_bytes as f64 * 100.0
        } else {
            0.0
        };

        Ok(Snapshot {
            host_id: hostname(),
            timestamp_ms: now_ms,
            interval_ms: self.interval.as_millis() as u32,
            system: SystemMetrics {
                cpu_percent: cpu_percent_val,
                cpu_per_core: per_core,
                load_avg: load,
                mem_total_bytes: mem.mem_total_bytes,
                mem_used_bytes: mem_used,
                mem_percent,
                swap_total_bytes: mem.swap_total_bytes,
                swap_used_bytes: mem.swap_total_bytes.saturating_sub(mem.swap_free_bytes),
                disks: self.collect_disks().unwrap_or_default(),
                net: self.collect_net().unwrap_or_default(),
            },
            processes: proc_infos,
        })
    }

    fn collect_disks(&self) -> Result<Vec<DiskMetrics>, ProcError> {
        // Kapasitas via statvfs (fix produksi: sebelumnya TODO "C2" yang tak
        // pernah diisi → total/used/percent 0). IO counters dari diskstats.
        let stats = ProcFs::system().diskstats()?;
        let mounts = ProcFs::system().mounts()?;
        // dedupe per device: beberapa mount bisa menunjuk device yang sama
        let mut seen_devices: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out = Vec::new();
        for m in &mounts {
            let dev_name = m.device.rsplit('/').next().unwrap_or("");
            if let Some(ds) = stats.iter().find(|d| d.name == dev_name) {
                if !seen_devices.insert(m.device.clone()) {
                    continue; // satu entri per device (root mount mewakili)
                }
                let (total_bytes, free_bytes) = statvfs_bytes(&m.mount);
                let used_bytes = total_bytes.saturating_sub(free_bytes);
                let percent = if total_bytes > 0 {
                    used_bytes as f64 / total_bytes as f64 * 100.0
                } else {
                    0.0
                };
                out.push(DiskMetrics {
                    mount: m.mount.clone(),
                    device: m.device.clone(),
                    total_bytes,
                    used_bytes,
                    percent,
                    read_bytes: sectors_to_bytes(ds.sectors_read),
                    write_bytes: sectors_to_bytes(ds.sectors_written),
                });
            }
        }
        Ok(out)
    }

    fn collect_net(&self) -> Result<Vec<NetMetrics>, ProcError> {
        Ok(ProcFs::system()
            .net_dev()?
            .into_iter()
            .map(|n| NetMetrics {
                interface: n.interface,
                rx_bytes: n.rx_bytes,
                tx_bytes: n.tx_bytes,
            })
            .collect())
    }
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

/// Refrensi CpuTime untuk fake source (test).
#[allow(dead_code)]
fn _type_witness(_c: &CpuTime) {}

/// statvfs → (total, free) dalam bytes. Fallback (0,0) bila gagal.
fn statvfs_bytes(mount: &str) -> (u64, u64) {
    let c = match std::ffi::CString::new(mount) {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut st) != 0 {
            return (0, 0);
        }
        let total = st.f_blocks as u64 * st.f_frsize as u64;
        let free = st.f_bfree as u64 * st.f_frsize as u64;
        (total, free)
    }
}
