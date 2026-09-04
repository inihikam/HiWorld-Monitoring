//! TDD: Poller broadcast (Task WS2, WS-AC-003, WS-AC-005).
//! Poller dengan hub → snapshot/event/host_status terkirim ke subscriber.

use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use hiworld_collector::hub::{BroadcastHub, BroadcastMessage};
use hiworld_collector::poller::Poller;
use hiworld_collector::store::Store;

fn snapshot_json(ts: u64) -> String {
    format!(
        r#"{{
        "host_id": "ws-host", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": 5.0, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0,
            "disks": [{{"mount":"/","device":"/dev/sda1","total_bytes":100,
                       "used_bytes":95,"percent":95.0,"read_bytes":0,"write_bytes":0}}],
            "net": []
        }},
        "processes": [
            {{"pid": 1234, "comm": "nginx", "cmdline": "nginx",
              "user": "www-data", "systemd_unit": "nginx.service",
              "cpu_percent": 91.5, "mem_rss_bytes": 5000000, "mem_pss_bytes": null}}
        ]
    }}"#
    )
}

fn setup(agent_uri: &str) -> (Poller, BroadcastHub, std::path::PathBuf) {
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("ws2-{}-{n}.db", std::process::id()));
    let poller_store = Store::open(&path).unwrap();
    // detector store: in-memory cukup (test ini tidak menguji baseline;
    // dua koneksi file + WAL di temp memicu disk I/O error di CI)
    let detector_store = Store::open_in_memory().unwrap();
    poller_store.register_host("ws-host", agent_uri).unwrap();

    let hub = BroadcastHub::new(1024);
    let poller = Poller::with_hub(
        poller_store,
        "t".into(),
        Duration::from_millis(10),
        Default::default(),
        detector_store,
        hub.clone(),
    );
    (poller, hub, path)
}

#[tokio::test]
async fn snapshot_broadcast_on_poll() {
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(1_000)))
        .mount(&agent)
        .await;

    let (mut poller, hub, db_path) = setup(&agent.uri());
    let mut rx = hub.subscribe();

    poller.poll_once_all().await.unwrap();

    // SD-AC-003: snapshot ter-broadcast
    let msg = rx.recv().await.unwrap();
    match msg {
        BroadcastMessage::Snapshot(s) => {
            assert_eq!(s.host_id, "ws-host");
            assert_eq!(s.timestamp_ms, 1_000);
        }
        other => panic!("harus Snapshot, dapat {other:?}"),
    }
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn event_broadcast_on_spike() {
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(1_000)))
        .mount(&agent)
        .await;

    let (mut poller, hub, db_path) = setup(&agent.uri());
    let mut rx = hub.subscribe();

    // snapshot ini punya cpu 91.5% & disk 95% → detector menghasilkan event
    poller.poll_once_all().await.unwrap();

    // kumpulkan pesan sampai ketemu event (snapshot juga ter-broadcast)
    let mut got_spike = false;
    let mut got_disk = false;
    for _ in 0..5 {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(BroadcastMessage::Event { kind, .. })) => {
                if kind == "spike_cpu" {
                    got_spike = true;
                }
                if kind == "disk_almost_full" {
                    got_disk = true;
                }
            }
            Ok(Ok(_)) => continue, // snapshot, skip
            _ => break,
        }
    }
    assert!(got_spike, "spike_cpu event harus ter-broadcast");
    assert!(got_disk, "disk_almost_full event harus ter-broadcast");
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn host_status_broadcast_on_down() {
    // agent tidak hidup → poll gagal → host_status offline
    let (mut poller, hub, db_path) = setup("http://127.0.0.1:1");
    let mut rx = hub.subscribe();

    poller.poll_once_all().await.unwrap();

    match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
        Ok(Ok(BroadcastMessage::HostStatus { host_id, online })) => {
            assert_eq!(host_id, "ws-host");
            assert!(!online, "host harus offline");
        }
        other => panic!("harus HostStatus offline, dapat {other:?}"),
    }
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn host_status_broadcast_on_recovery() {
    let agent = MockServer::start().await;

    let (mut poller, hub, db_path) = setup(&agent.uri());
    let mut rx = hub.subscribe();

    // fase 1: down (belum mount mock / snapshot → poll gagal)
    // trik: poll sebelum mock terpasang
    drop(agent);
    let down_server = MockServer::start().await;
    poller
        .store()
        .register_host("ws-host", &down_server.uri())
        .unwrap();
    poller.poll_once_all().await.unwrap(); // 404 → offline

    // fase 2: up — mock hidup di server asli (port beda, tapi host same id)
    // gunakan server yang sama: re-mount via new server
    let up_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(2_000)))
        .mount(&up_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backlog"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&up_server)
        .await;
    poller
        .store()
        .register_host("ws-host", &up_server.uri())
        .unwrap();
    poller.poll_once_all().await.unwrap();

    // kumpulkan pesan: harus ada HostStatus offline lalu online
    let mut saw_offline = false;
    let mut saw_online = false;
    for _ in 0..10 {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(BroadcastMessage::HostStatus { online, .. })) => {
                if !online {
                    saw_offline = true;
                } else {
                    saw_online = true;
                    break;
                }
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(saw_offline, "harus ada host_status offline");
    assert!(saw_online, "harus ada host_status online (recovery)");
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn no_hub_still_works() {
    // WS-AC-009: Poller::new (tanpa hub) kompatibel
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(1_000)))
        .mount(&agent)
        .await;

    let path = std::env::temp_dir().join(format!("ws2-nohub-{}.db", std::process::id()));
    let poller_store = Store::open(&path).unwrap();
    let detector_store = Store::open(&path).unwrap();
    poller_store.register_host("ws-host", &agent.uri()).unwrap();

    let mut poller = Poller::new(poller_store, "t".into(), Duration::from_millis(10));
    poller.poll_once_all().await.unwrap();
    assert!(poller.store().latest_snapshot("ws-host").unwrap().is_some());
    let _ = std::fs::remove_file(&path);
}
