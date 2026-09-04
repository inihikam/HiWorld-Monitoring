import { describe, it, expect, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import App from '../src/App.svelte'

describe('App (shell, WD7)', () => {
  beforeEach(() => {
    localStorage.clear()
    window.location.hash = '#/'
  })

  it('renders shell dengan overview page', () => {
    render(App)
    expect(screen.getByTestId('page-overview')).toBeInTheDocument()
  })

  it('theme & locale control tersedia di TopBar', () => {
    render(App)
    expect(screen.getByTestId('theme-toggle')).toBeInTheDocument()
    expect(screen.getByTestId('locale-select')).toBeInTheDocument()
  })
})
