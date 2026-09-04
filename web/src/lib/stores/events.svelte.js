/**
 * eventsStore (WE1, WE-AC-001/003/007): timeline event realtime.
 * list: [{host, ts, kind, severity, subject, detail}] — terbaru di atas.
 */

export const eventsStore = $state({
  list: [],
  limit: 100, // ADR WE-3: Load more naikkan limit
  loading: false,
  error: null,
})

/** Parse row API /api/events → item timeline. */
function norm(item) {
  return {
    host: item.host,
    ts: item.timestamp_ms,
    kind: item.kind,
    severity: item.severity,
    subject: item.subject,
    detail: item.detail ?? {},
  }
}

export async function loadEvents(apiFetch) {
  eventsStore.loading = true
  eventsStore.error = null
  try {
    const res = await apiFetch(`/api/events?limit=${eventsStore.limit}`)
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    const rows = await res.json()
    // API terlama dulu? pastikan terbaru di atas
    eventsStore.list = rows.map(norm).sort((a, b) => b.ts - a.ts)
  } catch (e) {
    eventsStore.error = e.message
  } finally {
    eventsStore.loading = false
  }
}

/** Load more: limit naik 100→200→300 (ADR WE-3). */
export function loadMore() {
  eventsStore.limit += 100
}

/** WS event baru → prepend (WE-AC-007) + dedupe by ts+kind+host. */
export function applyWsEvent(ev) {
  const item = norm(ev)
  const dup = eventsStore.list.some(
    (e) => e.ts === item.ts && e.kind === item.kind && e.host === item.host
  )
  if (!dup) eventsStore.list.unshift(item)
}

/** Group per hari (WE-AC-003): [{label, items}] — Today/Yesterday/tanggal. */
export function groupByDay(list, nowMs = Date.now()) {
  const dayStart = (ts) => {
    const d = new Date(ts)
    d.setHours(0, 0, 0, 0)
    return d.getTime()
  }
  const today = dayStart(nowMs)
  const yesterday = today - 86_400_000
  const groups = []
  let cur = null
  const sorted = [...list].sort((a, b) => b.ts - a.ts)
  for (const item of sorted) {
    const ds = dayStart(item.ts)
    const label =
      ds === today ? '__today__' : ds === yesterday ? '__yesterday__' : new Date(ds).toLocaleDateString()
    if (!cur || cur.label !== label) {
      cur = { label, items: [] }
      groups.push(cur)
    }
    cur.items.push(item)
  }
  return groups
}

/**
 * Filter client-side (WE-AC-005/006, ADR WE-2): severity/kind/search.
 * host & range server-side (dilakukan API) — di sini client filter sisanya.
 */
export function filterEvents(list, { severities = [], kinds = [], search = '' } = {}) {
  const q = search.trim().toLowerCase()
  return list.filter((e) => {
    if (severities.length && !severities.includes(e.severity)) return false
    if (kinds.length && !kinds.includes(e.kind)) return false
    if (q) {
      const hay = `${e.subject ?? ''} ${e.host} ${e.kind}`.toLowerCase()
      if (!hay.includes(q)) return false
    }
    return true
  })
}
