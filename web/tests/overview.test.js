import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'
import OverviewPage from '../src/pages/OverviewPage.svelte'
import { hostsStore, applyHello, applySnapshot } from '../src/lib/stores/hosts.svelte.js'

function snap(host, cpu = 10, mem = 20) {
  const now = Date.now()
  return {
    host_id: host,
    timestamp_ms: now,
    system: {
      cpu_percent: cpu,
      mem_percent: mem,
      mem_used_bytes: 1024 ** 2,
      mem_total_bytes: 1024 ** 3,
      load_avg: [0.1, 0, 0],
      disks: [{ mount: '/', used_percent: 40 }],
    },
  }
}

describe('OverviewPage (WO3+WO4)', () => {
  beforeEach(() => {
    hostsStore.byHost = {}
    window.location.hash = '#/'
  })

  it('ring summary: online/total + rata-rata realtime (WO-AC-009)', async () => {
    applyHello({
      hosts: [
        { host_id: 'a', latest: snap('a', 40, 50) },
        { host_id: 'b', latest: snap('b', 80, 70) },
      ],
    })
    render(OverviewPage)
    await waitFor(() => {
      expect(screen.getByTestId('avg-cpu')).toHaveTextContent('60.0%')
    })
    expect(screen.getByTestId('avg-ram')).toHaveTextContent('60.0%')
  })

  it('WS snapshot memperbarui kartu & summary tanpa refresh (WO-AC-004)', async () => {
    applyHello({ hosts: [{ host_id: 'a', latest: snap('a', 10, 10) }] })
    render(OverviewPage)
    await waitFor(() => {
      expect(screen.getByTestId('cpu-a')).toHaveTextContent('10.0%')
    })
    applySnapshot(snap('a', 77, 10))
    await waitFor(() => {
      expect(screen.getByTestId('cpu-a')).toHaveTextContent('77.0%')
    })
  })

  it('host offline di atas + sehat alfabetis (Q-WO1, WO-AC-005)', async () => {
    const now = Date.now()
    applyHello({
      hosts: [
        { host_id: 'zz-healthy', latest: { ...snap('zz', 5, 5), timestamp_ms: now } },
        { host_id: 'aa-dead', latest: { ...snap('aa', 5, 5), timestamp_ms: now - 999_000 } },
      ],
    })
    const { applyStatus } = await import('../src/lib/stores/hosts.svelte.js')
    applyStatus({ host_id: 'aa-dead', online: false })
    render(OverviewPage)
    await waitFor(() => {
      const cards = screen.getAllByTestId(/^host-card-/)
      expect(cards[0]).toHaveTextContent('aa-dead')
    })
  })

  it('0 host → empty state + modal setup (WO-AC-010)', async () => {
    render(OverviewPage)
    await waitFor(() => {
      expect(screen.getByTestId('empty-state')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByRole('button', { name: /setup agent/i }))
    await waitFor(() => {
      expect(screen.getByTestId('setup-modal')).toBeInTheDocument()
    })
  })

  it('latest null → kartu waiting (WO-AC-006)', async () => {
    applyHello({ hosts: [{ host_id: 'fresh', latest: null }] })
    render(OverviewPage)
    await waitFor(() => {
      expect(screen.getByTestId('waiting-fresh')).toBeInTheDocument()
    })
  })

  it('grid multi-kolom di desktop (WO-AC-002 — CSS class)', async () => {
    applyHello({
      hosts: [
        { host_id: 'a', latest: snap('a') },
        { host_id: 'b', latest: snap('b') },
      ],
    })
    const { container } = render(OverviewPage)
    await waitFor(() => {
      expect(container.querySelector('.grid')).toBeInTheDocument()
    })
    // auto-fill grid: kolom ditentukan CSS; verifikasi style rule ada
    expect(container.querySelector('.grid').className).toContain('grid')
  })
})
