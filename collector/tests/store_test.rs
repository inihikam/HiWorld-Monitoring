//! TDD RED: SQLite store (Task D1) — schema, insert, register host,
//! retensi. Integration test pakai SQLite in-memory.

use hiworld_collector::store::Store;
use hiworld_core::models::Snapshot;

fn fixture_snapshot(host: &str, ts_ms: u64, cpu: f64) -> Snapshot {
    serde_json::from_str(&format!(
        r#"{{
        "host_id": "{host}", "timestamp_ms": {ts_ms}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": {cpu}, "cpu_per_core": [], "load_avg": [0.1,0.2,0.3],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        }},
        "processes": []
    }}"#
    ))
    .unwrap()
}

#[test]
fn open_creates_schema() {
    let store = Store::open_in_memory().unwrap();
    // tabel kunci harus ada (hosts, users, metrics_raw, metrics_rollup, events)
    let tables = store.table_names().unwrap();
    for t in ["hosts", "users", "metrics_raw", "metrics_rollup", "events"] {
        assert!(
            tables.contains(&t.to_string()),
            "tabel {t} hilang; ada: {tables:?}"
        );
    }
}

#[test]
fn insert_and_query_snapshot() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_700_000_000_000, 42.5))
        .unwrap();

    let rows = store
        .query_system_history("web-01", 1_700_000_000_000 - 1, 1_700_000_000_001)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].timestamp_ms, 1_700_000_000_000);
    assert!((rows[0].cpu_percent - 42.5).abs() < 0.001);
    assert_eq!(rows[0].mem_used_bytes, 500);
}

#[test]
fn latest_snapshot_per_host() {
    let store = Store::open_in_memory().unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_000, 10.0))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 2_000, 20.0))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("db-01", 1_500, 30.0))
        .unwrap();

    let latest = store.latest_snapshot("web-01").unwrap().unwrap();
    assert_eq!(latest.cpu_percent, 20.0);
    let latest2 = store.latest_snapshot("db-01").unwrap().unwrap();
    assert_eq!(latest2.cpu_percent, 30.0);
    assert!(store.latest_snapshot("no-host").unwrap().is_none());
}

#[test]
fn host_register_idempotent() {
    let store = Store::open_in_memory().unwrap();

    // register pertama
    let id1 = store
        .register_host("web-01", "http://10.0.0.1:9100")
        .unwrap();
    // register ulang host sama → id sama (tidak duplikat)
    let id2 = store
        .register_host("web-01", "http://10.0.0.1:9100")
        .unwrap();
    assert_eq!(id1, id2);

    let hosts = store.list_hosts().unwrap();
    assert_eq!(hosts.len(), 1, "register 2x → 1 baris (ARCH-AC-021)");
    assert_eq!(hosts[0].host_id, "web-01");

    // host lain → id beda
    let id3 = store
        .register_host("db-01", "http://10.0.0.2:9100")
        .unwrap();
    assert_ne!(id1, id3);
}

#[test]
fn retention_purges_old_rows() {
    let store = Store::open_in_memory().unwrap();
    // 3 snapshot: 2 lama (di luar TTL), 1 baru
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_000, 1.0))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 2_000, 2.0))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 999_000, 3.0))
        .unwrap();

    let purged = store.purge_older_than(500_000).unwrap();
    assert!(purged >= 2, "minimal 2 baris lama terhapus, dapat {purged}");

    let rows = store
        .query_system_history("web-01", 0, 2_000_000_000)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cpu_percent, 3.0);
}

#[test]
fn rollup_downsample() {
    let store = Store::open_in_memory().unwrap();
    // 3 sample dalam bucket 60s yang sama
    store
        .insert_snapshot(&fixture_snapshot("web-01", 1_000, 10.0))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 2_000, 30.0))
        .unwrap();
    store
        .insert_snapshot(&fixture_snapshot("web-01", 3_000, 20.0))
        .unwrap();

    store.downsample(60).unwrap(); // bucket 60 detik

    let rollups = store.query_rollup("web-01", 0, 1_000_000).unwrap();
    assert_eq!(rollups.len(), 1, "3 sample → 1 bucket rollup");
    assert!((rollups[0].cpu_avg - 20.0).abs() < 0.001);
    assert!((rollups[0].cpu_min - 10.0).abs() < 0.001);
    assert!((rollups[0].cpu_max - 30.0).abs() < 0.001);
}
