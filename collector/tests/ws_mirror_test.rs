//! TDD: WS5 — lag handling (WS-AC-007) + mirror task latest_snapshots
//! (fondasi hello yang selalu fresh).

use std::sync::{Arc, Mutex};

use hiworld_collector::api::{router, AppState, CollectorConfig};
use hiworld_collector::hub::{BroadcastHub, BroadcastMessage};
use hiworld_collector::store::Store;

fn test_config() -> CollectorConfig {
    CollectorConfig {
        bind_addr: "127.0.0.1".into(),
        port: 0,
        static_dir: None,
        provisioning_token: "prov".into(),
        bootstrap_admin_user: "admin".into(),
        bootstrap_admin_pass: "initial-pass-123".into(),
        db_path: ":memory:".into(),
        agent_token: "agent-t".into(),
        poll_interval_ms: 10_000,
        detector: Default::default(),
    }
}

fn snapshot_of(host: &str, ts: u64) -> hiworld_core::models::Snapshot {
    serde_json::from_str(&format!(
        r#"{{
        "host_id": "{host}", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": 1.0, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1, "mem_used_bytes": 0, "mem_percent": 0.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        }},
        "processes": []
    }}"#
    ))
    .unwrap()
}

// ---------- WS-AC-007: mirror task update latest_snapshots dari broadcast ----------

#[tokio::test]
async fn mirror_task_updates_latest_snapshots() {
    let state = {
        let store = Store::open_in_memory().unwrap();
        AppState::bootstrap(store, test_config()).unwrap()
    };

    // jalankan mirror task (fungsi produksi, dipanggil di main)
    let mirror = hiworld_collector::ws::spawn_mirror_task(state.clone());

    // poller broadcast snapshot (simulasi)
    state
        .hub
        .send(BroadcastMessage::Snapshot(snapshot_of("web-01", 1_000)));
    state
        .hub
        .send(BroadcastMessage::Snapshot(snapshot_of("db-01", 2_000)));

    // beri waktu mirror memproses
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let latest = state.latest_snapshots.lock().unwrap();
    assert!(latest.contains_key("web-01"), "web-01 di-mirror");
    assert!(latest.contains_key("db-01"), "db-01 di-mirror");
    assert_eq!(latest.get("web-01").unwrap().timestamp_ms, 1_000);

    mirror.abort();
}

#[tokio::test]
async fn mirror_task_overwrites_old_snapshot() {
    let state = {
        let store = Store::open_in_memory().unwrap();
        AppState::bootstrap(store, test_config()).unwrap()
    };
    let mirror = hiworld_collector::ws::spawn_mirror_task(state.clone());

    state
        .hub
        .send(BroadcastMessage::Snapshot(snapshot_of("h", 100)));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    state
        .hub
        .send(BroadcastMessage::Snapshot(snapshot_of("h", 200)));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let latest = state.latest_snapshots.lock().unwrap();
    assert_eq!(latest.get("h").unwrap().timestamp_ms, 200, "terbaru menang");

    mirror.abort();
}

// ---------- WS-AC-007: hub lag tidak merusak state ----------

#[tokio::test]
async fn hub_lag_does_not_break_mirror() {
    let state = {
        let store = Store::open_in_memory().unwrap();
        AppState::bootstrap(store, test_config()).unwrap()
    };
    let mirror = hiworld_collector::ws::spawn_mirror_task(state.clone());

    // banjir 5000 pesan (kapasitas hub 1024) → mirror pasti Lagged
    for i in 0..5000 {
        state
            .hub
            .send(BroadcastMessage::Snapshot(snapshot_of("h", i)));
    }

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // mirror masih hidup (task tidak panic) dan state berisi snapshot terakhir
    assert!(!mirror.is_finished(), "mirror tidak boleh mati saat Lagged");
    let latest = state.latest_snapshots.lock().unwrap();
    assert!(
        latest.contains_key("h"),
        "setelah lag, mirror tetap memproses pesan berikutnya"
    );
    assert_eq!(latest.get("h").unwrap().timestamp_ms, 4999);

    mirror.abort();
}
