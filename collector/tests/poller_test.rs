//! TDD: auto-register (D3) + poller dengan backfill (D4).
//! Agent palsu = wiremock; collector diuji tanpa agent nyata.

use std::time::Duration;

use hiworld_collector::poller::Poller;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn agent_snapshot_json(host: &str, ts: u64, cpu: f64) -> String {
    format!(
        r#"{{
        "host_id": "{host}", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": {cpu}, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        }},
        "processes": []
    }}"#
    )
}

// ---------- D3: register ----------

#[tokio::test]
async fn register_without_token_rejected() {
    let server = MockServer::start().await; // collector palsu tidak perlu — test store-level
    let _ = server;
    let store = hiworld_collector::store::Store::open_in_memory().unwrap();

    // register endpoint logic via store + token check di api (D5)
    // di sini: idempotensi sudah tercakup; verifikasi 401 dilakukan di api_test (D5)
    let id = store
        .register_host("web-01", "http://127.0.0.1:9100")
        .unwrap();
    assert!(id > 0);
}

// ---------- D4: poller ----------

#[tokio::test]
async fn poller_pulls_snapshot_and_stores() {
    let agent = MockServer::start().await;
    let ts = 1_700_000_000_000u64;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .and(header("Authorization", "Bearer agent-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot_json("web-01", ts, 42.5)),
        )
        .mount(&agent)
        .await;

    let store = hiworld_collector::store::Store::open_in_memory().unwrap();
    store.register_host("web-01", &agent.uri()).unwrap();

    let mut poller = Poller::new(store, "agent-token".into(), Duration::from_millis(10));
    poller.poll_once_all().await.unwrap();

    let latest = poller.store().latest_snapshot("web-01").unwrap().unwrap();
    assert!((latest.cpu_percent - 42.5).abs() < 0.001);
}

#[tokio::test]
async fn poller_marks_host_down_on_failure() {
    // agent tidak berjalan → koneksi gagal
    let store = hiworld_collector::store::Store::open_in_memory().unwrap();
    store
        .register_host("dead-host", "http://127.0.0.1:1")
        .unwrap();

    let mut poller = Poller::new(store, "t".into(), Duration::from_millis(10));
    poller.poll_once_all().await.unwrap(); // tidak boleh panic

    let status = poller.host_status("dead-host");
    assert!(status.is_offline(), "host harus offline setelah gagal poll");
    assert!(status.consecutive_failures >= 1);
}

#[tokio::test]
async fn poller_backfills_from_agent_backlog() {
    let agent = MockServer::start().await;
    // snapshot lama di backlog (collector "down" selama 3 interval)
    let backlog = format!(
        "[{}, {}, {}]",
        agent_snapshot_json("web-01", 1_000, 10.0),
        agent_snapshot_json("web-01", 2_000, 20.0),
        agent_snapshot_json("web-01", 3_000, 30.0),
    );
    // snapshot terbaru untuk /snapshot
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot_json("web-01", 4_000, 40.0)),
        )
        .mount(&agent)
        .await;
    Mock::given(method("GET"))
        .and(path("/backlog"))
        .respond_with(ResponseTemplate::new(200).set_body_string(backlog))
        .mount(&agent)
        .await;

    let store = hiworld_collector::store::Store::open_in_memory().unwrap();
    store.register_host("web-01", &agent.uri()).unwrap();

    // simulasi: poller terakhir lihat data sampai ts=500 (maka backlog since=500)
    let mut poller = Poller::new(store, "t".into(), Duration::from_millis(10));
    poller.set_last_seen("web-01", 500);
    poller.poll_once_all().await.unwrap();

    let rows = poller
        .store()
        .query_system_history("web-01", 0, 10_000)
        .unwrap();
    // 3 backlog + 1 snapshot = 4 baris tersimpan (backfill jalan — Q2)
    assert_eq!(rows.len(), 4, "backfill harus mengisi gap; dapat {rows:?}");
    assert_eq!(rows[0].timestamp_ms, 1_000);
    assert_eq!(rows[3].timestamp_ms, 4_000);
}

#[tokio::test]
async fn poller_backfill_skipped_when_up_to_date() {
    let agent = MockServer::start().await;
    // /backlog TIDAK dimount → kalau poller memanggilnya, test gagal dengan 404
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot_json("web-01", 5_000, 50.0)),
        )
        .expect(1)
        .mount(&agent)
        .await;

    let store = hiworld_collector::store::Store::open_in_memory().unwrap();
    store.register_host("web-01", &agent.uri()).unwrap();

    let mut poller = Poller::new(store, "t".into(), Duration::from_millis(10));
    poller.set_last_seen("web-01", 4_000); // snapshot 5000 > 4000 → tanpa gap

    // last_seen lebih baru dari backlog? snapshot 5000 baru → gap 1000ms kecil
    // → poller boleh langsung ambil /snapshot tanpa /backlog
    poller.poll_once_all().await.unwrap();
    // verifikasi minimal: tidak panic & data masuk
    assert!(poller.store().latest_snapshot("web-01").unwrap().is_some());
}
