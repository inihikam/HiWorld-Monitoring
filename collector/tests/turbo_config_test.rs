//! TDD: TurboConfig (Task TM1, TM-AC-001..003).

use hiworld_collector::turbo::TurboConfig;

#[test]
fn default_is_disabled_and_clamped() {
    let cfg = TurboConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.interval_ms, 1000);
    assert_eq!(cfg.min_duration_ms, 60_000);
}

#[test]
fn parse_full_config() {
    let cfg: TurboConfig = toml::from_str(
        r#"
enabled = true
interval_ms = 2000
min_duration_ms = 120_000
"#,
    )
    .unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.interval_ms, 2000);
    assert_eq!(cfg.min_duration_ms, 120_000);
}

#[test]
fn interval_clamped_low() {
    // 500 → 1000 (clamp sama dengan agent, TM-AC-002)
    let cfg: TurboConfig = toml::from_str("interval_ms = 500").unwrap();
    assert_eq!(cfg.interval_ms, 1000);
}

#[test]
fn interval_clamped_high() {
    // 120000 → 60000
    let cfg: TurboConfig = toml::from_str("interval_ms = 120000").unwrap();
    assert_eq!(cfg.interval_ms, 60_000);
}

#[test]
fn min_duration_default_when_missing() {
    let cfg: TurboConfig = toml::from_str("enabled = true").unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.min_duration_ms, 60_000);
    assert_eq!(cfg.interval_ms, 1000); // Q-TM2: default 1 detik
}
