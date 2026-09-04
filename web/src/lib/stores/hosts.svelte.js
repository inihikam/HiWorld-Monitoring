/**
 * hostsStore (WO1): state host realtime + sorting bermasalah-di-atas.
 * byHost: { [host_id]: { latest, online, lastSeenMs } }
 */

export const hostsStore = $state({ byHost: {} })

/** hello bootstrap: { hosts: [{host_id, latest|null}] } */
export function applyHello(data) {
  const next = {}
  for (const h of data.hosts ?? []) {
    next[h.host_id] = {
      latest: h.latest ?? null,
      online: h.latest != null,
      lastSeenMs: h.latest?.timestamp_ms ?? null,
    }
  }
  hostsStore.byHost = next
}

/** WS snapshot → update satu host (+lastSeen). */
export function applySnapshot(snap) {
  const cur = hostsStore.byHost[snap.host_id] ?? { latest: null, online: false, lastSeenMs: null }
  hostsStore.byHost[snap.host_id] = {
    ...cur,
    latest: snap,
    online: true,
    lastSeenMs: Date.now(),
  }
}

/** WS host_status {host_id, online} → update badge (data dibekukan). */
export function applyStatus(st) {
  const cur = hostsStore.byHost[st.host_id]
  if (cur) {
    hostsStore.byHost[st.host_id] = { ...cur, online: !!st.online }
  }
}

/**
 * Q-WO1 (keputusan Sena): host bermasalah naik ke atas.
 * Urutan: offline (terlama tanpa data lebih atas) → stale → sehat (terbaru dulu).
 */
export function sortedHosts(byHost) {
  const rank = (h) => (!h.online ? 0 : isStale(h) ? 1 : 2)
  return Object.entries(byHost)
    .map(([host_id, h]) => ({ host_id, ...h }))
    .sort((a, b) => {
      const r = rank(a) - rank(b)
      if (r !== 0) return r
      // dalam kelas sama: offline → yang lastSeen paling tua dulu;
      // sehat/stale → nama host alfabetis (stabil)
      if (rank(a) === 0) {
        return (a.lastSeenMs ?? 0) - (b.lastSeenMs ?? 0)
      }
      return a.host_id.localeCompare(b.host_id)
    })
}

/** Stale: online tapi snapshot > 60s lalu (WO-AC-007). */
export function isStale(h, nowMs = Date.now()) {
  if (!h.online || !h.lastSeenMs) return false
  return nowMs - h.lastSeenMs > 60_000
}

/** Ring summary: agregat host online saja (ADR WO-4). */
export function summary(byHost) {
  const all = Object.values(byHost)
  const online = all.filter((h) => h.online)
  const avg = (fn) => {
    const vals = online.map(fn).filter((v) => v != null && !Number.isNaN(v))
    if (!vals.length) return null
    return vals.reduce((a, b) => a + b, 0) / vals.length
  }
  return {
    online: online.length,
    total: all.length,
    avgCpu: avg((h) => h.latest?.system?.cpu_percent),
    avgRam: avg((h) => h.latest?.system?.mem_percent),
  }
}
