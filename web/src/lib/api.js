/**
 * API wrapper (WD8, WD-AC-010/011).
 * - credentials: 'same-origin' (cookie session otomatis)
 * - 401 → redirect #/login (kecuali pada request login itu sendiri)
 */

let onUnauthorized = null

/** Daftarkan callback 401 global (dipanggil App init). */
export function setUnauthorizedHandler(fn) {
  onUnauthorized = fn
}

export async function api(path, options = {}) {
  const res = await fetch(path, {
    credentials: 'same-origin',
    headers: options.body ? { 'Content-Type': 'application/json' } : {},
    ...options,
  })
  if (res.status === 401 && onUnauthorized && !path.includes('/api/login')) {
    onUnauthorized()
  }
  return res
}

export async function apiJson(path, options = {}) {
  const res = await api(path, options)
  if (!res.ok) {
    const err = new Error(`HTTP ${res.status}`)
    err.status = res.status
    throw err
  }
  return res.json()
}

/** Probe sesi: true bila sudah login. */
export async function checkAuth() {
  try {
    const res = await api('/api/hosts')
    return res.ok
  } catch {
    return false
  }
}

export async function login(username, password) {
  const res = await api('/api/login', {
    method: 'POST',
    body: JSON.stringify({ username, password }),
  })
  return res.ok
}

export async function logout() {
  await api('/api/logout', { method: 'POST' })
}
