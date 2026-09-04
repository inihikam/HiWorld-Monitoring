//! TDD: config `[detector]` (Task SD1, SD-AC-032).

use hiworld_collector::api::CollectorConfig;

fn base_toml(extra: &str) -> String {
    format!(
        r#"
bind_addr = "127.0.0.1"
port = 8080
provisioning_token = "prov"
bootstrap_admin_user = "admin"
bootstrap_admin_pass = "initial-pass-123"
db_path = "/tmp/test.db"
agent_token = "agent-t"
poll_interval_ms = 10000
{extra}
"#
    )
}

#[test]
fn detector_defaults_when_section_absent() {
    let cfg: CollectorConfig = toml::from_str(&base_toml("")).unwrap();
    let d = &cfg.detector;
    // default dari PDD §3.4
    assert!((d.cpu_spike_threshold_percent - 85.0).abs() < 0.001);
    assert!((d.mem_spike_threshold_percent - 10.0).abs() < 0.001);
    assert!((d.disk_full_threshold_percent - 90.0).abs() < 0.001);
    assert_eq!(d.baseline_window_min, 30);
    assert_eq!(d.min_samples_for_baseline, 3);
}

#[test]
fn detector_values_from_toml() {
    let toml = base_toml(
        r#"
[detector]
cpu_spike_threshold_percent = 80.0
mem_spike_threshold_percent = 15.0
disk_full_threshold_percent = 85.0
baseline_window_min = 60
min_samples_for_baseline = 5
"#,
    );
    let cfg: CollectorConfig = toml::from_str(&toml).unwrap();
    let d = &cfg.detector;
    assert!((d.cpu_spike_threshold_percent - 80.0).abs() < 0.001);
    assert!((d.mem_spike_threshold_percent - 15.0).abs() < 0.001);
    assert!((d.disk_full_threshold_percent - 85.0).abs() < 0.001);
    assert_eq!(d.baseline_window_min, 60);
    assert_eq!(d.min_samples_for_baseline, 5);
}

#[test]
fn detector_partial_section_uses_defaults() {
    // hanya satu field di-set → lainnya default (serde default per field)
    let toml = base_toml(
        r#"
[detector]
cpu_spike_threshold_percent = 70.0
"#,
    );
    let cfg: CollectorConfig = toml::from_str(&toml).unwrap();
    let d = &cfg.detector;
    assert!((d.cpu_spike_threshold_percent - 70.0).abs() < 0.001);
    assert!(
        (d.mem_spike_threshold_percent - 10.0).abs() < 0.001,
        "default"
    );
    assert_eq!(d.baseline_window_min, 30, "default");
}
