//! TDD: /metrics Prometheus text format (Task C3, ADR-10).
//! Golden test: render dari Snapshot fixture → assert output exact.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use hiworld_agent::api::{router, AppState};
use hiworld_agent::config::{AgentConfig, SamplingConfig, ServerConfig};
use hiworld_agent::metrics::render_prometheus;
use hiworld_agent::ring_buffer::RingBuffer;
use hiworld_core::models::{DiskMetrics, NetMetrics, ProcessInfo, Snapshot, SystemMetrics};

fn fixture_snapshot() -> Snapshot {
    serde_json::from_str(
        r#"{
        "host_id": "web-01", "timestamp_ms": 1700000000000, "interval_ms": 10000,
        "system": {
            "cpu_percent": 42.5, "cpu_per_core": [10.0, 75.0],
            "load_avg": [0.5, 0.25, 0.125],
            "mem_total_bytes": 16777216000, "mem_used_bytes": 8388608000, "mem_percent": 50.0,
            "swap_total_bytes": 4294967296, "swap_used_bytes": 1073741824,
            "disks": [{
                "mount": "/", "device": "/dev/sda1",
                "total_bytes": 100000000000, "used_bytes": 25000000000, "percent": 25.0,
                "read_bytes": 486400000, "write_bytes": 435200000
            }],
            "net": [{"interface": "eth0", "rx_bytes": 9876543210, "tx_bytes": 5555555555}]
        },
        "processes": [
            {"pid": 1234, "comm": "nginx", "cmdline": "nginx: worker process",
             "user": "www-data", "systemd_unit": "nginx.service",
             "cpu_percent": 7.25, "mem_rss_bytes": 50003456, "mem_pss_bytes": null}
        ]
    }"#,
    )
    .unwrap()
}

fn empty_snapshot(host: &str) -> Snapshot {
    Snapshot {
        host_id: host.into(),
        timestamp_ms: 1,
        interval_ms: 10000,
        system: SystemMetrics {
            cpu_percent: None,
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
        processes: vec![],
    }
}

// ---------- GOLDEN test render ----------

#[test]
fn golden_render_prometheus_text() {
    let out = render_prometheus(&fixture_snapshot());
    let expected = "\
# HELP hiworld_cpu_percent System CPU usage percent (all cores)
# TYPE hiworld_cpu_percent gauge
hiworld_cpu_percent 42.5
# HELP hiworld_cpu_core_percent CPU usage percent per core
# TYPE hiworld_cpu_core_percent gauge
hiworld_cpu_core_percent{core=\"0\"} 10
hiworld_cpu_core_percent{core=\"1\"} 75
# HELP hiworld_load_avg Load average (1m,5m,15m)
# TYPE hiworld_load_avg gauge
hiworld_load_avg{window=\"1m\"} 0.5
hiworld_load_avg{window=\"5m\"} 0.25
hiworld_load_avg{window=\"15m\"} 0.125
# HELP hiworld_mem_total_bytes Total RAM bytes
# TYPE hiworld_mem_total_bytes gauge
hiworld_mem_total_bytes 16777216000
# HELP hiworld_mem_used_bytes Used RAM bytes (total - available)
# TYPE hiworld_mem_used_bytes gauge
hiworld_mem_used_bytes 8388608000
# HELP hiworld_mem_percent Used RAM percent
# TYPE hiworld_mem_percent gauge
hiworld_mem_percent 50
# HELP hiworld_swap_used_bytes Used swap bytes
# TYPE hiworld_swap_used_bytes gauge
hiworld_swap_used_bytes 1073741824
# HELP hiworld_disk_total_bytes Disk total bytes per mountpoint
# TYPE hiworld_disk_total_bytes gauge
hiworld_disk_total_bytes{mount=\"/\",device=\"/dev/sda1\"} 100000000000
# HELP hiworld_disk_used_bytes Disk used bytes per mountpoint
# TYPE hiworld_disk_used_bytes gauge
hiworld_disk_used_bytes{mount=\"/\",device=\"/dev/sda1\"} 25000000000
# HELP hiworld_disk_percent Disk used percent per mountpoint
# TYPE hiworld_disk_percent gauge
hiworld_disk_percent{mount=\"/\",device=\"/dev/sda1\"} 25
# HELP hiworld_disk_read_bytes_total Disk cumulative read bytes
# TYPE hiworld_disk_read_bytes_total counter
hiworld_disk_read_bytes_total{mount=\"/\",device=\"/dev/sda1\"} 486400000
# HELP hiworld_disk_write_bytes_total Disk cumulative write bytes
# TYPE hiworld_disk_write_bytes_total counter
hiworld_disk_write_bytes_total{mount=\"/\",device=\"/dev/sda1\"} 435200000
# HELP hiworld_net_rx_bytes_total Network cumulative received bytes
# TYPE hiworld_net_rx_bytes_total counter
hiworld_net_rx_bytes_total{interface=\"eth0\"} 9876543210
# HELP hiworld_net_tx_bytes_total Network cumulative transmitted bytes
# TYPE hiworld_net_tx_bytes_total counter
hiworld_net_tx_bytes_total{interface=\"eth0\"} 5555555555
# HELP hiworld_process_cpu_percent Process CPU usage percent (of total capacity)
# TYPE hiworld_process_cpu_percent gauge
hiworld_process_cpu_percent{pid=\"1234\",comm=\"nginx\",user=\"www-data\",unit=\"nginx.service\"} 7.25
# HELP hiworld_process_mem_rss_bytes Process resident memory bytes
# TYPE hiworld_process_mem_rss_bytes gauge
hiworld_process_mem_rss_bytes{pid=\"1234\",comm=\"nginx\",user=\"www-data\",unit=\"nginx.service\"} 50003456
";
    assert_eq!(out, expected, "format prometheus terkunci (ADR-10)");
}

#[test]
fn cpu_none_omits_metric_but_keeps_mem() {
    let mut snap = fixture_snapshot();
    snap.system.cpu_percent = None;
    let out = render_prometheus(&snap);
    assert!(
        !out.contains("hiworld_cpu_percent 4"),
        "cpu None → sample absent (HELP/TYPE boleh ada)"
    );
    assert!(
        out.contains("hiworld_mem_used_bytes 8388608000"),
        "mem tetap ada"
    );
}

#[test]
fn process_without_unit_omits_unit_label() {
    let mut snap = fixture_snapshot();
    snap.processes[0].systemd_unit = None;
    let out = render_prometheus(&snap);
    assert!(
        out.contains("pid=\"1234\",comm=\"nginx\",user=\"www-data\",unit=\"\"}"),
        "unit kosong tetap ada sebagai label kosong; dapat:\n{out}"
    );
}

#[test]
fn first_sample_snapshot_renders_without_cpu() {
    let out = render_prometheus(&empty_snapshot("fresh-host"));
    assert!(
        !out.contains("hiworld_cpu_percent 4"),
        "sample pertama: tanpa sample line cpu"
    );
    assert!(out.contains("hiworld_mem_total_bytes 1"));
}

#[test]
fn render_never_panics_on_special_chars_in_labels() {
    let mut snap = fixture_snapshot();
    snap.processes[0].comm = "nginx\"bad\\slash".into();
    let out = render_prometheus(&snap); // tidak boleh panic
    assert!(out.contains("\\\""), "quote di-escape");
    assert!(out.contains("\\\\"), "backslash di-escape");
}

// ---------- endpoint /metrics pakai format yang sama ----------

fn app(snapshot: Option<Snapshot>) -> axum::Router {
    let state = std::sync::Arc::new(std::sync::Mutex::new(AppState {
        config: AgentConfig {
            server: ServerConfig {
                bind_addr: "127.0.0.1".into(),
                port: 0,
                auth_token: "t".into(),
            },
            sampling: SamplingConfig {
                interval_ms: 10_000,
                top_n_processes: 5,
                collect_pss: false,
            },
            collector: None,
        },
        latest: snapshot,
        backlog: RingBuffer::new(1, std::time::Duration::from_secs(1)),
        started_at_ms: 0,
    }));
    router(state)
}

#[tokio::test]
async fn metrics_endpoint_serves_prometheus_format() {
    let app = app(Some(fixture_snapshot()));
    let req = Request::builder()
        .uri("/metrics")
        .header("Authorization", "Bearer t")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("Content-Type").unwrap().to_str().unwrap();
    assert!(
        ct.starts_with("text/plain"),
        "prometheus text format, dapat {ct}"
    );
    let body = axum::body::to_bytes(res.into_body(), 1_000_000)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("hiworld_cpu_percent 42.5"));
}
