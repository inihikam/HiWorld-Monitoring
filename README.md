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

## Status

**Fondasi arsitektur: SELESAI & TERVERIFIKASI** (2026-09-03)
**Agent self-register: SELESAI & VERIFIED e2e** (2026-09-03)
- 139 test hijau, clippy `-D warnings` bersih
- Smoke e2e: agent mendaftar sendiri ke collector, host muncul otomatis
- Spike detection aktif: CPU/mem/disk, dedup persisten, timeline agent up/down
- WebSocket /ws realtime: snapshot/event/host_status broadcast + hello bootstrap
- Retrospective: `docs/plan/*-retrospective.md` (lokal)

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
apt install musl-tools   # ring/rustls butuh musl-gcc
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

Unit hardened tersedia di `scripts/hiworld-agent.service` dan
`scripts/hiworld-collector.service` (ProtectSystem=strict, NoNewPrivileges,
MemoryDenyWriteExecute, dll — lihat PDD §10).

```bash
sudo useradd -r -s /usr/sbin/nologin hiworld
sudo cp target/x86_64-unknown-linux-musl/release/hiworld-* /usr/local/bin/
sudo cp scripts/hiworld-*.service /etc/systemd/system/
sudo mkdir -p /etc/hiworld /var/lib/hiworld
# salin & edit TOML ke /etc/hiworld/
sudo systemctl daemon-reload && sudo systemctl enable --now hiworld-agent hiworld-collector
```

## Menjalankan test

```bash
cargo test --workspace          # semua test (unit + integration + property)
cargo clippy --workspace -- -D warnings
bash scripts/smoke.sh           # e2e: collector+agent nyata, register+poll
# HIWORLD_BIN=/path/to/bin bash scripts/smoke.sh   # pakai binary kustom
```

## Struktur

```
core/       model data + parser /proc (MURNI, tanpa runtime/IO — dijaga test)
agent/      sampler, ring buffer, HTTP API, /metrics
collector/  store SQLite, poller+backfill, auth bcrypt, REST API
web/        dashboard Svelte 5 + M3 (task berikutnya)
docs/       LOKAL saja (gitignored): specs/, criteria/, plan/ per fitur
scripts/    smoke.sh + systemd units
```

## Keamanan (v1)

- Bearer token statik agent↔collector (shared token dari TOML)
- Password UI bcrypt di SQLite; sesi HttpOnly cookie
- HTTPS di-delegate ke reverse proxy (nginx/caddy) — bukan tanggung jawab binary
- Read-only terhadap host yang dimonitor — tidak ada eksekusi perintah via API

## Roadmap

- [x] Fondasi arsitektur (agent + collector + store + API) — 82 test, e2e verified
- [x] Agent self-register (dari `[collector]` config) — verified e2e
- [x] Spike detection + event log (attribution: proses apa penyebab spike)
- [ ] Telegram alert
- [x] Web foundation: dashboard Svelte 5 (M3 dark/light, bilingual EN/ID,
  sidebar/bottom-nav adaptif, login, WS realtime) — build: `cd web && npm
  run deploy`, disajikan collector dari `static_dir = "collector/web-dist"`
- [ ] Turbo mode (ubah interval runtime dari UI)

## Referensi desain

Semua keputusan teknis (ADR 1–10) di dokumentasi lokal `docs/specs/architecture.md`
(tidak di-push — lihat `.gitignore`).
