#!/usr/bin/env bash
# Smoke test e2e hiworld-monitoring (Task D6, ARCH-AC-032):
# 1. build release binary
# 2. start collector + 1 agent (config fixture)
# 3. agent auto-register → collector poll ≥1 snapshot
# 4. /api/hosts menampilkan host online
# Exit 0 = lulus.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$WORK"' EXIT


PORT_COLLECTOR=18080
PORT_AGENT=19100

echo "==> build release"
( cd "$ROOT" && cargo build --release --quiet )

echo "==> siapkan config"
cat > "$WORK/agent.toml" <<EOF
[server]
bind_addr = "127.0.0.1"
port = $PORT_AGENT
auth_token = "smoke-agent-token"

[collector]
url = "http://127.0.0.1:$PORT_COLLECTOR"
register_token = "smoke-prov-token"

[sampling]
interval_ms = 1000
EOF

cat > "$WORK/collector.toml" <<EOF
bind_addr = "127.0.0.1"
port = $PORT_COLLECTOR
provisioning_token = "smoke-prov-token"
bootstrap_admin_user = "admin"
bootstrap_admin_pass = "smoke-pass-123"
db_path = "$WORK/smoke.db"
agent_token = "smoke-agent-token"
poll_interval_ms = 1000
EOF

BIN="${HIWORLD_BIN:-$ROOT/target/release}"

echo "==> start collector"
"$BIN/hiworld-collector" --config "$WORK/collector.toml" &
COL_PID=$!

echo "==> start agent"
"$BIN/hiworld-agent" --config "$WORK/agent.toml" &
AGT_PID=$!

cleanup() { kill "$COL_PID" "$AGT_PID" 2>/dev/null || true; }
trap cleanup EXIT

echo "==> tunggu collector siap"
for i in $(seq 1 20); do
  curl -fsS "http://127.0.0.1:$PORT_COLLECTOR/health" > /dev/null 2>&1 && break
  sleep 0.5
done

# NOTE (SR5): TIDAK ada curl register manual — agent self-register sendiri
# dari [collector] config (Q1). Jika host tidak muncul = bug agent.

echo "==> tunggu poll (max 15s)"
for i in $(seq 1 30); do
  # login dan cek hosts
  if curl -fsS "http://127.0.0.1:$PORT_COLLECTOR/health" > /dev/null 2>&1; then
    COOKIE=$(curl -fsS -D- -o /dev/null \
      -H 'Content-Type: application/json' \
      -d '{"username":"admin","password":"smoke-pass-123"}' \
      "http://127.0.0.1:$PORT_COLLECTOR/api/login" 2>/dev/null | grep -i '^set-cookie' | cut -d' ' -f2 | cut -d';' -f1 || true)
    if [ -n "${COOKIE:-}" ]; then
      HOSTS=$(curl -fsS -H "Cookie: $COOKIE" "http://127.0.0.1:$PORT_COLLECTOR/api/hosts" 2>/dev/null || echo "[]")
      if echo "$HOSTS" | grep -q '"host_id"'; then
        echo "==> OK: agent self-register BERHASIL — host muncul di /api/hosts"
        echo "    hosts = $HOSTS"
        exit 0
      fi
    fi
  fi
  sleep 0.5
done

echo "GAGAL: host tidak terlihat dalam batas waktu" >&2
exit 1
