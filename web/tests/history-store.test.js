import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  historyStore,
  parseHistory,
  loadHistory,
  appendSnapshot,
} from '../src/lib/stores/history.svelte.js'

const ROWS = [
  {
    timestamp_ms: 1000,
    cpu_percent: 10,
    mem_percent: 20,
    mem_used_bytes: 1,
    mem_total_bytes: 2,
    load_1m: 0.5,
    disks: [{ mount: '/', used_percent: 40 }, { mount: '/data', used_percent: 80 }],
  },
  {
    timestamp_ms: 2000,
    cpu_percent: NaN, // agent down → null (WH-AC-005)
    mem_percent: 25,
    mem_used_bytes: 1,
    mem_total_bytes: 2,
    load_1m: NaN,
    disks: [{ mount: '/', used_percent: 41 }, { mount: '/data', used_percent: 81 }],
  },
]

describe('historyStore (WH1)', () => {
  beforeEach(() => {
    historyStore.host = null
    historyStore.series = { cpu: [], mem: [], disks: {}, load: [] }
    vi.unstubAllGlobals()
  })

  it('parseHistory: NaN → null (WH-AC-005)', () => {
    const s = parseHistory(ROWS)
    expect(s.cpu).toEqual([[1000, 10], [2000, null]])
    expect(s.mem[1]).toEqual([2000, 25])
    expect(s.load[1]).toEqual([2000, null])
    expect(s.disks['/']).toEqual([[1000, 40], [2000, 41]])
    expect(s.disks['/data']).toEqual([[1000, 80], [2000, 81]])
  })

  it('loadHistory: query params from/to sesuai range (WH-AC-003)', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ROWS })
    await loadHistory(fetchMock, 'web-01', 900_000)
    const [url] = fetchMock.mock.calls[0]
    expect(url).toMatch(/^\/api\/history\?host=web-01&from=\d+&to=\d+$/)
    expect(historyStore.host).toBe('web-01')
    expect(historyStore.rangeMs).toBe(900_000)
    expect(historyStore.series.cpu.length).toBe(2)
    expect(historyStore.loading).toBe(false)
  })

  it('loadHistory error → error state, loading false', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500 }))
    await loadHistory((u, o) => fetch(u, o), 'x')
    expect(historyStore.error).toBe('HTTP 500')
    expect(historyStore.loading).toBe(false)
  })

  it('appendSnapshot host lain → diabaikan (WH-AC-004 scope)', () => {
    historyStore.host = 'a'
    appendSnapshot({ host_id: 'b', timestamp_ms: 1, system: {} })
    expect(historyStore.series.cpu.length).toBe(0)
  })

  it('appendSnapshot host ini → push semua series (WH-AC-004)', () => {
    historyStore.host = 'a'
    historyStore.series = parseHistory(ROWS)
    const before = historyStore.series.cpu.length
    appendSnapshot({
      host_id: 'a',
      timestamp_ms: 3000,
      system: {
        cpu_percent: 55,
        mem_percent: 30,
        load_avg: [1.2, 0, 0],
        disks: [{ mount: '/', used_percent: 42 }],
      },
    })
    expect(historyStore.series.cpu.length).toBe(before + 1)
    expect(historyStore.series.cpu.at(-1)).toEqual([3000, 55])
    expect(historyStore.series.disks['/'].at(-1)).toEqual([3000, 42])
  })
})
