use clap::Parser;

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

    match hiworld_agent::config::AgentConfig::load(&args.config) {
        Ok(cfg) => {
            eprintln!(
                "hiworld-agent: config OK (bind {}:{}, interval {}ms)",
                cfg.server.bind_addr, cfg.server.port, cfg.sampling.interval_ms
            );
            // Runtime penuh (sampler + API) dibangun di task C1/C2.
        }
        Err(e) => {
            eprintln!("hiworld-agent: {e}");
            std::process::exit(1);
        }
    }
}
