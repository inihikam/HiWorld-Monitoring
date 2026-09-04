/**
 * Format helpers (WO1, WO-AC-012).
 * fmtBytes: B→TB pintar 1 desimal; fmtPercent; fmtLoad.
 */

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']

export function fmtBytes(n) {
  if (n == null || Number.isNaN(n)) return '—'
  let neg = n < 0
  n = Math.abs(n)
  if (n === 0) return '0 B'
  let i = Math.min(Math.floor(Math.log(n) / Math.log(1024)), UNITS.length - 1)
  const v = n / 1024 ** i
  const s = i === 0 ? v.toFixed(1) : v >= 100 ? v.toFixed(0) : v.toFixed(1)
  return `${neg ? '-' : ''}${s} ${UNITS[i]}`
}

export function fmtPercent(n) {
  if (n == null || Number.isNaN(n)) return '—'
  return `${n.toFixed(1)}%`
}

export function fmtLoad(n) {
  if (n == null || Number.isNaN(n)) return '—'
  return n.toFixed(2)
}

/** Timestamp ms → waktu pendek (HH:MM:SS lokal). */
export function fmtTime(tsMs) {
  if (!tsMs) return '—'
  return new Date(tsMs).toLocaleTimeString([], { hour12: false })
}
