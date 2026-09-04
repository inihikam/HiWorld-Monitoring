import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'
import { api, apiJson, setUnauthorizedHandler, checkAuth } from '../src/lib/api.js'

describe('api.js (WD8)', () => {
  beforeEach(() => {
    vi.unstubAllGlobals()
    setUnauthorizedHandler(null)
  })

  it('api() kirim credentials same-origin', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ status: 200, ok: true })
    vi.stubGlobal('fetch', fetchMock)
    await api('/api/hosts')
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/hosts',
      expect.objectContaining({ credentials: 'same-origin' })
    )
  })

  it('401 → callback unauthorized terpanggil (WD-AC-011)', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 401, ok: false }))
    let kicked = false
    setUnauthorizedHandler(() => (kicked = true))
    await api('/api/hosts')
    expect(kicked).toBe(true)
  })

  it('401 pada /api/login TIDAK memicu callback (biar halaman login proses)', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 401, ok: false }))
    let kicked = false
    setUnauthorizedHandler(() => (kicked = true))
    await api('/api/login', { method: 'POST' })
    expect(kicked).toBe(false)
  })

  it('apiJson throw bila !ok dengan status terlampir', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 500, ok: false }))
    await expect(apiJson('/api/hosts')).rejects.toMatchObject({ status: 500 })
  })

  it('checkAuth true bila 200, false bila 401/network error', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 200, ok: true }))
    expect(await checkAuth()).toBe(true)
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('net down')))
    expect(await checkAuth()).toBe(false)
  })
})
