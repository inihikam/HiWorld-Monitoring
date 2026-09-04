use clap::Parser;

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

    // Config collector dibangun di task D1; untuk sekarang cukup validasi
    // bahwa file ada & terbaca (AC ARCH-AC-003: error jelas bila tidak valid).
    match std::fs::read_to_string(&args.config) {
        Ok(_) => {
            eprintln!("hiworld-collector: config file ditemukan (runtime penuh di task D1+)");
        }
        Err(e) => {
            eprintln!(
                "hiworld-collector: config tidak dapat dibaca: {}: {e}",
                args.config
            );
            std::process::exit(1);
        }
    }
}
