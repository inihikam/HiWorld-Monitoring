//! TDD: Integrasi poller → turbo (Task TM4, TM-AC-016..018).
//! E2E wiremock: spike → POST /config interval turbo; normal streak → POST
//! interval normal; tanpa turbo → tidak ada POST.

use std::time::Duration;

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use hiworld_collector::api::DetectorConfig;
use hiworld_collector::store::Store;
use hiworld_collector::turbo::client::AgentConfigClient;
use hiworld_collector::turbo::manager::TurboManager;
use hiworld_collector::turbo::{TurboConfig, TurboHolder};
use hiworld_collector::{poller, turbo};

fn snapshot_json(host: &str, ts: u64, cpu: f64) -> String {
    format!(
        r#"{{
        "host_id": "{host}", "timestamp_ms": {ts}, "interval_ms": 10000,
        "system": {{
            "cpu_percent": 20.0, "cpu_per_core": [], "load_avg": [0,0,0],
            "mem_total_bytes": 1000, "mem_used_bytes": 500, "mem_percent": 50.0,
            "swap_total_bytes": 0, "swap_used_bytes": 0, "disks": [], "net": []
        }},
        "processes": [
            {{"pid": 1234, "comm": "nginx", "cmdline": "nginx: master",
              "user": "www-data", "systemd_unit": "nginx.service",
              "cpu_percent": {cpu}, "mem_rss_bytes": 1000000}}
        ]
    }}"#
    )
}

fn turbo_cfg(interval: u64) -> TurboConfig {
    TurboConfig {
        enabled: true,
        interval_ms: interval,
        min_duration_ms: 0, // test: pop langsung setelah streak 2x
    }
}

/// Jalankan n poll dengan CPU per poll. Return request /config yang diterima.
async fn run_polls(
    cpu_seq: &[f64],
    with_turbo: bool,
    turbo_interval: u64,
) -> Vec<serde_json::Value> {
    use std::sync::atomic::{AtomicU32, Ordering as TOrd};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "tm4-{}-{}",
        std::process::id(),
        N.fetch_add(1, TOrd::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let agent = MockServer::start().await;

    // SATU mock dgn responder dinamis (counter) — mock identik pertama
    // selalu menang, jadi loop-mount TIDAK bergantian (pelajaran TM4).
    let seq: Vec<String> = cpu_seq
        .iter()
        .map(|cpu| {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            snapshot_json("web-01", ts, *cpu)
        })
        .collect();
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seq_c = counter.clone();
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(move |_req: &wiremock::Request| {
            let i = seq_c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let body = seq
                .get(i)
                .cloned()
                .unwrap_or_else(|| seq.last().cloned().unwrap_or_default());
            ResponseTemplate::new(200).set_body_string(body)
        })
        .mount(&agent)
        .await;

    // tanpa expect — verifikasi via received_requests (anti race)
    if with_turbo {
        Mock::given(method("POST"))
            .and(path("/config"))
            .and(header("Authorization", "Bearer agent-t"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&agent)
            .await;
    }

    let agent_url = agent.uri();
    let db = dir.join("c.db");
    let store = Store::open(&db).unwrap();
    let detector_store = Store::open(&db).unwrap();
    store.register_host("web-01", &agent_url).unwrap();

    let mut det_cfg = DetectorConfig::default();
    det_cfg.cpu_spike_threshold_percent = 85.0;

    let turbo = if with_turbo {
        Some(turbo::TurboHolder::new(
            TurboManager::new(turbo_cfg(turbo_interval), turbo::manager::SystemTurboClock),
            AgentConfigClient::new(),
            agent_url.clone(),
            "agent-t".to_string(),
            turbo_interval,
        ))
    } else {
        None
    };

    let mut poll = poller::Poller::with_turbo(
        store,
        "agent-t".to_string(),
        Duration::from_millis(50),
        det_cfg,
        detector_store,
        turbo,
    );

    for _ in cpu_seq {
        poll.poll_once_all().await.unwrap();
    }

    // drop scoped mock → verifikasi expect; kumpulkan request
    let received = agent.received_requests().await.unwrap_or_default();
    let cfg_reqs: Vec<serde_json::Value> = received
        .into_iter()
        .filter(|r| r.url.path() == "/config")
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect();

    let _ = std::fs::remove_dir_all(&dir);
    cfg_reqs
}

#[tokio::test(flavor = "multi_thread")]
async fn spike_pushes_turbo_config() {
    // CPU 20 (baseline) → 95 (spike) → poller kirim POST interval turbo
    let reqs = run_polls(&[20.0, 95.0], true, 1000).await;
    assert!(
        reqs.iter().any(|r| r["interval_ms"] == 1000),
        "POST interval turbo 1000 harus ada, dapat: {reqs:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn no_turbo_no_config_post() {
    let reqs = run_polls(&[20.0, 95.0], false, 1000).await;
    assert!(reqs.is_empty(), "tanpa turbo → tidak ada POST /config");
}

#[tokio::test(flavor = "multi_thread")]
async fn full_cycle_push_and_pop() {
    // 20 (ok) → 95 (spike: push) → 20 (normal streak 1) → 20 (streak 2: pop)
    let reqs = run_polls(&[20.0, 95.0, 20.0, 20.0], true, 1000).await;
    assert!(
        reqs.iter().any(|r| r["interval_ms"] == 1000),
        "push turbo: {reqs:?}"
    );
    assert!(
        reqs.iter().any(|r| r["interval_ms"] == 10_000),
        "pop kembali interval normal 10000 (interval poller): {reqs:?}"
    );
}
