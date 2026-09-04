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

function stubFetchOk() {
  window.fetch = vi.fn().mockResolvedValue({ status: 200, ok: true })
}

function at(hash) {
  window.location.hash = hash
}

describe('App shell (WD7+WD8)', () => {
  beforeEach(() => {
    localStorage.clear()
    at('#/')
    vi.unstubAllGlobals()
    stubFetchOk()
  })

  it('desktop >=900px: NavRail tampil, NavBottom tidak (WD-AC-008)', async () => {
    setSize(1200)
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('nav-#/')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('mnav-#/')).not.toBeInTheDocument()
  })

  it('mobile <900px: NavBottom tampil, NavRail tidak (WD-AC-008)', async () => {
    setSize(500)
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('mnav-#/')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('nav-#/')).not.toBeInTheDocument()
  })

  it('navigasi hash: page berganti (WD-AC-009)', async () => {
    setSize(1200)
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('overview-page')).toBeInTheDocument()
    })
    at('#/events')
    await vi.waitFor(() => {
      expect(screen.getByTestId('events-page')).toBeInTheDocument()
    })
    at('#/settings')
    await vi.waitFor(() => {
      expect(screen.getByTestId('page-settings')).toBeInTheDocument()
    })
  })

  it('route tidak dikenal: 404', async () => {
    setSize(1200)
    at('#/wherever')
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('page-404')).toBeInTheDocument()
    })
  })

  it('host route: host detail page dengan id', async () => {
    setSize(1200)
    at('#/host/web-01')
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('detail-page')).toBeInTheDocument()
    })
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('web-01')
  })

  it('toggle theme di TopBar mengubah data-theme (WD-AC-005 via shell)', async () => {
    setSize(1200)
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('theme-toggle')).toBeInTheDocument()
    })
    const { fireEvent } = await import('@testing-library/svelte')
    const before = document.documentElement.dataset.theme
    await fireEvent.click(screen.getByTestId('theme-toggle'))
    expect(document.documentElement.dataset.theme).not.toBe(before)
  })

  it('locale select mengubah label nav (WD-AC-006 via shell)', async () => {
    setSize(1200)
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('locale-select')).toBeInTheDocument()
    })
    const { fireEvent } = await import('@testing-library/svelte')
    await fireEvent.change(screen.getByTestId('locale-select'), {
      target: { value: 'id' },
    })
    await vi.waitFor(() => {
      expect(screen.getByTestId('nav-#/')).toHaveTextContent('Ringkasan')
    })
  })

  it('belum login: LoginPage tampil (WD-AC-010)', async () => {
    setSize(1200)
    window.fetch = vi.fn().mockResolvedValue({ status: 401, ok: false })
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('login-form')).toBeInTheDocument()
    })
  })

  it('logout button ada di TopBar', async () => {
    setSize(1200)
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('logout-btn')).toBeInTheDocument()
    })
  })
})
