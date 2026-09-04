# hiworld-monitoring

> Simple Rust server monitoring agent — CPU/RAM/disk metrics + per-process
> attribution ("proses apa yang bikin resources terpakai?").
>
> Workspace brainstorming. Catatan lengkap: /root/ide-project/rust-server-monitoring.md

## Status

🧠 **Brainstorming selesai** — keputusan arsitektur & stack terkunci (lihat /root/ide-project/rust-server-monitoring.md). Siap scaffold.

### Keputusan final (ringkas)
- **Arsitektur:** multi-server — `hiworld-agent` (pasif, expose JSON+/metrics, ring buffer) + `hiworld-collector` (poll, SQLite, spike detection, alert Telegram, serve UI)
- **Repo:** Cargo workspace — `agent/`, `collector/`, `core/` (parser /proc & model bersama), `web/`
- **Frontend:** Svelte 5 + Vite, WebSocket realtime, Material Design 3 via token CSS manual (@material/web sedang maintenance mode — hindari), chart pakai uPlot
- **Interval sampling:** flexible via TOML (1000–60000 ms) + turbo mode dinamis dari collector; downsample di jalur penyimpanan

## Goal

- System metrics: CPU, RAM, disk, network (percentage + nominal)
- Per-process: top-N consumers, map ke systemd unit, event log untuk spike
- Kompatibel Prometheus (/metrics) tapi bisa berdiri sendiri
- Single binary, lightweight, untuk Ubuntu Server

## Roadmap draft

- [x] Brainstorming scope & stack
- [x] Finalisasi arsitektur (sprint 0) — multi-server agent+collector, stack terkunci
- [ ] Scaffold project Rust (cargo init)
- [ ] MVP: system metrics → CLI output
- [ ] Per-process monitoring + delta jiffies
- [ ] HTTP endpoint (axum) + /metrics
- [ ] Spike detection + event log
- [ ] Telegram alert
- [ ] Web UI (optional)
