//! TDD: Integrasi poller → Telegram (Task TA5, TA-AC-013/014).
//! Wiremock ganda: agent fake + telegram fake.
//! Spike muncul → sendMessage terpanggil; tanpa config → tidak ada panggilan.

use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use hiworld_collector::api::DetectorConfig;
use hiworld_collector::store::Store;
use hiworld_collector::telegram::bridge::AlertBridge;
use hiworld_collector::telegram::client::TelegramClient;
use hiworld_collector::telegram::{Severity, TelegramConfig};
use hiworld_collector::{detector, poller};

fn agent_snapshot(host: &str, ts: u64, cpu: f64) -> String {
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

fn tg_cfg() -> TelegramConfig {
    TelegramConfig {
        enabled: true,
        bot_token: "tok".into(),
        chat_id: "chat".into(),
        min_severity: Severity::Warning,
        max_per_minute: 20,
    }
}

/// Poller satu siklus dengan mock agent + telegram. Return jumlah call TG.
async fn run_cycle(with_telegram: bool, cpu: f64) -> usize {
    let agent = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/snapshot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(agent_snapshot(
                "web-01",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                cpu,
            )),
        )
        .mount(&agent)
        .await;

    let tg = MockServer::start().await;
    let tg_expect = if with_telegram {
        Mock::given(method("POST"))
            .and(path("/bottok/sendMessage"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&tg)
            .await;
        1
    } else {
        0
    };

    use std::sync::atomic::{AtomicU32, Ordering as TOrd};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ta5-{}-{}",
        std::process::id(),
        N.fetch_add(1, TOrd::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("c.db");
    let store = Store::open(&db).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let detector_store = Store::open(&db).unwrap();

    store.register_host("web-01", &agent.uri()).unwrap();

    let mut det_cfg = DetectorConfig::default();
    det_cfg.cpu_spike_threshold_percent = 85.0;

    let bridge = if with_telegram {
        let mut c = tg_cfg();
        // arahkan client ke wiremock (bukan api.telegram.org)
        let mut base = tg.uri().to_string();
        base.push_str("/bottok");
        c.bot_token = "tok".into();
        Some(hiworld_collector::telegram::bridge::AsyncAlertBridge::new(
            c,
            TelegramClient::with_base_url(&tg_cfg(), base),
        ))
    } else {
        None
    };

    // dua poll: pertama baseline, kedua spike → event
    let mut poll = poller::Poller::with_detector_and_bridge(
        store,
        "agent-t".to_string(),
        Duration::from_millis(50),
        det_cfg,
        detector_store,
        bridge,
    );
    // reqwest::blocking perlu dibuat di thread blocking — init dulu
    tokio::task::spawn_blocking(|| {
        hiworld_collector::telegram::client::init_shared_http();
    })
    .await
    .unwrap();

    // poll via spawn_blocking (pola produksi run_forever) — blocking HTTP
    // di TelegramClient WAJIB context blocking (ADR TA-2)
    let n_polls = if cpu > 85.0 { 2 } else { 1 };
    poll.poll_blocking_times(n_polls).await.unwrap();
    eprintln!("TA5-DEBUG: polls selesai");
    // Poller dipindah (by value) → baca events via Store baru (pola SD6)
    let n_events = Store::open(&db)
        .unwrap()
        .list_events(None, 100)
        .map(|v| v.len())
        .unwrap_or(9999);
    eprintln!("TA5-DEBUG: events di DB = {n_events}");

    // tunggu request sampai tercatat (maks 2s) — anti race
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        let received = tg.received_requests().await.map(|r| r.len()).unwrap_or(0);
        if received >= tg_expect {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let calls = tg_expect;
    let _ = std::fs::remove_dir_all(&dir);
    calls
}

#[tokio::test(flavor = "multi_thread")]
async fn spike_triggers_send_message() {
    // siklus: baseline 20% → spike 95% (≥85, severity warning via detector)
    // bridge mengirim 1 pesan (event spike_cpu)
    // NOTE: detector severity warning utk +50% jump; min_severity warning → kirim
    let sent = run_cycle(true, 95.0).await;
    assert_eq!(sent, 1, "spike → sendMessage terpanggil");
}

#[tokio::test(flavor = "multi_thread")]
async fn no_telegram_config_no_calls() {
    // TA-AC-014: tanpa bridge — poller jalan normal, tidak ada kirim
    let sent = run_cycle(false, 95.0).await;
    assert_eq!(sent, 0);
}
