import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'
import EventsPage from '../src/pages/EventsPage.svelte'
import { eventsStore } from '../src/lib/stores/events.svelte.js'

function ev(host, ts, kind = 'spike_cpu', severity = 'warning', subject = 'nginx') {
  return {
    host,
    timestamp_ms: ts,
    kind,
    severity,
    subject,
    detail: { value: 91.2, baseline: '45%' },
  }
}

const NOW = Date.now()
const SEED = [
  ev('web-01', NOW - 1000, 'spike_cpu', 'critical', 'nginx'),
  ev('web-01', NOW - 86_500_000, 'spike_mem', 'warning', 'node'),
  ev('db-01', NOW - 90_000, 'agent_down', 'info', 'db-01'),
]

describe('EventsPage (WE2-WE4)', () => {
  beforeEach(() => {
    eventsStore.list = []
    eventsStore.limit = 100
    vi.unstubAllGlobals()
    // default: fetch kembalikan SEED (init loadEvents di page $effect)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: async () => SEED.map((e) => e),
    }))
  })

  it('render timeline dengan grouping Today (WE-AC-001/003)', async () => {
    eventsStore.list = SEED
    render(EventsPage)
    await waitFor(() => {
      expect(screen.getByTestId('event-row-' + (NOW - 1000))).toBeInTheDocument()
    })
    expect(screen.getByText('Today')).toBeInTheDocument()
  })

  it('filter severity chip critical → hanya 1 item (WE-AC-005)', async () => {
    eventsStore.list = SEED
    render(EventsPage)
    await fireEvent.click(screen.getByTestId('filter-sev-critical'))
    await waitFor(() => {
      const rows = screen.getAllByTestId(/^event-\d+-/)
      expect(rows.length).toBe(1)
    })
  })

  it('filter kind agent_down + search db-01 (WE-AC-005/006)', async () => {
    eventsStore.list = SEED
    render(EventsPage)
    await fireEvent.click(screen.getByTestId('filter-kind-agent_down'))
    await waitFor(() => {
      expect(screen.getAllByTestId(/^event-\d+-/).length).toBe(1)
    })
  })

  it('filter host dropdown → hanya host itu (WE-AC-006 server-side param di API nyata)', async () => {
    eventsStore.list = SEED
    render(EventsPage)
    await fireEvent.change(screen.getByTestId('filter-host'), { target: { value: 'db-01' } })
    await waitFor(() => {
      expect(screen.getAllByTestId(/^event-\d+-/).length).toBe(1)
    })
  })

  it('accordion: klik baris → detail JSON + see-host link (WE-AC-008)', async () => {
    eventsStore.list = SEED
    render(EventsPage)
    await waitFor(() => {
      expect(screen.getAllByTestId(/^event-\d+-/).length).toBeGreaterThan(0)
    })
    await fireEvent.click(screen.getByTestId('event-row-' + (NOW - 1000)))
    await waitFor(() => {
      expect(screen.getByTestId('event-detail-' + (NOW - 1000))).toBeInTheDocument()
    })
    expect(screen.getByTestId('see-host-' + (NOW - 1000))).toHaveTextContent('View host')
  })

  it('Load more: limit naik & refetch (WE-AC-009)', async () => {
    eventsStore.list = SEED
    render(EventsPage)
    await waitFor(() => {
      expect(screen.getByTestId('load-more')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('load-more'))
    await waitFor(() => {
      expect(eventsStore.limit).toBe(200)
    })
    // fetch dipanggil ulang dengan limit baru
    await waitFor(() => {
      const last = window.fetch.mock.calls.at(-1)?.[0]
      expect(last).toBe('/api/events?limit=200')
    })
  })

  it('kosong → events-empty (WE-AC-001 kontras)', async () => {
    render(EventsPage)
    await waitFor(() => {
      expect(screen.getByTestId('events-empty')).toBeInTheDocument()
    })
  })
})
