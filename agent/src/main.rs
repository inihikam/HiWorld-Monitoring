use clap::Parser;

use hiworld_agent::api::{router, AppState};
use hiworld_agent::config::AgentConfig;
use hiworld_agent::ring_buffer::RingBuffer;
use hiworld_core::models::Snapshot;

/// hiworld-monitoring agent: samples /proc and serves snapshots.
#[derive(Parser, Debug)]
#[command(name = "hiworld-agent", version)]
struct Args {
    /// Path ke file konfigurasi TOML
    #[arg(long)]
    config: String,
}

fn main() {
    let args = Args::parse();

    let config: AgentConfig = match AgentConfig::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("hiworld-agent: {e}");
            std::process::exit(1);
        }
    };

    let bind = format!("{}:{}", config.server.bind_addr, config.server.port);
    let has_collector = config.collector.is_some();

    let state = std::sync::Arc::new(std::sync::Mutex::new(AppState {
        config: config.clone(),
        latest: None,
        backlog: RingBuffer::new(600, std::time::Duration::from_secs(600)),
        started_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(async move {
        // sampler loop (thread OS persist)
        hiworld_agent::runtime::spawn_sampler_loop(state.clone(), None);

        // registrar (auto-register, Q1) — hanya bila [collector] ada (SR-AC-005)
        if has_collector {
            let registrar = hiworld_agent::runtime::spawn_registrar(&config);
            eprintln!("hiworld-agent: self-register aktif (task latar)");
            std::mem::forget(registrar);
        } else {
            eprintln!("hiworld-agent: tanpa [collector] — mode standalone (tidak register)");
        }

        let app = router(state);
        let listener = tokio::net::TcpListener::bind(&bind)
            .await
            .unwrap_or_else(|e| {
                eprintln!("hiworld-agent: bind {bind} gagal: {e}");
                std::process::exit(1);
            });
        eprintln!("hiworld-agent: listening on {bind}");
        axum::serve(listener, app).await.expect("server error");

        // type witness agar import terpakai
        let _: Option<RingBuffer<Snapshot>> = None;
    });
}
