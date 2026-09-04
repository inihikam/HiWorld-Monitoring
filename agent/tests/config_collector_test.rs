//! TDD RED: config `[collector]` opsional (Task SR1, SR-AC-005, SR-AC-007).

use hiworld_agent::config::AgentConfig;

#[test]
fn config_without_collector_section_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.toml");
    std::fs::write(
        &path,
        r#"
[server]
bind_addr = "127.0.0.1"
port = 9100
auth_token = "secret"

[sampling]
interval_ms = 5000
"#,
    )
    .unwrap();

    let cfg = AgentConfig::load(&path).unwrap();
    assert!(
        cfg.collector.is_none(),
        "tanpa [collector] → None (SR-AC-005)"
    );
}

#[test]
fn config_with_collector_section_is_some() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.toml");
    std::fs::write(
        &path,
        r#"
[server]
bind_addr = "127.0.0.1"
port = 9100
auth_token = "secret"

[collector]
url = "http://collector:8080"
register_token = "prov-token"

[sampling]
interval_ms = 5000
"#,
    )
    .unwrap();

    let cfg = AgentConfig::load(&path).unwrap();
    let col = cfg.collector.expect("dengan [collector] → Some");
    assert_eq!(col.url, "http://collector:8080");
    assert_eq!(col.register_token, "prov-token");
    assert!(col.advertised_url.is_none(), "advertised_url opsional");
}

#[test]
fn config_advertised_url_override() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.toml");
    std::fs::write(
        &path,
        r#"
[server]
bind_addr = "127.0.0.1"
port = 9100
auth_token = "secret"

[collector]
url = "http://collector:8080"
register_token = "prov-token"
advertised_url = "http://10.0.0.5:9100"

[sampling]
interval_ms = 5000
"#,
    )
    .unwrap();

    let cfg = AgentConfig::load(&path).unwrap();
    let col = cfg.collector.unwrap();
    assert_eq!(
        col.advertised_url.as_deref(),
        Some("http://10.0.0.5:9100"),
        "advertised_url override (SR-AC-007)"
    );
}

#[test]
fn config_collector_empty_url_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.toml");
    std::fs::write(
        &path,
        r#"
[server]
bind_addr = "127.0.0.1"
port = 9100
auth_token = "secret"

[collector]
url = ""
register_token = "prov-token"
"#,
    )
    .unwrap();

    let err = AgentConfig::load(&path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("url"),
        "url kosong harus ditolak dengan pesan jelas, dapat: {msg}"
    );
}
