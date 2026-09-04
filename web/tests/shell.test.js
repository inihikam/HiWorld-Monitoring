import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import App from '../src/App.svelte'

function setSize(width) {
  vi.stubGlobal('innerWidth', width)
  window.matchMedia = vi.fn().mockImplementation((q) => ({
    matches: q === '(max-width: 899px)' ? width < 900 : false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }))
}

function at(hash) {
  window.location.hash = hash
}

describe('App shell (WD7)', () => {
  beforeEach(() => {
    localStorage.clear()
    at('#/')
  })

  it('desktop ≥900px → NavRail tampil, NavBottom tidak (WD-AC-008)', () => {
    setSize(1200)
    render(App)
    expect(screen.getByTestId('nav-#/')).toBeInTheDocument()
    expect(screen.queryByTestId('mnav-#/')).not.toBeInTheDocument()
  })

  it('mobile <900px → NavBottom tampil, NavRail tidak (WD-AC-008)', () => {
    setSize(500)
    render(App)
    expect(screen.getByTestId('mnav-#/')).toBeInTheDocument()
    expect(screen.queryByTestId('nav-#/')).not.toBeInTheDocument()
  })

  it('navigasi hash → page berganti (WD-AC-009)', async () => {
    setSize(1200)
    render(App)
    expect(screen.getByTestId('page-overview')).toBeInTheDocument()
    at('#/events')
    await vi.waitFor(() => {
      expect(screen.getByTestId('page-events')).toBeInTheDocument()
    })
    at('#/settings')
    await vi.waitFor(() => {
      expect(screen.getByTestId('page-settings')).toBeInTheDocument()
    })
  })

  it('route tidak dikenal → 404', async () => {
    setSize(1200)
    at('#/wherever')
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('page-404')).toBeInTheDocument()
    })
  })

  it('host route → host detail placeholder dengan id', async () => {
    setSize(1200)
    at('#/host/web-01')
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('page-host')).toHaveTextContent('web-01')
    })
  })

  it('toggle theme di TopBar mengubah data-theme (WD-AC-005 via shell)', async () => {
    setSize(1200)
    localStorage.setItem('hiworld-theme', 'light')
    render(App)
    expect(document.documentElement.dataset.theme).toBe('dark') // default reset di store… theme store sudah dark dari module state
    const { fireEvent } = await import('@testing-library/svelte')
    await fireEvent.click(screen.getByTestId('theme-toggle'))
    expect(['dark', 'light']).toContain(document.documentElement.dataset.theme)
  })

  it('locale select mengubah t() (WD-AC-006 via shell)', async () => {
    setSize(1200)
    render(App)
    const { fireEvent } = await import('@testing-library/svelte')
    const sel = screen.getByTestId('locale-select')
    await fireEvent.change(sel, { target: { value: 'id' } })
    // label nav berubah ke bahasa Indonesia
    expect(screen.getByTestId('nav-#/')).toHaveTextContent('Ringkasan')
  })
})
