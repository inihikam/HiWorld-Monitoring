use clap::Parser;

use hiworld_collector::api::{router, AppState, CollectorConfig};
use hiworld_collector::store::Store;

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
        // poller loop (koneksi store terpisah — WAL mendukung multi-koneksi)
        let poller_store = Store::open(&state.config.db_path).expect("store poller");
        let poller = hiworld_collector::poller::Poller::new(
            poller_store,
            state.config.agent_token.clone(),
            std::time::Duration::from_millis(state.config.poll_interval_ms),
        );
        tokio::spawn(poller.run_forever());

        let app = router(state);
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
