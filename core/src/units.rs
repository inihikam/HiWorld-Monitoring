//! Konversi & kalkulasi unit — murni, tanpa I/O (ARCH-AC-002).
//!
//! CPU% dihitung dari DELTA jiffies antara dua snapshot (ADR-1), bukan
//! nilai instan. Semua fungsi `None` bila interval tidak valid (delta <= 0)
//! — caller (sampler) memakai ini untuk menandai sample pertama.

use crate::proc_parser::{CpuTime, PidStat};

/// CPU% system antara dua pengukuran `/proc/stat`.
///
/// - numerator: delta busy (user+nice+system+irq+softirq+steal)
///   — idle & iowait dikecualikan dari "terpakai" (sesuai `top` modern)
/// - denominator: delta total semua state
/// - `None` bila total tidak maju (sample pertama, counter reset, atau delta 0)
pub fn cpu_percent(prev: &CpuTime, curr: &CpuTime) -> Option<f64> {
    let d_total = curr.total().checked_sub(prev.total())?;
    if d_total == 0 {
        return None;
    }
    // d_busy BISA negatif secara matematis (komposisi idle/iowait berubah
    // ekstrem antar dua sampling) — clamp ke 0, bukan None: interval tetap valid.
    let d_busy = curr.busy().saturating_sub(prev.busy());
    Some((d_busy as f64 / d_total as f64 * 100.0).clamp(0.0, 100.0))
}

/// CPU% satu proses antara dua pengukuran, dinormalisasi terhadap
/// total jiffies system pada interval yang sama (bukan per-core —
/// proses multi-thread bisa >100% jika per-core; pilihan v1: agregat 0..=100
/// sesuai kontrak model ARCH §4 — nilai ini persentase dari TOTAL kapasitas).
pub fn cpu_percent_proc(
    prev: &PidStat,
    curr: &PidStat,
    sys_prev: &CpuTime,
    sys_curr: &CpuTime,
) -> Option<f64> {
    let d_total = sys_curr.total().checked_sub(sys_prev.total())?;
    if d_total == 0 {
        return None;
    }
    let d_proc = (curr.utime + curr.stime).saturating_sub(prev.utime + prev.stime);
    Some((d_proc as f64 / d_total as f64 * 100.0).clamp(0.0, 100.0))
}

/// Konversi sektor disk → bytes (sector size 512, standar kernel).
pub const SECTOR_SIZE: u64 = 512;

pub fn sectors_to_bytes(sectors: u64) -> u64 {
    sectors * SECTOR_SIZE
}

/// Format bytes ke string manusiawi (untuk log/debug; UI memformat sendiri).
pub fn human_bytes(b: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = b as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{b} {}", UNITS[0])
    } else {
        format!("{v:.2} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_conversion() {
        assert_eq!(sectors_to_bytes(2), 1024);
        assert_eq!(sectors_to_bytes(0), 0);
    }

    #[test]
    fn human_bytes_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.00 KB");
        assert_eq!(human_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.00 GB");
    }
}
