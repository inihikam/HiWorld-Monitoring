//! TDD: event agent_up saat transisi down→up (Task SD7, SD-AC-033).

use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use hiworld_collector::poller::Poller;
use hiworld_collector::store::Store;

fn snapshot_json(ts: u64) -> String {
    format!(
        r#"{{
        "host_id": "flaky-host", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": 5.0, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        }},
        "processes": []
    }}"#
    )
}

/// Helper: start mock yang bisa di-UP/DOWN via control flag.
/// Simulasi down = server tidak merespons (shutdown), up = server hidup lagi.
#[tokio::test]
async fn agent_down_then_up_events() {
    let agent = MockServer::start().await;
    let agent_uri = agent.uri().clone();

    let (state_store, poller_store, detector_store, db_path) = {
        let path = std::env::temp_dir().join(format!("sd7-{}.db", std::process::id()));
        (
            Store::open(&path).unwrap(),
            Store::open(&path).unwrap(),
            Store::open(&path).unwrap(),
            path,
        )
    };
    poller_store
        .register_host("flaky-host", &agent_uri)
        .unwrap();

    let mut poller = Poller::with_detector(
        poller_store,
        "t".into(),
        Duration::from_millis(10),
        Default::default(),
        detector_store,
    );

    // Fase 1: UP — agent hidup, snapshot masuk
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(1_000)))
        .mount(&agent)
        .await;
    poller.poll_once_all().await.unwrap();
    assert!(poller.host_status("flaky-host").online);

    // Fase 2: DOWN — agent mati (shutdown mock server)
    agent.reset().await;
    poller.poll_once_all().await.unwrap();
    assert!(poller.host_status("flaky-host").is_offline());

    // Fase 3: UP lagi — mock hidup dengan snapshot baru
    let new_server = MockServer::start().await;
    // update URL host ke server baru
    poller
        .store()
        .register_host("flaky-host", &new_server.uri())
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(2_000)))
        .mount(&new_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backlog"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&new_server)
        .await;
    poller.poll_once_all().await.unwrap();
    assert!(poller.host_status("flaky-host").online);

    // SD-AC-033: timeline lengkap — agent_down DAN agent_up tercatat
    let events = state_store.list_events(Some("flaky-host"), 100).unwrap();
    let kinds: Vec<&str> = events.iter().map(|(_, _, k, _, _, _)| k.as_str()).collect();
    assert!(
        kinds.contains(&"agent_down"),
        "agent_down harus ada; kinds = {kinds:?}"
    );
    assert!(
        kinds.contains(&"agent_up"),
        "agent_up harus ada (Q-SD1: transparan); kinds = {kinds:?}"
    );
    // severity
    for (_, _, kind, severity, _, _) in &events {
        match kind.as_str() {
            "agent_down" => assert_eq!(severity, "warning"),
            "agent_up" => assert_eq!(severity, "info"),
            _ => {}
        }
    }
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn agent_up_not_duplicated_while_online() {
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(ResponseTemplate::new(200).set_body_string(snapshot_json(1_000)))
        .expect(3)
        .mount(&agent)
        .await;

    let (poller_store, _detector_store, _s, db_path) = {
        let path = std::env::temp_dir().join(format!("sd7b-{}.db", std::process::id()));
        (
            Store::open(&path).unwrap(),
            Store::open(&path).unwrap(),
            Store::open(&path).unwrap(),
            path,
        )
    };
    poller_store.register_host("h", &agent.uri()).unwrap();

    let mut poller = Poller::with_detector(
        poller_store,
        "t".into(),
        Duration::from_millis(10),
        Default::default(),
        Store::open(&db_path).unwrap(),
    );

    // 3 poll berturut-turut semua online → TIDAK ada agent_up spam
    poller.poll_once_all().await.unwrap();
    poller.poll_once_all().await.unwrap();
    poller.poll_once_all().await.unwrap();

    let poller_ref = poller.store();
    let events = poller_ref.list_events(Some("h"), 100).unwrap();
    let up_count = events
        .iter()
        .filter(|(_, _, k, _, _, _)| k == "agent_up")
        .count();
    assert_eq!(
        up_count, 0,
        "host online terus → tanpa agent_up; dapat {up_count}"
    );
    let _ = std::fs::remove_file(&db_path);
}
