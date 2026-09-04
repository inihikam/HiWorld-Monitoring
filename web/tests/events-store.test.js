import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  eventsStore,
  loadEvents,
  loadMore,
  applyWsEvent,
  groupByDay,
  filterEvents,
} from '../src/lib/stores/events.svelte.js'

function ev(host, ts, kind = 'spike_cpu', severity = 'warning', subject = 'nginx') {
  return { host, timestamp_ms: ts, kind, severity, subject, detail: { value: 91 } }
}

const DAY = 86_400_000

describe('eventsStore (WE1)', () => {
  beforeEach(() => {
    eventsStore.list = []
    eventsStore.limit = 100
    vi.unstubAllGlobals()
  })

  it('loadEvents: terbaru di atas (WE-AC-001)', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => [ev('a', 1000), ev('a', 3000), ev('b', 2000)],
    })
    await loadEvents(fetchMock)
    expect(eventsStore.list.map((e) => e.ts)).toEqual([3000, 2000, 1000])
    const [url] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/events?limit=100')
  })

  it('loadEvents error → error state', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500 }))
    await loadEvents((u) => fetch(u))
    expect(eventsStore.error).toBe('HTTP 500')
  })

  it('loadMore: limit naik 100→200 (WE-AC-009, ADR WE-3)', () => {
    loadMore()
    expect(eventsStore.limit).toBe(200)
    loadMore()
    expect(eventsStore.limit).toBe(300)
  })

  it('applyWsEvent: prepend + dedupe ts+kind+host (WE-AC-007)', () => {
    eventsStore.list = [ev('a', 2000)]
    applyWsEvent(ev('a', 3000, 'spike_cpu'))
    expect(eventsStore.list[0].ts).toBe(3000)
    // duplikat pesan WS re-send → tidak dobel
    applyWsEvent(ev('a', 3000, 'spike_cpu'))
    expect(eventsStore.list.length).toBe(2)
  })

  it('groupByDay: Today/Yesterday/tanggal (WE-AC-003)', () => {
    const now = Date.now()
    const list = [
      { ts: now - 1000 },
      { ts: now - DAY - 1000 },
      { ts: now - 2 * DAY - 1000 },
    ]
    const groups = groupByDay(list, now)
    expect(groups.length).toBe(3)
    expect(groups[0].label).toBe('__today__')
    expect(groups[1].label).toBe('__yesterday__')
    expect(groups[2].label).not.toBe('__today__')
    expect(groups[0].items.length).toBe(1)
  })

  it('filterEvents kombinasi severity+kind+search (WE-AC-005)', () => {
    const list = [
      ev('a', 1, 'spike_cpu', 'critical', 'nginx'),
      ev('a', 2, 'spike_mem', 'warning', 'node'),
      ev('b', 3, 'agent_down', 'info', 'web-01'),
      ev('b', 4, 'disk_almost_full', 'critical', '/data'),
    ]
    expect(filterEvents(list, { severities: ['critical'] }).length).toBe(2)
    expect(filterEvents(list, { kinds: ['agent_down'] }).length).toBe(1)
    expect(filterEvents(list, { search: 'nginx' }).length).toBe(1)
    expect(filterEvents(list, { search: 'WEB-01' }).length).toBe(1) // case-insensitive
    expect(
      filterEvents(list, { severities: ['critical'], kinds: ['spike_cpu'] }).length
    ).toBe(1)
    expect(filterEvents(list, {}).length).toBe(4)
  })
})
