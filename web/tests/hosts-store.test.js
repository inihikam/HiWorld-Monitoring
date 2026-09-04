import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  hostsStore,
  applyHello,
  applySnapshot,
  applyStatus,
  sortedHosts,
  isStale,
  summary,
} from '../src/lib/stores/hosts.svelte.js'

function snap(host, ts, cpu = 10, mem = 20) {
  return { host_id: host, timestamp_ms: ts, system: { cpu_percent: cpu, mem_percent: mem } }
}

describe('hostsStore (WO1)', () => {
  beforeEach(() => {
    hostsStore.byHost = {}
    vi.useFakeTimers()
    vi.setSystemTime(1_000_000_000_000)
  })

  it('applyHello mengisi hosts + online dari ada-tidaknya latest', () => {
    applyHello({ hosts: [{ host_id: 'a', latest: snap('a', 1) }, { host_id: 'b', latest: null }] })
    expect(hostsStore.byHost['a'].online).toBe(true)
    expect(hostsStore.byHost['b'].online).toBe(false)
    expect(hostsStore.byHost['b'].latest).toBeNull()
  })

  it('applySnapshot set online + lastSeen; applyStatus freeze data', () => {
    applyHello({ hosts: [{ host_id: 'x', latest: null }] })
    applySnapshot(snap('x', 5, 33))
    expect(hostsStore.byHost['x'].online).toBe(true)
    expect(hostsStore.byHost['x'].lastSeenMs).toBe(1_000_000_000_000)

    applyStatus({ host_id: 'x', online: false })
    expect(hostsStore.byHost['x'].online).toBe(false)
    expect(hostsStore.byHost['x'].latest.timestamp_ms).toBe(5) // dibekukan
  })

  it('sortedHosts (Q-WO1): offline paling atas, lalu stale, sehat bawah', () => {
    const now = 1_000_000_000_000
    applyHello({
      hosts: [
        { host_id: 'healthy-1', latest: snap('healthy-1', now - 1000) },
        { host_id: 'offline-1', latest: snap('offline-1', now - 999_000) },
        { host_id: 'stale-1', latest: snap('stale-1', now - 120_000) },
        { host_id: 'healthy-2', latest: snap('healthy-2', now - 2000) },
        { host_id: 'offline-2', latest: snap('offline-2', now - 500_000) },
      ],
    })
    // offline-1 & offline-2: lastSeen lama → tampak offline via applyStatus
    applyStatus({ host_id: 'offline-1', online: false })
    applyStatus({ host_id: 'offline-2', online: false })
    // stale-1: online tapi snapshot 120s lalu; lastSeen juga lama
    // (applyHello set lastSeen dari snapshot ts)

    const order = sortedHosts(hostsStore.byHost).map((h) => h.host_id)
    expect(order[0]).toBe('offline-1') // offline paling tua di paling atas
    expect(order[1]).toBe('offline-2')
    expect(order[2]).toBe('stale-1') // stale
    expect(order.slice(3)).toEqual(['healthy-1', 'healthy-2']) // sehat alfabetis
  })

  it('isStale: online + lastSeen >60s → true; offline → selalu false', () => {
    const now = Date.now()
    expect(isStale({ online: true, lastSeenMs: now - 61_000 }, now)).toBe(true)
    expect(isStale({ online: true, lastSeenMs: now - 30_000 }, now)).toBe(false)
    expect(isStale({ online: false, lastSeenMs: now - 999_999 }, now)).toBe(false)
  })

  it('summary: agregat dari host online saja (ADR WO-4)', () => {
    applyHello({
      hosts: [
        { host_id: 'a', latest: snap('a', 1, 40, 50) },
        { host_id: 'b', latest: snap('b', 2, 80, 60) },
        { host_id: 'c', latest: snap('c', 3, 99, 99) },
      ],
    })
    applyStatus({ host_id: 'c', online: false })
    const s = summary(hostsStore.byHost)
    expect(s.online).toBe(2)
    expect(s.total).toBe(3)
    expect(s.avgCpu).toBe(60) // (40+80)/2, host c dikecualikan
    expect(s.avgRam).toBe(55)
  })

  it('summary kosong → null avg, tidak NaN', () => {
    const s = summary({})
    expect(s.online).toBe(0)
    expect(s.avgCpu).toBeNull()
  })
})
