//! Render Prometheus text format (Task C3, ADR-10).
//!
//! Manual tanpa crate prometheus (dependency tipis — ARCH-AC-002 di core;
//! crate ini milik agent, tapi manual lebih mudah dikontrol golden test-nya).
//! Naming: `hiworld_<nama>_<unit>` — format terkunci oleh golden test.

use hiworld_core::models::Snapshot;

/// Escape nilai label Prometheus (quote & backslash; newline jadi \n).
fn escape_label(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn fmt_num(v: f64) -> String {
    // angka bulat tanpa desimal biar output stabil (golden test)
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

struct MetricWriter {
    out: String,
}

impl MetricWriter {
    fn new() -> Self {
        Self { out: String::new() }
    }

    fn help(&mut self, name: &str, help: &str) -> &mut Self {
        self.out.push_str(&format!("# HELP {name} {help}\n"));
        self
    }

    fn typ(&mut self, name: &str, typ: &str) -> &mut Self {
        self.out.push_str(&format!("# TYPE {name} {typ}\n"));
        self
    }

    fn sample(&mut self, name: &str, labels: &str, value: Option<f64>) -> &mut Self {
        if let Some(v) = value {
            if labels.is_empty() {
                self.out.push_str(&format!("{name} {}\n", fmt_num(v)));
            } else {
                self.out
                    .push_str(&format!("{name}{{{labels}}} {}\n", fmt_num(v)));
            }
        }
        // None → sample dilewati (metric absent), HELP/TYPE tetap konsisten
        self
    }
}

/// Render satu Snapshot menjadi text format Prometheus.
pub fn render_prometheus(snap: &Snapshot) -> String {
    let mut w = MetricWriter::new();
    let sys = &snap.system;

    // CPU system
    w.help(
        "hiworld_cpu_percent",
        "System CPU usage percent (all cores)",
    )
    .typ("hiworld_cpu_percent", "gauge")
    .sample("hiworld_cpu_percent", "", sys.cpu_percent);

    // CPU per-core
    w.help("hiworld_cpu_core_percent", "CPU usage percent per core")
        .typ("hiworld_cpu_core_percent", "gauge");
    for (i, pct) in sys.cpu_per_core.iter().enumerate() {
        w.sample("hiworld_cpu_core_percent", &format!("core=\"{i}\""), *pct);
    }

    // Load average
    w.help("hiworld_load_avg", "Load average (1m,5m,15m)")
        .typ("hiworld_load_avg", "gauge");
    for (i, window) in ["1m", "5m", "15m"].iter().enumerate() {
        w.sample(
            "hiworld_load_avg",
            &format!("window=\"{window}\""),
            Some(sys.load_avg[i]),
        );
    }

    // Memory
    w.help("hiworld_mem_total_bytes", "Total RAM bytes")
        .typ("hiworld_mem_total_bytes", "gauge")
        .sample(
            "hiworld_mem_total_bytes",
            "",
            Some(sys.mem_total_bytes as f64),
        );
    w.help(
        "hiworld_mem_used_bytes",
        "Used RAM bytes (total - available)",
    )
    .typ("hiworld_mem_used_bytes", "gauge")
    .sample(
        "hiworld_mem_used_bytes",
        "",
        Some(sys.mem_used_bytes as f64),
    );
    w.help("hiworld_mem_percent", "Used RAM percent")
        .typ("hiworld_mem_percent", "gauge")
        .sample("hiworld_mem_percent", "", Some(sys.mem_percent));
    w.help("hiworld_swap_used_bytes", "Used swap bytes")
        .typ("hiworld_swap_used_bytes", "gauge")
        .sample(
            "hiworld_swap_used_bytes",
            "",
            Some(sys.swap_used_bytes as f64),
        );

    // Disk
    for d in &sys.disks {
        let labels = format!(
            "mount=\"{}\",device=\"{}\"",
            escape_label(&d.mount),
            escape_label(&d.device)
        );
        w.help(
            "hiworld_disk_total_bytes",
            "Disk total bytes per mountpoint",
        )
        .typ("hiworld_disk_total_bytes", "gauge")
        .sample(
            "hiworld_disk_total_bytes",
            &labels,
            Some(d.total_bytes as f64),
        );
        w.help("hiworld_disk_used_bytes", "Disk used bytes per mountpoint")
            .typ("hiworld_disk_used_bytes", "gauge")
            .sample(
                "hiworld_disk_used_bytes",
                &labels,
                Some(d.used_bytes as f64),
            );
        w.help("hiworld_disk_percent", "Disk used percent per mountpoint")
            .typ("hiworld_disk_percent", "gauge")
            .sample("hiworld_disk_percent", &labels, Some(d.percent));
        w.help(
            "hiworld_disk_read_bytes_total",
            "Disk cumulative read bytes",
        )
        .typ("hiworld_disk_read_bytes_total", "counter")
        .sample(
            "hiworld_disk_read_bytes_total",
            &labels,
            Some(d.read_bytes as f64),
        );
        w.help(
            "hiworld_disk_write_bytes_total",
            "Disk cumulative write bytes",
        )
        .typ("hiworld_disk_write_bytes_total", "counter")
        .sample(
            "hiworld_disk_write_bytes_total",
            &labels,
            Some(d.write_bytes as f64),
        );
    }

    // Network
    for n in &sys.net {
        let labels = format!("interface=\"{}\"", escape_label(&n.interface));
        w.help(
            "hiworld_net_rx_bytes_total",
            "Network cumulative received bytes",
        )
        .typ("hiworld_net_rx_bytes_total", "counter")
        .sample(
            "hiworld_net_rx_bytes_total",
            &labels,
            Some(n.rx_bytes as f64),
        );
        w.help(
            "hiworld_net_tx_bytes_total",
            "Network cumulative transmitted bytes",
        )
        .typ("hiworld_net_tx_bytes_total", "counter")
        .sample(
            "hiworld_net_tx_bytes_total",
            &labels,
            Some(n.tx_bytes as f64),
        );
    }

    // Per-process
    for p in &snap.processes {
        let unit = p.systemd_unit.as_deref().unwrap_or("");
        let labels = format!(
            "pid=\"{}\",comm=\"{}\",user=\"{}\",unit=\"{}\"",
            p.pid,
            escape_label(&p.comm),
            escape_label(&p.user),
            escape_label(unit)
        );
        w.help(
            "hiworld_process_cpu_percent",
            "Process CPU usage percent (of total capacity)",
        )
        .typ("hiworld_process_cpu_percent", "gauge")
        .sample("hiworld_process_cpu_percent", &labels, p.cpu_percent);
        w.help(
            "hiworld_process_mem_rss_bytes",
            "Process resident memory bytes",
        )
        .typ("hiworld_process_mem_rss_bytes", "gauge")
        .sample(
            "hiworld_process_mem_rss_bytes",
            &labels,
            Some(p.mem_rss_bytes as f64),
        );
        if let Some(pss) = p.mem_pss_bytes {
            w.help(
                "hiworld_process_mem_pss_bytes",
                "Process proportional set size bytes (optional)",
            )
            .typ("hiworld_process_mem_pss_bytes", "gauge")
            .sample("hiworld_process_mem_pss_bytes", &labels, Some(pss as f64));
        }
    }

    w.out
}
