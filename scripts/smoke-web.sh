#!/usr/bin/env bash
# Smoke e2e web-foundation (WD10): collector serve dashboard statis
# → login via API (simulasi browser) → logout.
set -euo pipefail
cd "$(dirname "$0")/.."

PORT=18443
BASE="http://127.0.0.1:$PORT"
DB=$(mktemp -u /tmp/wd10-XXXXXX.db)
CFG=$(mktemp /tmp/wd10-cfg-XXXX.toml)
cat > "$CFG" << TOML
bind_addr = "127.0.0.1"
port = $PORT
db_path = "$DB"
bootstrap_admin_user = "admin"
bootstrap_admin_pass = "wd10-test-pass"
provisioning_token = "wd10-prov"
agent_token = "wd10-agent-t"
poll_interval_ms = 10000
static_dir = "collector/web-dist"
TOML

./target/release/hiworld-collector --config "$CFG" &
COL_PID=$!
trap 'kill $COL_PID 2>/dev/null || true' EXIT

# tunggu health
for i in $(seq 1 30); do
  if curl -sf "$BASE/health" > /dev/null; then break; fi
  sleep 0.2
done
echo "== health: $(curl -s $BASE/health)"

# 1) root → index.html (statik disajikan)
ROOT=$(curl -s "$BASE/")
echo "$ROOT" | grep -q "hiworld monitoring" && echo "OK: root serves index.html" || { echo "FAIL: root"; exit 1; }

# 2) SPA fallback: path bebas → index
SPA=$(curl -s "$BASE/anything/here")
echo "$SPA" | grep -q "hiworld" && echo "OK: SPA fallback" || { echo "FAIL: SPA fallback"; exit 1; }

# 3) API unknown → 404 JSON (tidak tertimpa SPA)
CODE=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/xyz")
[ "$CODE" = "404" ] && echo "OK: /api/xyz -> 404" || { echo "FAIL: api fallback ($CODE)"; exit 1; }

# 4) belum login → /api/hosts 401
CODE=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/hosts")
[ "$CODE" = "401" ] && echo "OK: unauth 401" || { echo "FAIL: unauth ($CODE)"; exit 1; }

# 5) login → cookie
CJ=$(mktemp)
CODE=$(curl -s -c "$CJ" -o /dev/null -w '%{http_code}' -X POST "$BASE/api/login" \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"wd10-test-pass"}')
[ "$CODE" = "200" ] && echo "OK: login 200" || { echo "FAIL: login ($CODE)"; exit 1; }

# 6) dengan cookie → 200
CODE=$(curl -s -b "$CJ" -o /dev/null -w '%{http_code}' "$BASE/api/hosts")
[ "$CODE" = "200" ] && echo "OK: authed /api/hosts 200" || { echo "FAIL: authed ($CODE)"; exit 1; }

# 7) logout → session invalid
curl -s -b "$CJ" -X POST "$BASE/api/logout" > /dev/null
CODE=$(curl -s -b "$CJ" -o /dev/null -w '%{http_code}' "$BASE/api/hosts")
[ "$CODE" = "401" ] && echo "OK: logout invalidates" || { echo "FAIL: logout ($CODE)"; exit 1; }

echo "== SMOKE WEB-FOUNDATION LULUS SEMUA"
