use clap::Parser;

use hiworld_collector::api::{AppState, CollectorConfig};
use hiworld_collector::store::Store;
use hiworld_collector::ws;

/// hiworld-monitoring collector: polls agents, stores metrics, serves UI.
#[derive(Parser, Debug)]
#[command(name = "hiworld-collector", version)]
struct Args {
    /// Path ke file konfigurasi TOML
    #[arg(long)]
    config: String,
}

fn main() {
    let args = Args::parse();

    let raw = match std::fs::read_to_string(&args.config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "hiworld-collector: config tidak dapat dibaca: {}: {e}",
                args.config
            );
            std::process::exit(1);
        }
    };

    let config: CollectorConfig = match toml::from_str(&raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("hiworld-collector: config parse gagal: {e}");
            std::process::exit(1);
        }
    };

    let store = match Store::open(&config.db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hiworld-collector: DB gagal dibuka: {e}");
            std::process::exit(1);
        }
    };

    let state = match AppState::bootstrap(store, config.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hiworld-collector: bootstrap gagal: {e}");
            std::process::exit(1);
        }
    };

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    rt.block_on(async move {
        // Poller PRODUKSI: detector + telegram + turbo + broadcast hub.
        // (Bug deploy: sebelumnya Poller::new — semua fitur realtime mati.)
        let poller_store = Store::open(&state.config.db_path).expect("store poller");
        let detector_store = Store::open(&state.config.db_path).expect("store detector");

        // Telegram bridge (TA5) — bila [telegram] configured
        let telegram = if config.telegram.is_configured() {
            Some(hiworld_collector::telegram::bridge::AsyncAlertBridge::new(
                config.telegram.clone(),
                hiworld_collector::telegram::client::TelegramClient::new(&config.telegram),
            ))
        } else {
            None
        };

        // Turbo (TM4): butuh agent_url yang baru diketahui setelah register.
        // v1: turbo diset on saat host pertama terlihat (poller-side TODO v2),
        // di sini None agar kompatibel — turbo tetap bisa dinyalakan v2.
        let turbo: Option<hiworld_collector::turbo::TurboHolder> = None;

        // hub SAMA dengan yang dipakai WS klien (dari AppState)
        let poller = hiworld_collector::poller::Poller::with_everything(
            poller_store,
            state.config.agent_token.clone(),
            std::time::Duration::from_millis(state.config.poll_interval_ms),
            config.detector.clone(),
            detector_store,
            state.hub.clone(),
            telegram,
            turbo,
        );
        tokio::spawn(poller.run_forever());

        // WD2/WD5: static serving + SPA fallback + mirror task realtime
        ws::spawn_mirror_task(state.clone());
        let app = hiworld_collector::api::router_with_static(state, &config.static_dir);
        let addr = format!("{}:{}", config.bind_addr, config.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .unwrap_or_else(|e| {
                eprintln!("hiworld-collector: bind {addr} gagal: {e}");
                std::process::exit(1);
            });
        eprintln!("hiworld-collector: listening on {addr}");
        axum::serve(listener, app).await.expect("server error");
    });
}
