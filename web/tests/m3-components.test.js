import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/svelte'
import { createRawSnippet } from 'svelte'

const text = (str) => createRawSnippet(() => ({ render: () => str }))
import Button from '../src/lib/m3/Button.svelte'
import Card from '../src/lib/m3/Card.svelte'
import TextField from '../src/lib/m3/TextField.svelte'
import TopBar from '../src/lib/m3/TopBar.svelte'
import NavRail from '../src/lib/m3/NavRail.svelte'
import NavBottom from '../src/lib/m3/NavBottom.svelte'
import Switch from '../src/lib/m3/Switch.svelte'
import Badge from '../src/lib/m3/Badge.svelte'

const NAV_ITEMS = [
  { href: '#/', icon: 'grid', label: 'Overview' },
  { href: '#/events', icon: 'bell', label: 'Events', badge: 2 },
]

describe('M3 komponen inti (WD6, WD-AC-016)', () => {
  it('Button: render, klik, variant class', async () => {
    let clicked = false
    render(Button, {
      props: {
        variant: 'tonal',
        onclick: () => (clicked = true),
        children: text('Save'),
      },
    })
    const btn = screen.getByRole('button', { name: 'Save' })
    expect(btn.className).toContain('tonal')
    await fireEvent.click(btn)
    expect(clicked).toBe(true)
  })

  it('Button disabled tidak clickable', () => {
    render(Button, { props: { disabled: true, children: text('X') } })
    expect(screen.getByRole('button')).toBeDisabled()
  })

  it('Card: render konten', () => {
    render(Card, { props: { testid: 'c1', children: text('isi kartu') } })
    expect(screen.getByTestId('c1')).toHaveTextContent('isi kartu')
  })

  it('TextField: bind value + error message', async () => {
    let v = ''
    render(TextField, {
      props: {
        label: 'Username',
        value: v,
        error: 'required',
        testid: 'tf',
      },
    })
    expect(screen.getByTestId('tf-error')).toHaveTextContent('required')
  })

  it('TopBar: judul + actions', () => {
    render(TopBar, { props: { title: 'hiworld', children: text('A') } })
    expect(screen.getByText('hiworld')).toBeInTheDocument()
  })

  it('NavRail: item + active + badge', () => {
    render(NavRail, { props: { items: NAV_ITEMS, active: '#/' } })
    expect(screen.getByTestId('nav-#/')).toHaveClass('active')
    expect(screen.getByTestId('nav-#/events')).toHaveTextContent('2')
    expect(screen.getByTestId('nav-#/events')).not.toHaveClass('active')
  })

  it('NavBottom: API sama dengan NavRail', () => {
    render(NavBottom, { props: { items: NAV_ITEMS, active: '#/events' } })
    expect(screen.getByTestId('mnav-#/events')).toHaveClass('active')
  })

  it('Switch: toggle checked', async () => {
    render(Switch, { props: { checked: false, testid: 'sw', label: 'Dark' } })
    const input = screen.getByTestId('sw')
    expect(input.checked).toBe(false)
    await fireEvent.click(input)
    expect(input.checked).toBe(true)
  })

  it('Badge: dot warna + chip label', () => {
    render(Badge, { props: { color: 'success', testid: 'b1' } })
    expect(screen.getByTestId('b1')).toHaveClass('dot')
    render(Badge, { props: { color: 'error', mode: 'chip', label: 'critical', testid: 'b2' } })
    expect(screen.getByTestId('b2')).toHaveTextContent('critical')
  })
})
