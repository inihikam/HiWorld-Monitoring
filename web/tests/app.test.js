import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import App from '../src/App.svelte'

describe('App scaffold', () => {
  it('renders title', () => {
    render(App)
    expect(screen.getByText('hiworld monitoring')).toBeInTheDocument()
  })

  it('reaches api probe state', async () => {
    // stub fetch sebelum render
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true }))
    render(App)
    await vi.waitFor(() => {
      expect(screen.getByTestId('scaffold-status')).toHaveTextContent('api reachable')
    })
  })
})
