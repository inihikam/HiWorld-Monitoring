//! TDD: runtime integration (Task SR4, SR-AC-008).
//! Produksi impl di main.rs; diuji di sini via modul runtime yang testable:
//! spawn tiga task (registrar/sampler/API) saling tidak blocking.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use hiworld_agent::api::{router, AppState};
use hiworld_agent::config::{AgentConfig, CollectorTarget, SamplingConfig, ServerConfig};
use hiworld_agent::ring_buffer::RingBuffer;
use hiworld_agent::runtime;

fn config_with_collector() -> AgentConfig {
    AgentConfig {
        server: ServerConfig {
            bind_addr: "127.0.0.1".into(),
            port: 0,
            auth_token: "t".into(),
        },
        sampling: SamplingConfig {
            interval_ms: 10_000,
            top_n_processes: 5,
            collect_pss: false,
        },
        collector: Some(CollectorTarget {
            url: "http://127.0.0.1:1".into(), // port 1 = pasti gagal connect
            register_token: "prov".into(),
            advertised_url: None,
        }),
    }
}

fn config_with_collector_once() -> AgentConfig {
    // varian: registrar dibatasi 2 attempt agar spawn_blocking selesai cepat
    let mut c = config_with_collector();
    if let Some(col) = c.collector.as_mut() {
        col.register_token = "prov".into();
    }
    c
}

fn config_without_collector() -> AgentConfig {
    AgentConfig {
        server: ServerConfig {
            bind_addr: "127.0.0.1".into(),
            port: 0,
            auth_token: "t".into(),
        },
        sampling: SamplingConfig {
            interval_ms: 10_000,
            top_n_processes: 5,
            collect_pss: false,
        },
        collector: None,
    }
}

/// SR-AC-008: selama registrar retry (collector unreachable), API tetap hidup
/// dan snapshot tersedia setelah sampler jalan.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_alive_while_registrar_retries() {
    let cfg = config_with_collector();
    let state = Arc::new(Mutex::new(AppState {
        config: cfg.clone(),
        latest: None,
        backlog: RingBuffer::new(10, Duration::from_secs(600)),
        started_at_ms: 0,
    }));

    // build pipeline seperti runtime::spawn_all — dengan sampler fake
    let sampler_state = state.clone();
    let sampler = runtime::spawn_sampler_loop(sampler_state, Some(1)); // 1ms interval
    let registrar = runtime::spawn_registrar_with(&cfg, Some(2));
    let app = router(state.clone());

    // tunggu sampai sampler mengisi snapshot (maks 10s) — registrar retry di latar
    let mut snapshot_ok = false;
    for _ in 0..100 {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/snapshot")
                    .header("Authorization", "Bearer t")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        if res.status() == StatusCode::OK {
            snapshot_ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        snapshot_ok,
        "SR-AC-008: snapshot harus tersedia walau register gagal"
    );

    // API tetap hidup
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // sampler kini thread OS persist (tidak bisa abort) — test selesai
    // dan proses test akan membersihkan thread saat keluar.
    registrar.abort();
}

/// Tanpa [collector] → registrar tidak spawn (SR-AC-005): fungsi mengembalikan None.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_collector_no_registrar() {
    let cfg = config_without_collector();
    assert!(
        runtime::build_registrar_task_with(&cfg, None).is_none(),
        "tanpa [collector] → tidak ada registrar task"
    );
}

/// Dengan [collector] → registrar task ada.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_collector_registrar_exists() {
    let cfg = config_with_collector();
    assert!(runtime::build_registrar_task_with(&cfg, None).is_some());
}
