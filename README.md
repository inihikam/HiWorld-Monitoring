# hiworld-monitoring

Simple, self-hosted **multi-server monitoring** for Ubuntu Server — CPU/RAM/disk in percentage & nominal, **plus per-process attribution**: see *which process or service* is eating your resources (the killer feature Grafana+Prometheus don't give you out-of-the-box).

```
┌── Server A ──┐   ┌── Server B ──┐
│ hiworld-agent│   │ hiworld-agent│   ← passive, reads /proc, ring buffer
└──────┬───────┘   └──────┬───────┘
       │ poll (HTTP pull) │
       └────────┬─────────┘
                ▼
      ┌── hiworld-collector ──┐
      │ SQLite + spike detect │   ← single point: store, alerts,
      │ alert + API + web UI  │      realtime WS, Material 3 dashboard
      └───────────────────────┘
```

## Features

- **Per-process attribution** — on the dashboard, sort processes by CPU/RAM and instantly see the culprit (PID, systemd unit, RSS) of any resource spike
- **Spike detection** — CPU/memory/disk spikes are detected on the collector and logged as events; repeated spikes don't spam (dedup with persistent active-state)
- **Realtime web dashboard** — Material Design 3, dark & light theme, English/Bahasa Indonesia, responsive (sidebar on desktop, bottom-nav on mobile). Hosts with problems automatically float to the top
- **Time-series charts** — uPlot charts (CPU/RAM/disk per mount) with live updates over WebSocket; historical ranges 15m/1h/6h/24h
- **Telegram alerts** — spikes and agent up/down pushed to your Telegram chat (severity filter, rate-limited, retried)
- **Turbo mode** — when a spike hits, the collector automatically raises that agent's sampling rate to 1s for granular attribution, then restores it when calm
- **Single binary, zero heavy deps** — collector serves the dashboard and stores data in SQLite; agents are pull-only (read-only on your servers)

## Quick start

### 1. Build

```bash
git clone https://github.com/<you>/hiworld-monitoring.git
cd hiworld-monitoring
cargo build --release
# → target/release/hiworld-agent
# → target/release/hiworld-collector

# or fully static (deploy to a clean Ubuntu without any shared libs):
rustup target add x86_64-unknown-linux-musl
apt install musl-tools
cargo build --release --target x86_64-unknown-linux-musl
```

### 2. Dashboard (optional, but nice)

```bash
cd web
npm install
npm run deploy     # builds and copies to collector/web-dist/
```

The collector serves the dashboard automatically when `static_dir` is set (see config below). No web server needed.

### 3. Run the collector (central server)

`collector.toml`:

```toml
bind_addr = "0.0.0.0"
port = 8080
provisioning_token = "<generate: openssl rand -hex 32>"   # agents use this to register
bootstrap_admin_user = "admin"
bootstrap_admin_pass = "<initial — CHANGE IT after first login>"
db_path = "./hiworld.db"
agent_token = "<generate: openssl rand -hex 32>"          # shared secret agent↔collector
poll_interval_ms = 10000                                   # how often to poll agents
static_dir = "collector/web-dist"                          # serve the dashboard

# optional: Telegram alerts
[telegram]
enabled = false
bot_token = ""            # from @BotFather
chat_id = ""              # your chat or group id
min_severity = "warning"  # info | warning | critical
max_per_minute = 20

# optional: adaptive sampling on spikes
[turbo]
enabled = false
interval_ms = 1000        # sampling rate during a spike
min_duration_ms = 60000   # minimum turbo duration before restoring
```

```bash
./hiworld-collector --config collector.toml
```

Open `http://collector-host:8080/`, log in with the bootstrap credentials, then change the password via **Settings → Change password** (stored bcrypt-hashed in SQLite).

### 4. Run an agent (on every server you want to monitor)

`agent.toml`:

```toml
[server]
bind_addr = "0.0.0.0"
port = 9100
auth_token = "<same as collector's agent_token>"

[collector]                          # auto-register — the host appears on the dashboard by itself
url = "http://collector-host:8080"
register_token = "<same as collector's provisioning_token>"

[sampling]
interval_ms = 10000                  # 1000..=60000, can change at runtime (turbo mode)
top_n_processes = 10
collect_pss = false                  # PSS is more accurate but expensive — default off
```

```bash
./hiworld-agent --config agent.toml
```

That's it — the agent registers itself with the collector and your server appears on the Overview page within one poll cycle.

## systemd

Hardened units are included in `scripts/` (ProtectSystem=strict, NoNewPrivileges, MemoryDenyWriteExecute, etc.):

```bash
sudo useradd -r -s /usr/sbin/nologin hiworld
sudo cp target/x86_64-unknown-linux-musl/release/hiworld-* /usr/local/bin/
sudo cp scripts/hiworld-*.service /etc/systemd/system/
sudo mkdir -p /etc/hiworld /var/lib/hiworld
# copy & edit your TOML files into /etc/hiworld/
sudo systemctl daemon-reload
sudo systemctl enable --now hiworld-agent hiworld-collector
```

## Using the dashboard

| Page | What you get |
|------|--------------|
| **Overview** | All hosts as cards: status badge, CPU/RAM/disk bars with red threshold coloring, load. Ring summary on top. Problem hosts (offline/stale/spiking) float to the top automatically. Click a card → host detail |
| **Host detail** | Time-series charts (CPU/RAM/disk per mount), range selector 15m–24h, live chart updates via WebSocket, network totals, and the **process table** (PID, systemd unit, CPU %, RAM %, RSS) — sorted by CPU by default |
| **Events** | Timeline of spikes and agent up/down with severity icons, filters (host / severity / kind / text), and click-to-expand details with a "View host" link that opens the chart around the event |
| **Settings** | Change password |

Theme (dark/light) and language (EN/ID) switches are in the top bar — both persist.

## Configuration reference

### Agent (`agent.toml`)

| Key | Default | Notes |
|-----|---------|-------|
| `server.port` | 9100 | agent HTTP API |
| `server.auth_token` | — | shared secret, must match collector's `agent_token` |
| `collector.url` | — | collector base URL |
| `collector.register_token` | — | must match collector's `provisioning_token` |
| `sampling.interval_ms` | 10000 | 1000–60000, clamped |
| `sampling.top_n_processes` | 10 | processes reported per snapshot |
| `sampling.collect_pss` | false | PSS (more accurate memory) at extra cost |

### Collector (`collector.toml`)

| Key | Default | Notes |
|-----|---------|-------|
| `port` | 8080 | UI + API + agent endpoints |
| `poll_interval_ms` | 10000 | normal polling frequency |
| `static_dir` | — | directory containing the built dashboard |
| `[telegram]` | disabled | see Quick start; messages are plain text, retried 3× |
| `[turbo]` | disabled | spike → push `interval_ms` (1s) to the agent; restore after calm period (≥2 normal snapshots + min duration) |

## FAQ

**Does the agent do anything invasive on my server?**
No. It only *reads* `/proc` and exposes the data over HTTP. There is no command execution via the API.

**What happens if the collector is down?**
Agents keep serving their latest snapshot; the collector pulls a backlog (`/backlog?since=...`) when it comes back, so gaps are backfilled.

**What happens if an agent goes down?**
The collector marks it offline, logs `agent_down` (+ Telegram if enabled), and `agent_up` when it returns. Dashboard cards freeze the last data with a red badge.

**HTTPS?**
Terminate TLS at your reverse proxy (nginx/Caddy) pointing at the collector. Agent↔collector traffic is authenticated with a bearer token.

**Storage growth?**
Metrics live in SQLite (WAL). Rollups and retention are on the roadmap (see below).

## Security model (v1)

- Bearer token (shared secret) for all agent↔collector traffic; provisioning token for self-registration
- Dashboard login: bcrypt password in SQLite, HttpOnly session cookie; WebSocket requires the same session
- Read-only toward monitored hosts
- Deploy behind a reverse proxy for TLS

## Roadmap

- [x] Core: agent, collector, SQLite store, REST API
- [x] Agent self-registration
- [x] Spike detection + event log (per-process attribution)
- [x] WebSocket realtime + web dashboard (Material 3)
- [x] Telegram alerts
- [x] Turbo mode (adaptive sampling)
- [ ] Data retention & downsampling config
- [ ] Event acknowledge + CSV export
- [ ] Editable detector thresholds from the UI
- [ ] Network rate charts

## Development

```bash
cargo test --workspace          # all tests (unit + integration + property)
cargo clippy --workspace -- -D warnings
bash scripts/smoke.sh           # e2e: real collector+agent, register+poll
cd web && npm test              # dashboard tests
```

Repo layout:

```
core/       data model + /proc parser (pure, no runtime/IO — enforced by tests)
agent/      sampler, ring buffer, HTTP API, /metrics (Prometheus format)
collector/  SQLite store, poller+backfill, bcrypt auth, REST API, WS, alerts, turbo
web/        Svelte 5 dashboard (Material 3 tokens, bilingual)
scripts/    smoke.sh + systemd units
```

## License

MIT — see [LICENSE](LICENSE).

---

Built with Rust + Svelte. If this saves you a Grafana setup or shows you *which process* ate your RAM at 3 AM, a ⭐ is appreciated.
