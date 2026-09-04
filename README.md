# hiworld-monitoring

Simple **multi-server monitoring** untuk Ubuntu Server — CPU/RAM/disk dalam
percentage & nominal, **plus per-process attribution**: proses/service apa yang
sedang makan resources (killer feature yang tidak diberikan Grafana+Prometheus
out-of-the-box).

```
┌── Server A ──┐   ┌── Server B ──┐
│ hiworld-agent│   │ hiworld-agent│   ← pasif, baca /proc, ring buffer
└──────┬───────┘   └──────┬───────┘
       │ poll (HTTP pull) │
       └────────┬─────────┘
                ▼
      ┌── hiworld-collector ──┐
      │ SQLite + spike detect │   ← satu titik: store, alert Telegram,
      │ alert + API + web UI  │      realtime WS, dashboard M3
      └───────────────────────┘
```

## Build

```bash
# dev
cargo build

# release (2 binary)
cargo build --release
# → target/release/hiworld-agent
# → target/release/hiworld-collector

# statis musl (deploy ke Ubuntu bersih, tanpa dependency .so)
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Menjalankan

### 1. Agent (di tiap server yang dimonitor)

`agent.toml`:

```toml
[server]
bind_addr = "0.0.0.0"
port = 9100
auth_token = "<generate: openssl rand -hex 32>"

[collector]                      # auto-register (Q1)
url = "http://collector-host:8080"
register_token = "<provisioning token dari collector>"

[sampling]
interval_ms = 10000              # 1000..=60000, bisa diubah runtime (turbo)
top_n_processes = 10
collect_pss = false              # PSS akurat tapi mahal — default off (ADR-3)
```

```bash
./hiworld-agent --config agent.toml
```

Endpoint agent: `GET /snapshot`, `GET /backlog?since=<ts>`,
`GET|POST /config`, `GET /metrics` (Prometheus format) — semua butuh
`Authorization: Bearer <auth_token>`; `GET /health` terbuka.

### 2. Collector (satu server pusat)

`collector.toml`:

```toml
bind_addr = "0.0.0.0"
port = 8080
provisioning_token = "<sama dengan register_token agent>"
bootstrap_admin_user = "admin"
bootstrap_admin_pass = "<initial, WAJIB diganti setelah login pertama (Q4)>"
db_path = "./hiworld.db"
agent_token = "<sama dengan auth_token agent>"
poll_interval_ms = 10000
```

```bash
./hiworld-collector --config collector.toml
```

UI: `http://collector:8080/` — login dengan bootstrap password, lalu ganti
via **Settings → Change password** (tersimpan bcrypt di SQLite).

API utama (butuh session cookie dari `POST /api/login`):
`GET /api/hosts`, `GET /api/history?host=&from=&to=`, `GET /api/events`,
`POST /api/agents/register` (provisioning token, tanpa sesi).

## Instalasi systemd

`/etc/systemd/system/hiworld-agent.service`:

```ini
[Unit]
Description=hiworld-monitoring agent
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/hiworld-agent --config /etc/hiworld/agent.toml
Restart=always
RestartSec=5
User=hiworld
Group=hiworld

# Hardening (agent hanya butuh BACA /proc & jaringan listen)
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6
ReadOnlyPaths=/proc
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
```

`hiworld-collector.service` serupa, tanpa `ReadOnlyPaths=/proc` tapi dengan
`ReadWritePaths=<dir db>` bila `ProtectSystem=strict` dipakai.

```bash
sudo useradd -r -s /usr/sbin/nologin hiworld
sudo systemctl daemon-reload && sudo systemctl enable --now hiworld-agent
```

## Menjalankan test

```bash
cargo test --workspace          # semua test (unit + integration + property)
cargo clippy --workspace -- -D warnings
bash scripts/smoke.sh           # e2e: collector+agent nyata, register+poll
```

## Struktur

```
core/       model data + parser /proc (MURNI, tanpa runtime/IO — dijaga test)
agent/      sampler, ring buffer, HTTP API, /metrics
collector/  store SQLite, poller+backfill, auth bcrypt, REST API
web/        dashboard Svelte 5 + M3 (task berikutnya)
docs/       LOKAL saja (gitignored): specs/, criteria/, plan/ per fitur
scripts/    smoke.sh (e2e)
```

## Keamanan (v1)

- Bearer token statik agent↔collector (shared token dari TOML)
- Password UI bcrypt di SQLite; sesi HttpOnly cookie
- HTTPS di-delegate ke reverse proxy (nginx/caddy) — bukan tanggung jawab binary
- Read-only terhadap host yang dimonitor — tidak ada eksekusi perintah via API

## Roadmap

- [x] Fondasi arsitektur (agent + collector + store + API) — docs/plan/architecture.md
- [ ] Agent self-register (dari `[collector]` config)
- [ ] Spike detection + event log
- [ ] Telegram alert
- [ ] Web dashboard (Svelte 5 + M3 + uPlot + WebSocket)
- [ ] Turbo mode (ubah interval runtime dari UI)

## Referensi desain

Semua keputusan teknis (ADR 1–10) di dokumentasi lokal `docs/specs/architecture.md`
(tidak di-push — lihat `.gitignore`).
