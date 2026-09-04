//! TDD: BroadcastHub (Task WS1, WS-AC-009).
//! Hub = wrapper broadcast channel; unit test murni tanpa HTTP.

use std::sync::Arc;

use hiworld_collector::hub::{BroadcastHub, BroadcastMessage};
use hiworld_core::models::Snapshot;

fn dummy_snapshot(host: &str, ts: u64) -> Snapshot {
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

#[tokio::test]
async fn subscribe_then_receive_snapshot() {
    let hub = BroadcastHub::new(1024);
    let mut rx = hub.subscribe();

    hub.send(BroadcastMessage::Snapshot(dummy_snapshot("web-01", 1_000)));

    let msg = rx.recv().await.unwrap();
    match msg {
        BroadcastMessage::Snapshot(s) => {
            assert_eq!(s.host_id, "web-01");
            assert_eq!(s.timestamp_ms, 1_000);
        }
        other => panic!("harus Snapshot, dapat {other:?}"),
    }
}

#[tokio::test]
async fn multiple_subscribers_all_receive() {
    let hub = BroadcastHub::new(1024);
    let mut rx1 = hub.subscribe();
    let mut rx2 = hub.subscribe();

    hub.send(BroadcastMessage::HostStatus {
        host_id: "h".into(),
        online: false,
    });

    for rx in [&mut rx1, &mut rx2] {
        match rx.recv().await.unwrap() {
            BroadcastMessage::HostStatus { host_id, online } => {
                assert_eq!(host_id, "h");
                assert!(!online);
            }
            other => panic!("harus HostStatus, dapat {other:?}"),
        }
    }
}

#[tokio::test]
async fn subscriber_after_send_does_not_receive_old() {
    // broadcast semantics: hanya subscriber AKTIF saat send yang terima
    let hub = BroadcastHub::new(1024);
    hub.send(BroadcastMessage::Event {
        host_id: "h".into(),
        kind: "spike_cpu".into(),
        severity: "warning".into(),
        subject: "x".into(),
        detail: serde_json::json!({}),
    });

    let mut late_rx = hub.subscribe();
    assert!(
        late_rx.try_recv().is_err(),
        "late subscriber tidak menerima pesan lama"
    );
}

#[tokio::test]
async fn lagged_receiver_skips_without_panic() {
    // kapasitas kecil → kirim banyak tanpa drain → recv() mengembalikan
    // Err(Lagged) (bukan panic), lalu pesan berikutnya tetap bisa diterima
    let hub = BroadcastHub::new(4);
    let mut rx = hub.subscribe();

    for i in 0..20 {
        hub.send(BroadcastMessage::Snapshot(dummy_snapshot("h", i)));
    }

    let mut got_lagged = false;
    let mut got_snapshot_after = false;
    for _ in 0..25 {
        match rx.recv().await {
            Ok(BroadcastMessage::Snapshot(_)) => {
                got_snapshot_after = true;
                break;
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                got_lagged = true;
            }
            Err(e) => panic!("error tak terduga: {e}"),
            _ => {}
        }
    }
    assert!(got_lagged || got_snapshot_after, "lag harus tertangani");
}

#[tokio::test]
async fn hub_clonable_and_shared() {
    // AppState memegang Arc<hub> — clone mengarah ke channel yang sama
    let hub = Arc::new(BroadcastHub::new(1024));
    let hub2 = hub.clone();
    let mut rx = hub.subscribe();

    hub2.send(BroadcastMessage::Event {
        host_id: "h".into(),
        kind: "agent_up".into(),
        severity: "info".into(),
        subject: "h".into(),
        detail: serde_json::json!({}),
    });

    match rx.recv().await.unwrap() {
        BroadcastMessage::Event { kind, .. } => assert_eq!(kind, "agent_up"),
        other => panic!("harus Event, dapat {other:?}"),
    }
}

// tanpa subscriber → send tetap sukses (tidak error) — poller aman saat
// tidak ada dashboard terbuka
#[tokio::test]
async fn send_without_subscribers_is_ok() {
    let hub = BroadcastHub::new(1024);
    hub.send(BroadcastMessage::Snapshot(dummy_snapshot("h", 1)));
    // tidak ada assert — yang diuji: tidak panic
}
