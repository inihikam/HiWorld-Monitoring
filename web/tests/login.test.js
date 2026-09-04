import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import LoginPage from '../src/pages/LoginPage.svelte'
import App from '../src/App.svelte'

function stubMedia() {
  window.matchMedia = vi.fn().mockImplementation((q) => ({
    matches: false,
    media: q,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }))
}

describe('LoginPage (WD8, WD-AC-010)', () => {
  beforeEach(() => {
    localStorage.clear()
    window.location.hash = '#/login'
    vi.unstubAllGlobals()
    stubMedia()
  })

  it('render form lengkap', () => {
    render(LoginPage)
    expect(screen.getByTestId('login-form')).toBeInTheDocument()
    expect(screen.getByTestId('login-username')).toBeInTheDocument()
    expect(screen.getByTestId('login-password')).toBeInTheDocument()
  })

  it('login sukses → tidak ada error', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, status: 200 }))
    render(LoginPage)
    const { fireEvent } = await import('@testing-library/svelte')
    await fireEvent.click(screen.getByRole('button'))
    await waitFor(() => {
      expect(screen.queryByTestId('login-error')).not.toBeInTheDocument()
    })
  })

  it('login gagal (401) → pesan error tampil', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 401 }))
    render(LoginPage)
    const { fireEvent } = await import('@testing-library/svelte')
    await fireEvent.click(screen.getByRole('button'))
    await waitFor(() => {
      expect(screen.getByTestId('login-error')).toBeInTheDocument()
    })
  })
})

describe('App auth guard (WD-AC-010/011)', () => {
  beforeEach(() => {
    localStorage.clear()
    window.location.hash = '#/'
    vi.unstubAllGlobals()
    stubMedia()
  })

  it('belum login → boot probe lalu LoginPage', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 401, ok: false }))
    render(App)
    await waitFor(() => {
      expect(screen.getByTestId('login-form')).toBeInTheDocument()
    })
  })

  it('sudah login → shell tampil', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ status: 200, ok: true }))
    render(App)
    await waitFor(() => {
      expect(screen.getByTestId('page-overview')).toBeInTheDocument()
    })
  })
})
