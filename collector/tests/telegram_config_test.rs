//! TDD: TelegramConfig (Task TA1, TA-AC-001..003).
//! Zero-config aman; enabled tanpa token → disabled; severity invalid → fallback.

use hiworld_collector::telegram::{Severity, TelegramConfig};

#[test]
fn empty_section_disables() {
    // TOML tanpa [telegram] → default: disabled, nilai aman
    let cfg = TelegramConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.bot_token.is_empty());
    assert!(cfg.chat_id.is_empty());
    assert_eq!(cfg.min_severity, Severity::Warning);
    assert_eq!(cfg.max_per_minute, 20);
}

#[test]
fn parse_full_config() {
    let raw = r#"
enabled = true
bot_token = "123:ABC"
chat_id = "456"
min_severity = "critical"
max_per_minute = 5
"#;
    let cfg: TelegramConfig = toml::from_str(raw).unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.bot_token, "123:ABC");
    assert_eq!(cfg.chat_id, "456");
    assert_eq!(cfg.min_severity, Severity::Critical);
    assert_eq!(cfg.max_per_minute, 5);
}

#[test]
fn parse_missing_fields_uses_defaults() {
    // hanya enabled → sisanya default
    let cfg: TelegramConfig = toml::from_str("enabled = true").unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.min_severity, Severity::Warning);
    assert_eq!(cfg.max_per_minute, 20);
}

#[test]
fn invalid_severity_falls_back_to_warning() {
    // serde tidak gagal: unknown → warning fallback (TA-AC-003)
    let cfg: TelegramConfig = toml::from_str("min_severity = \"extreme\"").unwrap();
    assert_eq!(cfg.min_severity, Severity::Warning);
}

#[test]
fn severity_ordering_for_filter() {
    // info < warning < critical (TA-AC-010/011)
    assert!(Severity::Info < Severity::Warning);
    assert!(Severity::Warning < Severity::Critical);
    assert!(Severity::Critical >= Severity::Warning);
}

#[test]
fn is_configured_requires_token_and_chat() {
    let mut cfg = TelegramConfig::default();
    cfg.enabled = true;
    assert!(
        !cfg.is_configured(),
        "enabled tanpa token → tidak configured"
    );

    cfg.bot_token = "tok".into();
    assert!(
        !cfg.is_configured(),
        "token tanpa chat_id → tidak configured"
    );

    cfg.chat_id = "chat".into();
    assert!(cfg.is_configured());
}
