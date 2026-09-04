use hiworld_agent::config::AgentConfig;

#[test]
fn config_loads_valid_toml() {
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
    assert_eq!(cfg.server.port, 9100);
    assert_eq!(cfg.server.auth_token, "secret");
    assert_eq!(cfg.sampling.interval_ms, 5000);
}

#[test]
fn config_missing_file_is_clear_error() {
    let err = AgentConfig::load("/nonexistent/agent.toml").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("config"),
        "error harus menyebut 'config', dapat: {msg}"
    );
}

#[test]
fn config_invalid_toml_is_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.toml");
    std::fs::write(&path, "this is not [valid toml {{{{").unwrap();

    let err = AgentConfig::load(&path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("parse"),
        "error harus menyebut parse, dapat: {msg}"
    );
}
