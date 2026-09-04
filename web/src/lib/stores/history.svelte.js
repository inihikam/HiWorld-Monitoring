/**
 * historyStore (WH1, WH-AC-003): time-series per host dari /api/history.
 * series: { cpu: [{ts, v}], mem: [...], disks: {mount: [...]}, load: [...] }
 * NaN → null (gap di chart, WH-AC-005).
 */

export const historyStore = $state({
  host: null,
  rangeMs: 3600_000, // default 1h (ADR WH-5)
  series: { cpu: [], mem: [], disks: {}, load: [] },
  loading: false,
  error: null,
})

/** Parse response /api/history → series dgn null utk NaN. */
export function parseHistory(rows) {
  const cpu = []
  const mem = []
  const load = []
  const disks = {}
  const clean = (v) => (v == null || Number.isNaN(v) ? null : v)
  for (const r of rows ?? []) {
    const ts = r.timestamp_ms
    cpu.push([ts, clean(r.cpu_percent)])
    mem.push([ts, clean(r.mem_percent)])
    load.push([ts, clean(r.load_1m)])
    for (const d of r.disks ?? []) {
      ;(disks[d.mount] ??= []).push([ts, clean(d.used_percent)])
    }
  }
  return { cpu, mem, load, disks }
}

export async function loadHistory(apiFetch, host, rangeMs = historyStore.rangeMs) {
  historyStore.host = host
  historyStore.rangeMs = rangeMs
  historyStore.loading = true
  historyStore.error = null
  try {
    const now = Date.now()
    const res = await apiFetch(
      `/api/history?host=${encodeURIComponent(host)}&from=${now - rangeMs}&to=${now}`
    )
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    const rows = await res.json()
    historyStore.series = parseHistory(rows)
  } catch (e) {
    historyStore.error = e.message
  } finally {
    historyStore.loading = false
  }
}

/** WS snapshot host ini → append titik (WH-AC-004, tanpa re-fetch). */
export function appendSnapshot(snap) {
  if (historyStore.host !== snap.host_id) return
  const ts = snap.timestamp_ms
  historyStore.series.cpu.push([ts, snap.system?.cpu_percent ?? null])
  historyStore.series.mem.push([ts, snap.system?.mem_percent ?? null])
  historyStore.series.load.push([ts, snap.system?.load_avg?.[0] ?? null])
  for (const d of snap.system?.disks ?? []) {
    ;(historyStore.series.disks[d.mount] ??= []).push([ts, d.used_percent ?? null])
  }
}
