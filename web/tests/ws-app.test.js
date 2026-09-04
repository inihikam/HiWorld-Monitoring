import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import App from '../src/App.svelte'
import { hostsStore, applyHello, applySnapshot, applyStatus } from '../src/lib/stores/hosts.svelte.js'

function stubMedia() {
  window.matchMedia = vi.fn().mockImplementation((q) => ({
    matches: false,
    media: q,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }))
}

describe('App + WS integration (WD9, WD-AC-013)', () => {
  beforeEach(() => {
    localStorage.clear()
    window.location.hash = '#/'
    vi.unstubAllGlobals()
    stubMedia()
    hostsStore.byHost = {}
  })

  it('auth ok → WS dibuat & badge status tampil di TopBar', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 200, ok: true }))
    render(App)
    await waitFor(() => {
      expect(screen.getByTestId('ws-state')).toBeInTheDocument()
    })
    // MockWebSocket global jsdom? tidak ada — WS stub diperlukan
  })

  it('hostsStore: applyHello → applySnapshot → applyStatus benar', () => {
    applyHello({ hosts: [{ host_id: 'h1', latest: null }] })
    expect(hostsStore.byHost['h1']).toEqual({ latest: null, online: false })

    applySnapshot({ host_id: 'h1', timestamp_ms: 1 })
    expect(hostsStore.byHost['h1'].latest.timestamp_ms).toBe(1)
    expect(hostsStore.byHost['h1'].online).toBe(true)

    applyStatus({ host_id: 'h1', online: false })
    expect(hostsStore.byHost['h1'].online).toBe(false)
    // data latest tetap dibekukan
    expect(hostsStore.byHost['h1'].latest.timestamp_ms).toBe(1)
  })

  it('applyStatus untuk host tak dikenal → diabaikan (tanpa crash)', () => {
    expect(() => applyStatus({ host_id: 'ghost', online: true })).not.toThrow()
    expect(hostsStore.byHost['ghost']).toBeUndefined()
  })
})
